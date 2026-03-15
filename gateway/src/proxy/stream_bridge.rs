//! Stream bridge: SSE passthrough with background accumulation.
//!
//! Provides [`tee_sse_stream`] which takes a reqwest streaming response and:
//! 1. Applies PII redaction to each SSE data payload
//! 2. Forwards (redacted) SSE bytes to the client immediately
//! 3. Feeds each line into a [`StreamAccumulator`] in a background task
//! 4. Resolves a [`StreamResult`] when the stream completes (for audit/cost)
//!
//! Uses `tokio::sync::Notify` for instant stream-completion signaling.

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use bytes::Bytes;
use futures::StreamExt;
use tokio::sync::{Mutex, Notify};

use crate::proxy::stream::{StreamAccumulator, StreamResult};

/// Spawn a fire-and-forget task with panic logging.
/// Panics are logged instead of silently swallowed.
macro_rules! spawn_logged {
    ($task:expr) => {{
        let handle = tokio::spawn($task);
        tokio::spawn(async move {
            if let Err(e) = handle.await {
                tracing::error!("Spawned task failed: {:?}", e);
            }
        });
    }};
}

/// MED-12: Maximum residual buffer size (4KB) to prevent unbounded memory growth.
/// This prevents malicious or corrupted streams from consuming excessive memory.
const MAX_UTF8_RESIDUAL_SIZE: usize = 4 * 1024;

/// A shared slot that will be populated with the stream result once the
/// SSE stream completes. The background task writes to this; the caller
/// can await it after the response has been sent to the client.
pub type StreamResultSlot = Arc<Mutex<Option<StreamResult>>>;

