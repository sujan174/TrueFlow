use serde_json::{json, Value};

use super::Provider;

pub(crate) fn translate_request(provider: Provider, body: &Value) -> Option<Value> {
    match provider {
        Provider::Anthropic => Some(openai_to_anthropic_request(body)),
        Provider::Gemini => Some(openai_to_gemini_request(body)),
        Provider::Bedrock => Some(openai_to_bedrock_request(body)),
        // OpenAI-compatible providers — no translation needed
        Provider::OpenAI
        | Provider::AzureOpenAI
        | Provider::Groq
        | Provider::Mistral
        | Provider::TogetherAI
        | Provider::Cohere
        | Provider::Ollama
        | Provider::Unknown => None,
    }
}

/// Translate a provider's native response body back to OpenAI format.
// ═══════════════════════════════════════════════════════════════
// OpenAI → Anthropic (Messages API)
// ═══════════════════════════════════════════════════════════════
pub(crate) fn openai_to_anthropic_request(body: &Value) -> Value {
    let mut result = serde_json::Map::new();

    // Model (required)
    if let Some(model) = body.get("model") {
        result.insert("model".into(), model.clone());
    }

    // Max tokens (required by Anthropic, default 4096)
    let max_tokens = body
        .get("max_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(4096);
    result.insert("max_tokens".into(), json!(max_tokens));

    // Messages: extract system message as top-level param
    if let Some(messages) = body.get("messages").and_then(|m| m.as_array()) {
        let mut system_parts = Vec::new();
        let mut user_messages = Vec::new();

        for msg in messages {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
            match role {
                "system" => {
                    if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                        system_parts.push(content.to_string());
                    }
                }
                "user" | "assistant" => {
                    let mut new_msg = serde_json::Map::new();
                    new_msg.insert("role".into(), json!(role));

                    // Handle content (string or array of content blocks)
                    if let Some(content) = msg.get("content") {
                        if content.is_string() {
                            new_msg.insert("content".into(), content.clone());
                        } else if content.is_array() {
                            // Convert OpenAI content parts to Anthropic format
                            let parts = content.as_array().unwrap();
                            let anthropic_parts: Vec<Value> = parts
                                .iter()
                                .map(|p| {
                                    let part_type =
                                        p.get("type").and_then(|t| t.as_str()).unwrap_or("text");
                                    match part_type {
                                        "text" => json!({
                                            "type": "text",
                                            "text": p.get("text").cloned().unwrap_or(json!(""))
                                        }),
                                        "image_url" => {
                                            let url = p
                                                .get("image_url")
                                                .and_then(|u| u.get("url"))
                                                .and_then(|u| u.as_str())
                                                .unwrap_or("");
                                            if url.starts_with("data:") {
                                                // Base64 data URI → Anthropic base64 source block
                                                let mime = url
                                                    .split_once(';')
                                                    .and_then(|(prefix, _)| {
                                                        prefix.strip_prefix("data:")
                                                    })
                                                    .unwrap_or("image/jpeg");
                                                let data = url
                                                    .split_once(',')
                                                    .map(|(_, d)| d)
                                                    .unwrap_or("");
                                                json!({
                                                    "type": "image",
                                                    "source": {
                                                        "type": "base64",
                                                        "media_type": mime,
                                                        "data": data
                                                    }
                                                })
                                            } else {
                                                // HTTP URL → Anthropic URL source block
                                                json!({
                                                    "type": "image",
                                                    "source": {
                                                        "type": "url",
                                                        "url": url
                                                    }
                                                })
                                            }
                                        }
                                        _ => p.clone(),
                                    }
                                })
                                .collect();
                            new_msg.insert("content".into(), json!(anthropic_parts));
                        }
                    }

                    user_messages.push(Value::Object(new_msg));
                }
                "tool" => {
                    // Tool results: OpenAI → Anthropic
                    user_messages.push(json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": msg.get("tool_call_id").cloned().unwrap_or(json!("")),
                            "content": msg.get("content").cloned().unwrap_or(json!(""))
                        }]
                    }));
                }
                _ => {
                    user_messages.push(msg.clone());
                }
            }
        }

        if !system_parts.is_empty() {
            result.insert("system".into(), json!(system_parts.join("\n")));
        }
        result.insert("messages".into(), json!(user_messages));
    }

    // Temperature
    if let Some(temp) = body.get("temperature") {
        result.insert("temperature".into(), temp.clone());
    }

    // Top P
    if let Some(top_p) = body.get("top_p") {
        result.insert("top_p".into(), top_p.clone());
    }

    // Stop sequences
    if let Some(stop) = body.get("stop") {
        if let Some(arr) = stop.as_array() {
            result.insert("stop_sequences".into(), json!(arr));
        } else if let Some(s) = stop.as_str() {
            result.insert("stop_sequences".into(), json!([s]));
        }
    }

    // Tools
    if let Some(tools) = body.get("tools").and_then(|t| t.as_array()) {
        let anthropic_tools: Vec<Value> = tools.iter().filter_map(|tool| {
            let func = tool.get("function")?;
            Some(json!({
                "name": func.get("name").cloned().unwrap_or(json!("")),
                "description": func.get("description").cloned().unwrap_or(json!("")),
                "input_schema": func.get("parameters").cloned().unwrap_or(json!({"type": "object"}))
            }))
        }).collect();
        if !anthropic_tools.is_empty() {
            result.insert("tools".into(), json!(anthropic_tools));
        }
    }

    // tool_choice: map OpenAI → Anthropic format
    // OpenAI: "auto" | "none" | "required" | {"type":"function","function":{"name":"X"}}
    // Anthropic: {"type":"auto"} | {"type":"any"} | {"type":"tool","name":"X"}
    if let Some(tc) = body.get("tool_choice") {
        match tc.as_str() {
            Some("auto") => {
                result.insert("tool_choice".into(), json!({"type": "auto"}));
            }
            Some("required") => {
                result.insert("tool_choice".into(), json!({"type": "any"}));
            }
            Some("none") => {
                // FIX H-3: Anthropic has no "none" tool_choice — omit tool_choice
                // AND remove tools array (already inserted above) to prevent the
                // model from attempting tool calls.
                result.remove("tools");
            }
            None if tc.is_object() => {
                // Specific function: forward as Anthropic "tool" type
                if let Some(name) = tc.get("function").and_then(|f| f.get("name")) {
                    result.insert("tool_choice".into(), json!({"type": "tool", "name": name}));
                }
            }
            _ => {}
        }
    }

    // Stream
    if let Some(stream) = body.get("stream") {
        result.insert("stream".into(), stream.clone());
    }

    // Top K (Anthropic native, no OpenAI equivalent — forward if present)
    if let Some(top_k) = body.get("top_k") {
        result.insert("top_k".into(), top_k.clone());
    }

    // Metadata (for user tracking)
    if let Some(metadata) = body.get("metadata") {
        result.insert("metadata".into(), metadata.clone());
    }

    Value::Object(result)
}

