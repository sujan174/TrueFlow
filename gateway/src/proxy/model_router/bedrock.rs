use serde_json::{json, Value};

use super::streaming::openai_sse_chunk;

pub(crate) fn parse_event_stream_header(data: &[u8]) -> Option<(String, String, usize)> {
    if data.is_empty() {
        return None;
    }
    let name_len = data[0] as usize;
    if data.len() < 1 + name_len + 1 {
        return None;
    }
    let name = std::str::from_utf8(&data[1..1 + name_len]).ok()?.to_string();
    let value_type = data[1 + name_len];
    let rest = &data[1 + name_len + 1..];

    match value_type {
        // Type 7 = String
        7 => {
            if rest.len() < 2 {
                return None;
            }
            let val_len = u16::from_be_bytes([rest[0], rest[1]]) as usize;
            if rest.len() < 2 + val_len {
                return None;
            }
            let value = std::str::from_utf8(&rest[2..2 + val_len]).ok()?.to_string();
            Some((name, value, 1 + name_len + 1 + 2 + val_len))
        }
        // Other types: skip based on known sizes
        0 => Some((name, "true".to_string(), 1 + name_len + 1 + 1)),  // bool true
        1 => Some((name, "false".to_string(), 1 + name_len + 1 + 1)), // bool false
        2 => Some((name, "".to_string(), 1 + name_len + 1 + 1)),      // byte
        3 => Some((name, "".to_string(), 1 + name_len + 1 + 2)),      // short
        4 => Some((name, "".to_string(), 1 + name_len + 1 + 4)),      // int
        5 => Some((name, "".to_string(), 1 + name_len + 1 + 8)),      // long
        6 => {  // bytes
            if rest.len() < 2 {
                return None;
            }
            let val_len = u16::from_be_bytes([rest[0], rest[1]]) as usize;
            Some((name, "".to_string(), 1 + name_len + 1 + 2 + val_len))
        }
        8 => Some((name, "".to_string(), 1 + name_len + 1 + 8)),      // timestamp
        9 => Some((name, "".to_string(), 1 + name_len + 1 + 16)),     // uuid
        _ => None, // Unknown type
    }
}

/// Decode a Bedrock binary event stream into JSON events with their event types.
/// Returns Vec<(event_type, payload_json)>.
pub(crate) fn decode_bedrock_event_stream(data: &[u8]) -> Vec<(String, Value)> {
    let mut events = Vec::new();
    let mut offset = 0;

    while offset + 12 <= data.len() {
        // Prelude: 4B total_length + 4B headers_length + 4B prelude_CRC
        let total_length = u32::from_be_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
        ]) as usize;
        let headers_length = u32::from_be_bytes([
            data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7]
        ]) as usize;
        // FIX(C2): Validate prelude CRC32 — reject corrupt frames
        let prelude_crc_expected = u32::from_be_bytes([
            data[offset + 8], data[offset + 9], data[offset + 10], data[offset + 11]
        ]);
        let prelude_crc_actual = crc32_checksum(&data[offset..offset + 8]);
        if prelude_crc_expected != prelude_crc_actual {
            tracing::warn!(
                expected = prelude_crc_expected,
                actual = prelude_crc_actual,
                "Bedrock event stream: prelude CRC mismatch, skipping frame"
            );
            break; // Can't trust total_length if prelude is corrupt
        }

        if offset + total_length > data.len() || total_length < 16 {
            break; // Incomplete or invalid message
        }

        // FIX(C2): Validate full message CRC32
        let msg_crc_offset = offset + total_length - 4;
        let msg_crc_expected = u32::from_be_bytes([
            data[msg_crc_offset], data[msg_crc_offset + 1],
            data[msg_crc_offset + 2], data[msg_crc_offset + 3]
        ]);
        let msg_crc_actual = crc32_checksum(&data[offset..msg_crc_offset]);
        if msg_crc_expected != msg_crc_actual {
            tracing::warn!(
                expected = msg_crc_expected,
                actual = msg_crc_actual,
                "Bedrock event stream: message CRC mismatch, skipping frame"
            );
            offset += total_length;
            continue; // Skip this frame but try the next
        }

        // Headers start at offset + 12
        let headers_start = offset + 12;
        let headers_end = headers_start + headers_length;

        // Parse headers
        let mut event_type = String::new();
        let mut message_type = String::new();
        let mut h_offset = headers_start;
        while h_offset < headers_end {
            if let Some((name, value, consumed)) = parse_event_stream_header(&data[h_offset..]) {
                if name == ":event-type" {
                    event_type = value;
                } else if name == ":message-type" {
                    message_type = value;
                }
                h_offset += consumed;
            } else {
                break;
            }
        }

        // Payload: between headers_end and (offset + total_length - 4) for message CRC
        let payload_end = offset + total_length - 4; // exclude trailing 4B message CRC
        if headers_end <= payload_end {
            let payload_bytes = &data[headers_end..payload_end];
            if !payload_bytes.is_empty() {
                if let Ok(json) = serde_json::from_slice::<Value>(payload_bytes) {
                    if message_type == "exception" {
                        // FIX: Surface Bedrock stream exceptions (internalServerException,
                        // throttlingException, modelStreamErrorException, etc.) instead
                        // of silently dropping them. The event_type from the headers
                        // contains the exception class (e.g. "throttlingException").
                        tracing::warn!(
                            event_type = %event_type,
                            payload = %json,
                            "Bedrock stream exception received"
                        );
                    }
                    events.push((event_type.clone(), json));
                }
            }
        }

        offset += total_length;
    }

    events
}