/// Tee an upstream SSE response into two consumers:
/// - An [`axum::body::Body`] that streams bytes directly to the HTTP client
/// - A [`StreamResultSlot`] that resolves with accumulated usage/tool-call data
///
/// The `start` instant is used to compute TTFT (time-to-first-token).
///
/// # Usage
/// ```ignore
/// let (body, result_slot) = tee_sse_stream(upstream_resp, Instant::now());
/// // Send body to client immediately
/// let response = Response::builder().body(body).unwrap();
/// // Later (in a spawned task), read the result for audit/cost
/// let result = wait_for_stream_result(&result_slot, Duration::from_secs(300)).await;
/// ```
pub fn tee_sse_stream(
    upstream_resp: reqwest::Response,
    start: Instant,
) -> (Body, StreamResultSlot, Arc<Notify>) {
    let result_slot: StreamResultSlot = Arc::new(Mutex::new(None));
    let slot_for_bg = result_slot.clone();
    let notify = Arc::new(Notify::new());
    let notify_bg = notify.clone();

    let accumulator = Arc::new(Mutex::new(StreamAccumulator::new_with_start(start)));
    let mut byte_stream = upstream_resp.bytes_stream();

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(1024);

    spawn_logged!(async move {
        let mut first = true;
        // 5A-1 FIX: Track whether the client has disconnected. When true, we
        // continue reading the upstream to capture the usage/cost data from
        // the final SSE chunk, but skip sending bytes to the (gone) client.
        let mut client_gone = false;
        // B3-2 FIX: Buffer for incomplete UTF-8 sequences split across TCP chunks.
        // SSE is text-based, so a multi-byte char can be sliced at a chunk boundary.
        // We hold trailing incomplete bytes and prepend them to the next chunk.
        // MED-12: Limit residual buffer size to prevent unbounded memory growth.
        let mut utf8_residual: Vec<u8> = Vec::new();

        while let Some(chunk_result) = byte_stream.next().await {
            match chunk_result {
                Ok(bytes) => {
                    let mut acc_guard = accumulator.lock().await;

                    // Record TTFT on the very first data chunk
                    if first {
                        first = false;
                        acc_guard.set_ttft_ms(start.elapsed().as_millis() as u64);
                    }

                    // FIX H-7: Use Cow to avoid unsafe raw pointer aliasing.
                    // When no residual, borrow bytes directly (zero alloc).
                    // When residual exists, combine into owned Vec.
                    let combined: Cow<[u8]> = if utf8_residual.is_empty() {
                        Cow::Borrowed(&bytes[..])
                    } else {
                        let mut buf = std::mem::take(&mut utf8_residual);
                        buf.extend_from_slice(&bytes);
                        Cow::Owned(buf)
                    };

                    // Find the longest valid UTF-8 prefix; keep any trailing
                    // incomplete sequence for the next iteration.
                    let (valid_str, leftover) = match std::str::from_utf8(&combined) {
                        Ok(s) => (s, &[][..]),
                        Err(e) => {
                            let valid_up_to = e.valid_up_to();
                            // SAFETY: we just validated up to this index
                            let s = unsafe {
                                std::str::from_utf8_unchecked(&combined[..valid_up_to])
                            };
                            (s, &combined[valid_up_to..])
                        }
                    };
                    // MED-12: Check residual buffer size to prevent unbounded growth
                    if leftover.len() > MAX_UTF8_RESIDUAL_SIZE {
                        tracing::warn!(
                            residual_size = leftover.len(),
                            max_size = MAX_UTF8_RESIDUAL_SIZE,
                            "MED-12: UTF-8 residual buffer overflow, truncating. \
                             This may indicate a corrupted or malicious stream."
                        );
                        // Truncate to max size to prevent memory exhaustion
                        utf8_residual = leftover[..MAX_UTF8_RESIDUAL_SIZE].to_vec();
                    } else {
                        utf8_residual = leftover.to_vec();
                    }

                    // Feed each SSE line to the accumulator
                    let mut done = false;
                    for line in valid_str.split('\n') {
                        if acc_guard.push_sse_line(line) {
                            done = true;
                        }
                    }

                    // If stream is done via [DONE] tag, extract result early
                    if done {
                        let mut slot_guard = slot_for_bg.lock().await;
                        if slot_guard.is_none() {
                            let finished_acc =
                                std::mem::replace(&mut *acc_guard, StreamAccumulator::new());
                            *slot_guard = Some(finished_acc.finish());
                        }
                        notify_bg.notify_waiters();
                    }
                    drop(acc_guard);

                    // STREAMING-PII FIX: Apply PII redaction to SSE data
                    // lines before sending to the client. Non-data lines and
                    // chunks with no PII pass through with zero extra alloc.
                    let send_bytes = if !valid_str.is_empty() {
                        let (redacted, did_redact) =
                            crate::middleware::sanitize::redact_sse_chunk(valid_str);
                        if did_redact {
                            Bytes::from(redacted)
                        } else {
                            bytes
                        }
                    } else {
                        bytes
                    };

                    // 5A-1 FIX: Send to client unless they've disconnected.
                    // On disconnect, set client_gone and CONTINUE reading
                    // upstream so we capture the final usage/cost chunk.
                    if !client_gone && tx.send(Ok(send_bytes)).await.is_err() {
                        client_gone = true;
                        tracing::debug!(
                            "Client disconnected — continuing upstream read for billing"
                        );
                    }
                    // If [DONE] was already processed, we can stop early
                    if client_gone {
                        let slot_guard = slot_for_bg.lock().await;
                        if slot_guard.is_some() {
                            break; // Usage captured, safe to stop
                        }
                    }
                }
                Err(e) => {
                    // Emit a structured SSE error event so SSE clients receive a
                    // parseable error payload instead of a silent TCP reset.
                    let sse_error = format!(
                        "data: {{\"error\":{{\"message\":\"upstream connection lost: {}\",\"type\":\"stream_error\"}}}}\n\n",
                        e.to_string().replace('"', "'")
                    );
                    let _ = tx.send(Ok(Bytes::from(sse_error))).await;

                    // Populate the slot with partial results so audit logging still
                    // captures whatever tokens/content arrived before the drop.
                    {
                        let mut slot_guard = slot_for_bg.lock().await;
                        if slot_guard.is_none() {
                            let mut acc_guard = accumulator.lock().await;
                            let partial =
                                std::mem::replace(&mut *acc_guard, StreamAccumulator::new());
                            *slot_guard = Some(partial.finish());
                        }
                    }
                    notify_bg.notify_waiters();

                    let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, e.to_string());
                    let _ = tx.send(Err(io_err)).await;
                    break;
                }
            }
        }

        // EOF or client disconnect: ensure result_slot is populated if not already
        let mut slot_guard = slot_for_bg.lock().await;
        if slot_guard.is_none() {
            let mut acc_guard = accumulator.lock().await;
            let finished_acc = std::mem::replace(&mut *acc_guard, StreamAccumulator::new());
            *slot_guard = Some(finished_acc.finish());
        }
        notify_bg.notify_waiters();
    });

    let mapped = tokio_stream::wrappers::ReceiverStream::new(rx);
    let body = Body::from_stream(mapped);
    (body, result_slot, notify)
}