/// Translate an OpenAI content value (string or parts array) into Gemini `parts`.
/// Handles text, image_url (HTTP URLs → fileData, base64 data URIs → inlineData).
pub(crate) fn translate_content_to_gemini_parts(content: Option<&Value>) -> Vec<Value> {
    match content {
        Some(Value::String(s)) => vec![json!({"text": s})],
        Some(Value::Array(parts)) => parts
            .iter()
            .map(|p| {
                match p.get("type").and_then(|t| t.as_str()) {
                    Some("text") => json!({"text": p.get("text").cloned().unwrap_or(json!(""))}),
                    Some("image_url") => {
                        let url = p
                            .get("image_url")
                            .and_then(|u| u.get("url"))
                            .and_then(|u| u.as_str())
                            .unwrap_or("");
                        if url.starts_with("data:") {
                            // data:image/jpeg;base64,<data> → Gemini inlineData
                            let mime = url
                                .split_once(';')
                                .and_then(|(prefix, _)| prefix.strip_prefix("data:"))
                                .unwrap_or("image/jpeg");
                            let data = url.split_once(',').map(|(_, d)| d).unwrap_or("");
                            json!({"inlineData": {"mimeType": mime, "data": data}})
                        } else {
                            // HTTP URL → Gemini fileData
                            // Gemini requires MIME type; try to infer from URL extension
                            let mime = if url.ends_with(".png") {
                                "image/png"
                            } else if url.ends_with(".gif") {
                                "image/gif"
                            } else if url.ends_with(".webp") {
                                "image/webp"
                            } else {
                                "image/jpeg"
                            };
                            json!({"fileData": {"mimeType": mime, "fileUri": url}})
                        }
                    }
                    _ => p.clone(),
                }
            })
            .collect(),
        Some(Value::Null) | None => vec![json!({"text": ""})],
        // Fallback: not a known content type, skip
        _ => vec![],
    }
}