/// Translate decoded Bedrock event stream events into OpenAI SSE format.
/// This works on the raw binary bytes of the event stream.
#[allow(dead_code)]
pub(crate) fn translate_bedrock_event_stream_to_openai(body: &[u8], model: &str) -> Vec<u8> {
    let events = decode_bedrock_event_stream(body);
    let chunk_id = format!("chatcmpl-{}", uuid::Uuid::new_v4().simple());
    let mut output = String::new();

    for (event_type, payload) in &events {
        match event_type.as_str() {
            "messageStart" => {
                // Emit role chunk
                let role = payload.get("role")
                    .and_then(|r| r.as_str())
                    .unwrap_or("assistant");
                output.push_str(&openai_sse_chunk(
                    &chunk_id, model,
                    json!({"role": role, "content": ""}),
                    None,
                ));
            }
            "contentBlockStart" => {
                // Tool use start
                if let Some(start) = payload.get("start") {
                    if let Some(tool_use) = start.get("toolUse") {
                        let index = payload.get("contentBlockIndex")
                            .and_then(|i| i.as_u64())
                            .unwrap_or(0);
                        let name = tool_use.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let tool_id = tool_use.get("toolUseId").and_then(|id| id.as_str()).unwrap_or("");
                        output.push_str(&openai_sse_chunk(
                            &chunk_id, model,
                            json!({"tool_calls": [{
                                "index": index,
                                "id": tool_id,
                                "type": "function",
                                "function": {"name": name, "arguments": ""}
                            }]}),
                            None,
                        ));
                    }
                }
            }
            "contentBlockDelta" => {
                if let Some(delta) = payload.get("delta") {
                    // Text delta
                    if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                        output.push_str(&openai_sse_chunk(
                            &chunk_id, model,
                            json!({"content": text}),
                            None,
                        ));
                    }
                    // Tool input delta
                    if let Some(input) = delta.get("toolUse").and_then(|tu| tu.get("input")) {
                        let input_str = if input.is_string() {
                            input.as_str().unwrap_or("").to_string()
                        } else {
                            serde_json::to_string(input).unwrap_or_default()
                        };
                        let index = payload.get("contentBlockIndex")
                            .and_then(|i| i.as_u64())
                            .unwrap_or(0);
                        output.push_str(&openai_sse_chunk(
                            &chunk_id, model,
                            json!({"tool_calls": [{
                                "index": index,
                                "function": {"arguments": input_str}
                            }]}),
                            None,
                        ));
                    }
                }
            }
            "messageStop" => {
                let finish = match payload.get("stopReason").and_then(|s| s.as_str()) {
                    Some("end_turn") => "stop",
                    Some("tool_use") => "tool_calls",
                    Some("max_tokens") => "length",
                    Some("stop_sequence") => "stop",
                    Some("content_filtered") => "content_filter",
                    _ => "stop",
                };
                output.push_str(&openai_sse_chunk(
                    &chunk_id, model,
                    json!({}),
                    Some(finish),
                ));
            }
            "metadata" => {
                // Usage info — emit as a usage chunk (OpenAI style)
                // We don't emit this as a separate SSE event since usage
                // is typically included in the final chunk
            }
            // FIX: Surface Bedrock stream exceptions as SSE error events.
            // These arrive when the provider encounters errors mid-stream
            // (after the 200 OK was already sent).
            "internalServerException" | "modelStreamErrorException"
            | "throttlingException" | "validationException"
            | "modelTimeoutException" | "serviceUnavailableException" => {
                let message = payload.get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown stream error");
                let error_event = format!(
                    "data: {{\"error\":{{\"message\":\"{}\",\"type\":\"{}\",\"code\":\"{}\"}}}}\n\n",
                    message.replace('"', "'"),
                    "stream_error",
                    event_type,
                );
                output.push_str(&error_event);
            }
            _ => {}
        }
    }

    // Ensure [DONE] marker
    if !output.ends_with("data: [DONE]\n\n") {
        output.push_str("data: [DONE]\n\n");
    }

    output.into_bytes()
}