/// FIX(X2/X3): Tee an SSE stream with per-chunk translation.
///
/// Identical to [`tee_sse_stream`] except each text chunk is passed through
/// `translate_fn(raw_bytes, model)` before being sent to the client.
/// This is used for Anthropic and Gemini, which return SSE in their own
/// event format — the translation converts each chunk into OpenAI
/// `chat.completion.chunk` SSE before the client sees it.
///
/// The StreamAccumulator receives the **translated** SSE lines so that
/// token/cost extraction works correctly (it expects OpenAI format).
pub fn tee_translating_sse_stream<F>(
    upstream_resp: reqwest::Response,
    start: Instant,
    model: String,
    translate_fn: F,
) -> (Body, StreamResultSlot, Arc<Notify>)
where
    F: Fn(&[u8], &str) -> Vec<u8> + Send + 'static,
{
    let result_slot: StreamResultSlot = Arc::new(Mutex::new(None));
    let slot_for_bg = result_slot.clone();
    let notify = Arc::new(Notify::new());
    let notify_bg = notify.clone();

    let accumulator = Arc::new(Mutex::new(StreamAccumulator::new_with_start(start)));
    let mut byte_stream = upstream_resp.bytes_stream();

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(1024);

    spawn_logged!(async move {
        let mut first = true;
        // 5A-1 FIX: Continue reading upstream after client disconnect for billing.
        let mut client_gone = false;
        let mut utf8_residual: Vec<u8> = Vec::new();

        while let Some(chunk_result) = byte_stream.next().await {
            match chunk_result {
                Ok(bytes) => {
                    let mut acc_guard = accumulator.lock().await;

                    // Record TTFT on first data chunk
                    if first {
                        first = false;
                        acc_guard.set_ttft_ms(start.elapsed().as_millis() as u64);
                    }

                    // FIX H-7: Use Cow to avoid unsafe raw pointer aliasing.
                    let combined: Cow<[u8]> = if utf8_residual.is_empty() {
                        Cow::Borrowed(&bytes[..])
                    } else {
                        let mut buf = std::mem::take(&mut utf8_residual);
                        buf.extend_from_slice(&bytes);
                        Cow::Owned(buf)
                    };

                    // Find longest valid UTF-8 prefix
                    let (valid_bytes, leftover) = match std::str::from_utf8(&combined) {
                        Ok(_) => (&combined[..], &[][..]),
                        Err(e) => {
                            let valid_up_to = e.valid_up_to();
                            (&combined[..valid_up_to], &combined[valid_up_to..])
                        }
                    };
                    // MED-12: Check residual buffer size to prevent unbounded growth
                    if leftover.len() > MAX_UTF8_RESIDUAL_SIZE {
                        tracing::warn!(
                            residual_size = leftover.len(),
                            max_size = MAX_UTF8_RESIDUAL_SIZE,
                            "MED-12: UTF-8 residual buffer overflow, truncating (translated stream). \
                             This may indicate a corrupted or malicious stream."
                        );
                        utf8_residual = leftover[..MAX_UTF8_RESIDUAL_SIZE].to_vec();
                    } else {
                        utf8_residual = leftover.to_vec();
                    }

                    if valid_bytes.is_empty() {
                        drop(acc_guard);
                        continue;
                    }

                    // Translate provider SSE → OpenAI SSE
                    let translated = translate_fn(valid_bytes, &model);

                    // Feed translated SSE to accumulator
                    if let Ok(translated_str) = std::str::from_utf8(&translated) {
                        let mut done = false;
                        for line in translated_str.split('\n') {
                            if acc_guard.push_sse_line(line) {
                                done = true;
                            }
                        }

                        if done {
                            let mut slot_guard = slot_for_bg.lock().await;
                            if slot_guard.is_none() {
                                let finished_acc =
                                    std::mem::replace(&mut *acc_guard, StreamAccumulator::new());
                                *slot_guard = Some(finished_acc.finish());
                            }
                            notify_bg.notify_waiters();
                        }
                    }

                    drop(acc_guard);

                    // STREAMING-PII FIX: Apply PII redaction to translated
                    // SSE bytes before sending to the client.
                    let send_bytes = if let Ok(text) = std::str::from_utf8(&translated) {
                        let (redacted, did_redact) =
                            crate::middleware::sanitize::redact_sse_chunk(text);
                        if did_redact {
                            Bytes::from(redacted)
                        } else {
                            Bytes::from(translated)
                        }
                    } else {
                        Bytes::from(translated)
                    };

                    // 5A-1 FIX: Send translated bytes to client unless disconnected.
                    if !client_gone && tx.send(Ok(send_bytes)).await.is_err() {
                        client_gone = true;
                        tracing::debug!("Client disconnected — continuing upstream read for billing (translated)");
                    }
                    if client_gone {
                        let slot_guard = slot_for_bg.lock().await;
                        if slot_guard.is_some() {
                            break; // Usage captured, safe to stop
                        }
                    }
                }
                Err(e) => {
                    let sse_error = format!(
                        "data: {{\"error\":{{\"message\":\"upstream connection lost: {}\",\"type\":\"stream_error\"}}}}\n\n",
                        e.to_string().replace('"', "'")
                    );
                    let _ = tx.send(Ok(Bytes::from(sse_error))).await;

                    {
                        let mut slot_guard = slot_for_bg.lock().await;
                        if slot_guard.is_none() {
                            let mut acc_guard = accumulator.lock().await;
                            let partial =
                                std::mem::replace(&mut *acc_guard, StreamAccumulator::new());
                            *slot_guard = Some(partial.finish());
                        }
                    }
                    notify_bg.notify_waiters();

                    let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, e.to_string());
                    let _ = tx.send(Err(io_err)).await;
                    break;
                }
            }
        }

        // EOF: ensure result_slot is populated
        let mut slot_guard = slot_for_bg.lock().await;
        if slot_guard.is_none() {
            let mut acc_guard = accumulator.lock().await;
            let finished_acc = std::mem::replace(&mut *acc_guard, StreamAccumulator::new());
            *slot_guard = Some(finished_acc.finish());
        }
        notify_bg.notify_waiters();
    });

    let mapped = tokio_stream::wrappers::ReceiverStream::new(rx);
    let body = Body::from_stream(mapped);
    (body, result_slot, notify)
}