pub(crate) fn openai_to_gemini_request(body: &Value) -> Value {
    let mut result = serde_json::Map::new();

    // Messages → contents (with full multimodal support)
    if let Some(messages) = body.get("messages").and_then(|m| m.as_array()) {
        let mut contents = Vec::new();
        // FIX 4D-1: Collect ALL system messages instead of keeping only the last.
        // Previously `system_instruction = Some(...)` overwrote on each system message,
        // silently dropping earlier ones (e.g. security guardrails in the first message).
        let mut system_texts: Vec<String> = Vec::new();

        for msg in messages {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");

            match role {
                "system" => {
                    // Gemini system instruction — always text; collect all
                    let text = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
                    if !text.is_empty() {
                        system_texts.push(text.to_string());
                    }
                }
                "user" => {
                    let parts = translate_content_to_gemini_parts(msg.get("content"));
                    if !parts.is_empty() {
                        contents.push(json!({ "role": "user", "parts": parts }));
                    }
                }
                "assistant" => {
                    let parts = translate_content_to_gemini_parts(msg.get("content"));
                    if !parts.is_empty() {
                        contents.push(json!({ "role": "model", "parts": parts }));
                    }
                }
                "tool" => {
                    // Function result → Gemini functionResponse
                    // Gemini requires the function NAME, not the tool_call_id.
                    // Look up the function name from the tool_call_id by scanning
                    // preceding assistant messages for matching tool_calls.
                    let tool_call_id = msg
                        .get("tool_call_id")
                        .and_then(|t| t.as_str())
                        .unwrap_or("unknown");
                    let func_name = messages
                        .iter()
                        .filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("assistant"))
                        .filter_map(|m| m.get("tool_calls").and_then(|tc| tc.as_array()))
                        .flatten()
                        .find(|tc| tc.get("id").and_then(|id| id.as_str()) == Some(tool_call_id))
                        .and_then(|tc| {
                            tc.get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(|n| n.as_str())
                        });

                    // FIX: Log a warning if function name lookup fails instead of silently
                    // using tool_call_id (which would cause Gemini API errors)
                    let func_name = match func_name {
                        Some(name) => name.to_string(),
                        None => {
                            tracing::warn!(
                                tool_call_id = %tool_call_id,
                                "Could not find function name for tool_call_id in Gemini translation - \
                                 this may cause API errors if the tool call history is incomplete"
                            );
                            // Use a placeholder that indicates the issue
                            format!("unknown_tool_for_{}", tool_call_id)
                        }
                    };

                    let content_val = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
                    contents.push(json!({
                        "role": "user",
                        "parts": [{
                            "functionResponse": {
                                "name": func_name,
                                "response": { "result": content_val }
                            }
                        }]
                    }));
                }
                _ => {}
            }
        }

        result.insert("contents".into(), json!(contents));
        if !system_texts.is_empty() {
            let joined = system_texts.join("\n");
            result.insert(
                "systemInstruction".into(),
                json!({
                    "parts": [{"text": joined}]
                }),
            );
        }
    }

    // Generation config (temperature, max tokens, top_p, stop sequences)
    let mut gen_config = serde_json::Map::new();
    if let Some(temp) = body.get("temperature") {
        gen_config.insert("temperature".into(), temp.clone());
    }
    if let Some(max_tokens) = body.get("max_tokens") {
        gen_config.insert("maxOutputTokens".into(), max_tokens.clone());
    }
    if let Some(top_p) = body.get("top_p") {
        gen_config.insert("topP".into(), top_p.clone());
    }
    if let Some(stop) = body.get("stop") {
        if let Some(arr) = stop.as_array() {
            gen_config.insert("stopSequences".into(), json!(arr));
        } else if let Some(s) = stop.as_str() {
            gen_config.insert("stopSequences".into(), json!([s]));
        }
    }

    // response_format → Gemini responseMimeType + responseSchema
    // OpenAI: {"type":"json_object"} | {"type":"json_schema","json_schema":{"schema":{...}}}
    if let Some(rf) = body.get("response_format") {
        match rf.get("type").and_then(|t| t.as_str()) {
            Some("json_object") => {
                gen_config.insert("responseMimeType".into(), json!("application/json"));
            }
            Some("json_schema") => {
                gen_config.insert("responseMimeType".into(), json!("application/json"));
                // json_schema.schema contains the JSON Schema object
                if let Some(schema) = rf.get("json_schema").and_then(|s| s.get("schema")) {
                    gen_config.insert("responseSchema".into(), schema.clone());
                }
            }
            _ => {}
        }
    }

    if !gen_config.is_empty() {
        result.insert("generationConfig".into(), Value::Object(gen_config));
    }

    // Tools (OpenAI functions → Gemini functionDeclarations)
    if let Some(tools) = body.get("tools").and_then(|t| t.as_array()) {
        let function_declarations: Vec<Value> = tools.iter().filter_map(|tool| {
            let func = tool.get("function")?;
            Some(json!({
                "name": func.get("name").cloned().unwrap_or(json!("")),
                "description": func.get("description").cloned().unwrap_or(json!("")),
                "parameters": func.get("parameters").cloned().unwrap_or(json!({"type": "object"}))
            }))
        }).collect();
        if !function_declarations.is_empty() {
            result.insert(
                "tools".into(),
                json!([{
                    "functionDeclarations": function_declarations
                }]),
            );
        }
    }

    // tool_choice → Gemini toolConfig.functionCallingConfig
    // OpenAI: "auto" | "none" | "required" | {"type":"function","function":{"name":"X"}}
    // Gemini mode: AUTO | NONE | ANY | specific function via allowedFunctionNames
    if let Some(tc) = body.get("tool_choice") {
        let (mode, allowed_names): (&str, Vec<&str>) = match tc.as_str() {
            Some("auto") => ("AUTO", vec![]),
            Some("none") => ("NONE", vec![]),
            Some("required") => ("ANY", vec![]),
            _ => {
                // Specific function: {"type":"function","function":{"name":"X"}}
                if let Some(name) = tc
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                {
                    ("ANY", vec![name])
                } else {
                    ("AUTO", vec![])
                }
            }
        };
        let mut fc_config = json!({"mode": mode});
        if !allowed_names.is_empty() {
            fc_config["allowedFunctionNames"] = json!(allowed_names);
        }
        result.insert(
            "toolConfig".into(),
            json!({"functionCallingConfig": fc_config}),
        );
    }

    Value::Object(result)
}