/// Build a test Bedrock binary event stream message for unit testing.
/// Constructs a valid binary frame with correct CRC32 checksums.
#[cfg(test)]
pub(crate) fn build_test_bedrock_event(event_type: &str, payload: Value) -> Vec<u8> {
    let payload_bytes = serde_json::to_vec(&payload).unwrap();

    // Build headers: :event-type (string, type=7) + :content-type (string, type=7)
    // + :message-type (string, type=7)
    let mut headers = Vec::new();

    // :event-type header
    let et_name = b":event-type";
    let et_value = event_type.as_bytes();
    headers.push(et_name.len() as u8);
    headers.extend_from_slice(et_name);
    headers.push(7u8); // string type
    headers.extend_from_slice(&(et_value.len() as u16).to_be_bytes());
    headers.extend_from_slice(et_value);

    // :content-type header
    let ct_name = b":content-type";
    let ct_value = b"application/json";
    headers.push(ct_name.len() as u8);
    headers.extend_from_slice(ct_name);
    headers.push(7u8);
    headers.extend_from_slice(&(ct_value.len() as u16).to_be_bytes());
    headers.extend_from_slice(ct_value);

    // :message-type header
    let mt_name = b":message-type";
    let mt_value = b"event";
    headers.push(mt_name.len() as u8);
    headers.extend_from_slice(mt_name);
    headers.push(7u8);
    headers.extend_from_slice(&(mt_value.len() as u16).to_be_bytes());
    headers.extend_from_slice(mt_value);

    let headers_length = headers.len() as u32;
    // total = 4 (total_len) + 4 (headers_len) + 4 (prelude_crc) + headers + payload + 4 (msg_crc)
    let total_length = 4 + 4 + 4 + headers.len() + payload_bytes.len() + 4;

    let mut message = Vec::with_capacity(total_length);

    // Prelude: total_length + headers_length
    message.extend_from_slice(&(total_length as u32).to_be_bytes());
    message.extend_from_slice(&headers_length.to_be_bytes());

    // Prelude CRC32
    let prelude_crc = crc32_checksum(&message[0..8]);
    message.extend_from_slice(&prelude_crc.to_be_bytes());

    // Headers + Payload
    message.extend_from_slice(&headers);
    message.extend_from_slice(&payload_bytes);

    // Message CRC32 (over everything so far)
    let msg_crc = crc32_checksum(&message);
    message.extend_from_slice(&msg_crc.to_be_bytes());

    message
}

/// CRC32 checksum (CRC-32/ISO-HDLC, same as zlib/gzip CRC32).
/// Used for Bedrock event stream frame verification in both production and tests.
pub(crate) fn crc32_checksum(data: &[u8]) -> u32 {
    // Simple CRC32 implementation (CRC-32/ISO-HDLC)
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

// ── Tests ───────────────────────────────────────────────────────