/// Wait for the stream result to be populated, with a timeout.
/// Uses `Notify` for instant signaling instead of polling.
/// Returns `None` if the timeout expires before the stream completes.
pub async fn wait_for_stream_result(
    slot: &StreamResultSlot,
    notify: &Notify,
    timeout: std::time::Duration,
) -> Option<StreamResult> {
    // Check if already populated (fast path for non-streaming or very fast streams)
    {
        let mut guard = slot.lock().await;
        if guard.is_some() {
            return guard.take();
        }
    }
    // Wait for Notify signal with timeout
    match tokio::time::timeout(timeout, notify.notified()).await {
        Ok(()) => {
            // Signaled — result should be ready
            slot.lock().await.take()
        }
        Err(_) => {
            // Timeout — try one last time in case of race
            slot.lock().await.take()
        }
    }
}

/// Tee a Bedrock binary event stream (`application/vnd.amazon.eventstream`)
/// into two consumers, translating binary frames to OpenAI SSE on the fly:
/// - An [`axum::body::Body`] that streams translated SSE to the HTTP client
/// - A [`StreamResultSlot`] for accumulated usage/audit data
///
/// This differs from [`tee_sse_stream`] because Bedrock uses a binary framing
/// protocol, not text-based SSE. We:
/// 1. Buffer incoming binary chunks (frames can span TCP boundaries)
/// 2. Decode complete binary frames using `decode_bedrock_event_stream`
/// 3. Translate each Bedrock event to an OpenAI `chat.completion.chunk` SSE line
/// 4. Pipe the translated SSE text to the client
/// 5. Feed SSE lines to StreamAccumulator for cost/audit tracking
pub fn tee_bedrock_stream(
    upstream_resp: reqwest::Response,
    start: Instant,
    model: String,
) -> (Body, StreamResultSlot, Arc<Notify>) {
    let result_slot: StreamResultSlot = Arc::new(Mutex::new(None));
    let slot_for_bg = result_slot.clone();
    let notify = Arc::new(Notify::new());
    let notify_bg = notify.clone();

    let accumulator = Arc::new(Mutex::new(StreamAccumulator::new_with_start(start)));
    let mut byte_stream = upstream_resp.bytes_stream();

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(1024);

    spawn_logged!(async move {
        let mut first = true;
        // 5A-1 FIX: Continue reading upstream after client disconnect for billing.
        let mut client_gone = false;
        // Buffer for accumulating binary data across TCP chunks.
        // Bedrock binary frames can be split across chunk boundaries.
        let mut binary_buffer: Vec<u8> = Vec::new();
        let chunk_id = format!("chatcmpl-{}", uuid::Uuid::new_v4().simple());

        while let Some(chunk_result) = byte_stream.next().await {
            match chunk_result {
                Ok(bytes) => {
                    let mut acc_guard = accumulator.lock().await;

                    if first {
                        first = false;
                        acc_guard.set_ttft_ms(start.elapsed().as_millis() as u64);
                    }

                    // Append new bytes to the binary buffer
                    binary_buffer.extend_from_slice(&bytes);

                    // Try to decode complete frames from the buffer
                    let mut sse_output = String::new();
                    let mut consumed = 0;

                    while consumed + 12 <= binary_buffer.len() {
                        let remaining = &binary_buffer[consumed..];
                        if remaining.len() < 4 {
                            break;
                        }
                        let total_length = u32::from_be_bytes([
                            remaining[0],
                            remaining[1],
                            remaining[2],
                            remaining[3],
                        ]) as usize;

                        if total_length < 16 || remaining.len() < total_length {
                            break; // Incomplete frame, wait for more data
                        }

                        // Decode this single complete frame
                        let frame_bytes = &remaining[..total_length];
                        let events =
                            crate::proxy::model_router::decode_bedrock_event_stream(frame_bytes);

                        for (event_type, payload) in &events {
                            match event_type.as_str() {
                                "messageStart" => {
                                    let role = payload
                                        .get("role")
                                        .and_then(|r| r.as_str())
                                        .unwrap_or("assistant");
                                    sse_output.push_str(
                                        &crate::proxy::model_router::openai_sse_chunk(
                                            &chunk_id,
                                            &model,
                                            serde_json::json!({"role": role, "content": ""}),
                                            None,
                                        ),
                                    );
                                }
                                "contentBlockStart" => {
                                    if let Some(start) = payload.get("start") {
                                        if let Some(tool_use) = start.get("toolUse") {
                                            let index = payload
                                                .get("contentBlockIndex")
                                                .and_then(|i| i.as_u64())
                                                .unwrap_or(0);
                                            let name = tool_use
                                                .get("name")
                                                .and_then(|n| n.as_str())
                                                .unwrap_or("");
                                            let tool_id = tool_use
                                                .get("toolUseId")
                                                .and_then(|id| id.as_str())
                                                .unwrap_or("");
                                            sse_output.push_str(
                                                &crate::proxy::model_router::openai_sse_chunk(
                                                    &chunk_id,
                                                    &model,
                                                    serde_json::json!({"tool_calls": [{
                                                        "index": index,
                                                        "id": tool_id,
                                                        "type": "function",
                                                        "function": {"name": name, "arguments": ""}
                                                    }]}),
                                                    None,
                                                ),
                                            );
                                        }
                                    }
                                }
                                "contentBlockDelta" => {
                                    if let Some(delta) = payload.get("delta") {
                                        if let Some(text) =
                                            delta.get("text").and_then(|t| t.as_str())
                                        {
                                            sse_output.push_str(
                                                &crate::proxy::model_router::openai_sse_chunk(
                                                    &chunk_id,
                                                    &model,
                                                    serde_json::json!({"content": text}),
                                                    None,
                                                ),
                                            );
                                        }
                                        if let Some(input) =
                                            delta.get("toolUse").and_then(|tu| tu.get("input"))
                                        {
                                            let input_str = if input.is_string() {
                                                input.as_str().unwrap_or("").to_string()
                                            } else {
                                                serde_json::to_string(input).unwrap_or_default()
                                            };
                                            let index = payload
                                                .get("contentBlockIndex")
                                                .and_then(|i| i.as_u64())
                                                .unwrap_or(0);
                                            sse_output.push_str(
                                                &crate::proxy::model_router::openai_sse_chunk(
                                                    &chunk_id,
                                                    &model,
                                                    serde_json::json!({"tool_calls": [{
                                                        "index": index,
                                                        "function": {"arguments": input_str}
                                                    }]}),
                                                    None,
                                                ),
                                            );
                                        }
                                    }
                                }
                                "messageStop" => {
                                    let finish =
                                        match payload.get("stopReason").and_then(|s| s.as_str()) {
                                            Some("end_turn") => "stop",
                                            Some("tool_use") => "tool_calls",
                                            Some("max_tokens") => "length",
                                            Some("stop_sequence") => "stop",
                                            Some("content_filtered") => "content_filter",
                                            _ => "stop",
                                        };
                                    sse_output.push_str(
                                        &crate::proxy::model_router::openai_sse_chunk(
                                            &chunk_id,
                                            &model,
                                            serde_json::json!({}),
                                            Some(finish),
                                        ),
                                    );
                                    sse_output.push_str("data: [DONE]\n\n");
                                }
                                "metadata" => {
                                    // Bedrock sends usage in the metadata event:
                                    // {"usage": {"inputTokens": N, "outputTokens": M}}
                                    // Emit as an OpenAI-format usage chunk so the
                                    // StreamAccumulator captures it for cost tracking.
                                    if let Some(usage) = payload.get("usage") {
                                        let input = usage
                                            .get("inputTokens")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0);
                                        let output_t = usage
                                            .get("outputTokens")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0);
                                        if input > 0 || output_t > 0 {
                                            let usage_chunk = serde_json::json!({
                                                "id": chunk_id,
                                                "object": "chat.completion.chunk",
                                                "created": chrono::Utc::now().timestamp(),
                                                "model": model,
                                                "choices": [],
                                                "usage": {
                                                    "prompt_tokens": input,
                                                    "completion_tokens": output_t,
                                                    "total_tokens": input + output_t,
                                                }
                                            });
                                            sse_output.push_str(&format!(
                                                "data: {}\n\n",
                                                serde_json::to_string(&usage_chunk)
                                                    .unwrap_or_default()
                                            ));
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }

                        consumed += total_length;
                    }

                    // Remove consumed bytes from buffer
                    if consumed > 0 {
                        binary_buffer.drain(..consumed);
                    }

                    // Feed translated SSE lines to accumulator
                    if !sse_output.is_empty() {
                        let mut done = false;
                        for line in sse_output.split('\n') {
                            if acc_guard.push_sse_line(line) {
                                done = true;
                            }
                        }
                        if done {
                            let mut slot_guard = slot_for_bg.lock().await;
                            if slot_guard.is_none() {
                                let finished =
                                    std::mem::replace(&mut *acc_guard, StreamAccumulator::new());
                                *slot_guard = Some(finished.finish());
                            }
                            notify_bg.notify_waiters();
                        }
                        drop(acc_guard);

                        // STREAMING-PII FIX: Apply PII redaction to Bedrock
                        // translated SSE before sending to the client.
                        let (redacted, did_redact) =
                            crate::middleware::sanitize::redact_sse_chunk(&sse_output);
                        let send_bytes = if did_redact {
                            Bytes::from(redacted)
                        } else {
                            Bytes::from(sse_output)
                        };

                        // 5A-1 FIX: Send translated SSE to client unless disconnected.
                        if !client_gone && tx.send(Ok(send_bytes)).await.is_err() {
                            client_gone = true;
                            tracing::debug!("Client disconnected — continuing upstream read for billing (bedrock)");
                        }
                        if client_gone {
                            let slot_guard = slot_for_bg.lock().await;
                            if slot_guard.is_some() {
                                break; // Usage captured, safe to stop
                            }
                        }
                    } else {
                        drop(acc_guard);
                    }
                }
                Err(e) => {
                    let sse_error = format!(
                        "data: {{\"error\":{{\"message\":\"Bedrock stream error: {}\",\"type\":\"stream_error\"}}}}\n\n",
                        e.to_string().replace('"', "'")
                    );
                    let _ = tx.send(Ok(Bytes::from(sse_error))).await;

                    {
                        let mut slot_guard = slot_for_bg.lock().await;
                        if slot_guard.is_none() {
                            let mut acc_guard = accumulator.lock().await;
                            let partial =
                                std::mem::replace(&mut *acc_guard, StreamAccumulator::new());
                            *slot_guard = Some(partial.finish());
                        }
                    }
                    notify_bg.notify_waiters();

                    let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, e.to_string());
                    let _ = tx.send(Err(io_err)).await;
                    break;
                }
            }
        }

        // EOF: ensure result_slot is populated
        let mut slot_guard = slot_for_bg.lock().await;
        if slot_guard.is_none() {
            let mut acc_guard = accumulator.lock().await;
            let finished = std::mem::replace(&mut *acc_guard, StreamAccumulator::new());
            *slot_guard = Some(finished.finish());
        }
        notify_bg.notify_waiters();
    });

    let mapped = tokio_stream::wrappers::ReceiverStream::new(rx);
    let body = Body::from_stream(mapped);
    (body, result_slot, notify)
}