pub(crate) fn openai_to_bedrock_request(body: &Value) -> Value {
    let mut result = serde_json::Map::new();

    // Model — Bedrock uses modelId in the URL, but we include it for completeness
    if let Some(model) = body.get("model") {
        result.insert("modelId".into(), model.clone());
    }

    // Messages: extract system messages as top-level `system` array
    if let Some(messages) = body.get("messages").and_then(|m| m.as_array()) {
        let mut system_blocks = Vec::new();
        let mut bedrock_messages = Vec::new();

        for msg in messages {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
            match role {
                "system" => {
                    if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                        system_blocks.push(json!({"text": content}));
                    }
                }
                "user" | "assistant" => {
                    let mut content_blocks =
                        translate_openai_content_to_bedrock(msg.get("content"));

                    // FIX: Translate OpenAI tool_calls in assistant messages to Bedrock
                    // toolUse content blocks. Without this, multi-turn tool calling
                    // conversations lose the tool invocations from history.
                    if role == "assistant" {
                        if let Some(tool_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array())
                        {
                            for tc in tool_calls {
                                let func = tc.get("function");
                                let name = func
                                    .and_then(|f| f.get("name"))
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("");
                                let tool_id = tc.get("id").and_then(|id| id.as_str()).unwrap_or("");
                                let input: Value = func
                                    .and_then(|f| f.get("arguments"))
                                    .and_then(|a| a.as_str())
                                    .and_then(|s| serde_json::from_str(s).ok())
                                    .unwrap_or(json!({}));
                                content_blocks.push(json!({
                                    "toolUse": {
                                        "toolUseId": tool_id,
                                        "name": name,
                                        "input": input
                                    }
                                }));
                            }
                        }
                    }

                    if !content_blocks.is_empty() {
                        bedrock_messages.push(json!({
                            "role": role,
                            "content": content_blocks
                        }));
                    }
                }
                "tool" => {
                    // Tool result: OpenAI → Bedrock toolResult
                    let tool_use_id = msg
                        .get("tool_call_id")
                        .and_then(|t| t.as_str())
                        .unwrap_or("");

                    // FIX: Handle different content types for Bedrock tool results
                    let bedrock_content = match msg.get("content") {
                        Some(Value::String(s)) => {
                            vec![json!({"text": s})]
                        }
                        Some(Value::Array(arr)) => {
                            // Handle array content - convert each part
                            arr.iter()
                                .filter_map(|part| {
                                    if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                        Some(json!({"text": text}))
                                    } else if let Some(image) = part.get("image_url") {
                                        // Image in tool result - convert to Bedrock format
                                        let url = image.get("url")
                                            .and_then(|u| u.as_str())
                                            .unwrap_or("");
                                        if url.starts_with("data:") {
                                            let mime = url.split_once(';')
                                                .and_then(|(prefix, _)| prefix.strip_prefix("data:"))
                                                .unwrap_or("image/jpeg");
                                            let data = url.split_once(',').map(|(_, d)| d).unwrap_or("");
                                            Some(json!({
                                                "image": {
                                                    "format": mime.rsplit_once('/').map(|(_, s)| s).unwrap_or("jpeg"),
                                                    "source": {"bytes": data}
                                                }
                                            }))
                                        } else {
                                            // Can't handle HTTP URLs in tool results - skip
                                            None
                                        }
                                    } else {
                                        // Other content types - stringify
                                        Some(json!({"text": serde_json::to_string(part).unwrap_or_default()}))
                                    }
                                })
                                .collect()
                        }
                        Some(other) => {
                            // Non-string, non-array content - stringify it
                            vec![json!({"text": serde_json::to_string(other).unwrap_or_default()})]
                        }
                        None => {
                            vec![json!({"text": ""})]
                        }
                    };

                    bedrock_messages.push(json!({
                        "role": "user",
                        "content": [{
                            "toolResult": {
                                "toolUseId": tool_use_id,
                                "content": bedrock_content
                            }
                        }]
                    }));
                }
                _ => {}
            }
        }

        if !system_blocks.is_empty() {
            result.insert("system".into(), json!(system_blocks));
        }
        result.insert("messages".into(), json!(bedrock_messages));
    }

    // Inference config (temperature, max_tokens, top_p, stop sequences)
    let mut inference_config = serde_json::Map::new();
    if let Some(temp) = body.get("temperature") {
        inference_config.insert("temperature".into(), temp.clone());
    }
    if let Some(max_tokens) = body.get("max_tokens") {
        inference_config.insert("maxTokens".into(), max_tokens.clone());
    }
    if let Some(top_p) = body.get("top_p") {
        inference_config.insert("topP".into(), top_p.clone());
    }
    if let Some(stop) = body.get("stop") {
        if let Some(arr) = stop.as_array() {
            inference_config.insert("stopSequences".into(), json!(arr));
        } else if let Some(s) = stop.as_str() {
            inference_config.insert("stopSequences".into(), json!([s]));
        }
    }
    if !inference_config.is_empty() {
        result.insert("inferenceConfig".into(), Value::Object(inference_config));
    }

    // Tools → Bedrock toolConfig
    if let Some(tools) = body.get("tools").and_then(|t| t.as_array()) {
        let bedrock_tools: Vec<Value> = tools.iter().filter_map(|tool| {
            let func = tool.get("function")?;
            Some(json!({
                "toolSpec": {
                    "name": func.get("name").cloned().unwrap_or(json!("")),
                    "description": func.get("description").cloned().unwrap_or(json!("")),
                    "inputSchema": {
                        "json": func.get("parameters").cloned().unwrap_or(json!({"type": "object"}))
                    }
                }
            }))
        }).collect();
        if !bedrock_tools.is_empty() {
            result.insert("toolConfig".into(), json!({"tools": bedrock_tools}));
        }
    }

    Value::Object(result)
}

/// Translate OpenAI content (string or multipart array) to Bedrock content blocks.
pub(crate) fn translate_openai_content_to_bedrock(content: Option<&Value>) -> Vec<Value> {
    match content {
        Some(Value::String(s)) => vec![json!({"text": s})],
        Some(Value::Array(parts)) => parts.iter().map(|p| {
            match p.get("type").and_then(|t| t.as_str()) {
                Some("text") => json!({"text": p.get("text").and_then(|t| t.as_str()).unwrap_or("")}),
                Some("image_url") => {
                    let url = p.get("image_url")
                        .and_then(|u| u.get("url"))
                        .and_then(|u| u.as_str())
                        .unwrap_or("");
                    if url.starts_with("data:") {
                        // Base64 data URI → Bedrock image block
                        let mime = url.split_once(';')
                            .and_then(|(prefix, _)| prefix.strip_prefix("data:"))
                            .unwrap_or("image/jpeg");
                        let data = url.split_once(',').map(|(_, d)| d).unwrap_or("");
                        json!({
                            "image": {
                                "format": mime.rsplit_once('/').map(|(_, s)| s).unwrap_or("jpeg"),
                                "source": {"bytes": data}
                            }
                        })
                    } else {
                        // FIX: Bedrock doesn't support HTTP URL references directly.
                        // Don't expose the URL in text (could leak sensitive info).
                        // Log a warning and use a generic placeholder.
                        tracing::warn!(
                            url = %url,
                            "Bedrock doesn't support HTTP URL image references - \
                             image will not be included in the request"
                        );
                        json!({"text": "[Image: external URL not supported by Bedrock]"})
                    }
                }
                _ => p.clone(),
            }
        }).collect(),
        _ => vec![],
    }
}
