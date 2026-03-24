use axum::http::{HeaderMap, Method, Uri};
use serde_json::Value;
use std::collections::HashMap;

/// All contextual data available for field resolution during policy evaluation.
#[derive(Debug)]
pub struct RequestContext<'a> {
    // ── Request data ──
    pub method: &'a Method,
    pub path: &'a str,
    pub uri: &'a Uri,
    pub headers: &'a HeaderMap,
    pub body: Option<&'a Value>,
    pub body_size: usize,

    // ── Identity ──
    pub agent_name: Option<&'a str>,
    pub token_id: &'a str,
    pub token_name: &'a str,
    pub project_id: &'a str,
    pub client_ip: Option<&'a str>,
    pub token_purpose: &'a str,

    // ── Response data (Phase 2 / post-flight only) ──
    pub response_status: Option<u16>,
    pub response_body: Option<&'a Value>,
    pub response_headers: Option<&'a HeaderMap>,

    // ── Usage counters (resolved lazily) ──
    pub usage: HashMap<String, f64>,
}

/// Resolve a dot-notation field path to a JSON value from the request context.
///
/// Supported prefixes:
/// - `request.method`, `request.path`, `request.body_size`
/// - `request.body.<json_path>` (dot-notation into JSON body)
/// - `request.headers.<header_name>`
/// - `request.query.<param_name>`
/// - `response.status`, `response.body.<json_path>`, `response.headers.<name>`
/// - `agent.name`
/// - `token.id`, `token.name`, `token.project_id`
/// - `context.ip`
/// - `context.time.hour`, `context.time.weekday`, `context.time.date`
/// - `usage.<counter_name>`
pub fn resolve_field(field: &str, ctx: &RequestContext<'_>) -> Option<Value> {
    let (prefix, rest) = field.split_once('.')?;

    match prefix {
        "request" => resolve_request(rest, ctx),
        "response" => resolve_response(rest, ctx),
        "agent" => resolve_agent(rest, ctx),
        "token" => resolve_token(rest, ctx),
        "context" => resolve_context(rest, ctx),
        "usage" => resolve_usage(rest, ctx),
        _ => None,
    }
}

fn resolve_request(path: &str, ctx: &RequestContext<'_>) -> Option<Value> {
    // Direct fields
    match path {
        "method" => return Some(Value::String(ctx.method.to_string())),
        "path" => return Some(Value::String(ctx.path.to_string())),
        "body_size" => return Some(Value::Number(ctx.body_size.into())),
        _ => {}
    }

    // request.body.<json_path>
    if let Some(json_path) = path.strip_prefix("body.") {
        return ctx.body.and_then(|b| extract_json_path(b, json_path));
    }
    if path == "body" {
        return ctx.body.cloned();
    }

    // request.headers.<name>
    if let Some(header_name) = path.strip_prefix("headers.") {
        return ctx
            .headers
            .get(header_name)
            .and_then(|v| v.to_str().ok())
            .map(|s| Value::String(s.to_string()));
    }

    // request.query.<name>
    if let Some(param_name) = path.strip_prefix("query.") {
        return ctx.uri.query().and_then(|qs| {
            url_params(qs)
                .into_iter()
                .find(|(k, _)| k == param_name)
                .map(|(_, v)| Value::String(v))
        });
    }

    None
}

fn resolve_response(path: &str, ctx: &RequestContext<'_>) -> Option<Value> {
    if path == "status" {
        return ctx.response_status.map(|s| Value::Number(s.into()));
    }

    if let Some(json_path) = path.strip_prefix("body.") {
        return ctx
            .response_body
            .and_then(|b| extract_json_path(b, json_path));
    }

    if let Some(header_name) = path.strip_prefix("headers.") {
        return ctx
            .response_headers
            .and_then(|h| h.get(header_name))
            .and_then(|v| v.to_str().ok())
            .map(|s| Value::String(s.to_string()));
    }

    None
}

fn resolve_agent(path: &str, ctx: &RequestContext<'_>) -> Option<Value> {
    match path {
        "name" => ctx.agent_name.map(|s| Value::String(s.to_string())),
        _ => None,
    }
}

fn resolve_token(path: &str, ctx: &RequestContext<'_>) -> Option<Value> {
    match path {
        "id" => Some(Value::String(ctx.token_id.to_string())),
        "name" => Some(Value::String(ctx.token_name.to_string())),
        "project_id" => Some(Value::String(ctx.project_id.to_string())),
        "purpose" => Some(Value::String(ctx.token_purpose.to_string())),
        _ => None,
    }
}

fn resolve_context(path: &str, ctx: &RequestContext<'_>) -> Option<Value> {
    match path {
        "ip" => ctx.client_ip.map(|s| Value::String(s.to_string())),
        "time.hour" => {
            let now = chrono::Utc::now();
            Some(Value::Number(chrono::Timelike::hour(&now).into()))
        }
        "time.weekday" => {
            let now = chrono::Utc::now();
            let day = chrono::Datelike::weekday(&now);
            let name = match day {
                chrono::Weekday::Mon => "mon",
                chrono::Weekday::Tue => "tue",
                chrono::Weekday::Wed => "wed",
                chrono::Weekday::Thu => "thu",
                chrono::Weekday::Fri => "fri",
                chrono::Weekday::Sat => "sat",
                chrono::Weekday::Sun => "sun",
            };
            Some(Value::String(name.to_string()))
        }
        "time.date" => {
            let now = chrono::Utc::now();
            Some(Value::String(now.format("%Y-%m-%d").to_string()))
        }
        _ => None,
    }
}

fn resolve_usage(path: &str, ctx: &RequestContext<'_>) -> Option<Value> {
    ctx.usage
        .get(path)
        .and_then(|v| serde_json::Number::from_f64(*v))
        .map(Value::Number)
}

// ── JSON Path Extraction ─────────────────────────────────────

/// Simple dot-notation JSON path extractor.
///
/// Supports:
/// - `amount` → `obj["amount"]`
/// - `user.name` → `obj["user"]["name"]`
/// - `messages[0].content` → `obj["messages"][0]["content"]`
/// - `messages[*].content` → collects all `obj["messages"][i]["content"]` into an array
fn extract_json_path(value: &Value, path: &str) -> Option<Value> {
    let segments: Vec<&str> = path.split('.').collect();
    extract_segments(value, &segments)
}

fn extract_segments(value: &Value, segments: &[&str]) -> Option<Value> {
    if segments.is_empty() {
        return Some(value.clone());
    }

    let seg = segments[0];
    let rest = &segments[1..];

    // Handle array indexing: field[0] or field[*]
    if let Some((field, idx)) = parse_array_access(seg) {
        let arr_value = if field.is_empty() {
            value.clone()
        } else {
            value.get(&field)?.clone()
        };

        let arr = arr_value.as_array()?;

        if idx == "*" {
            // Wildcard: collect matching values from all elements
            let results: Vec<Value> = arr
                .iter()
                .filter_map(|elem| extract_segments(elem, rest))
                .collect();
            if results.is_empty() {
                return None;
            }
            return Some(Value::Array(results));
        }

        // Numeric index
        let i: usize = idx.parse().ok()?;
        let elem = arr.get(i)?;
        return extract_segments(elem, rest);
    }

    // Simple field access
    let child = value.get(seg)?;
    extract_segments(child, rest)
}

/// Parse `"field[0]"` into `("field", "0")` or `"field[*]"` into `("field", "*")`.
fn parse_array_access(seg: &str) -> Option<(String, String)> {
    let bracket_start = seg.find('[')?;
    let bracket_end = seg.find(']')?;
    let field = seg[..bracket_start].to_string();
    let index = seg[bracket_start + 1..bracket_end].to_string();
    Some((field, index))
}

/// Parse URL query string into key-value pairs.
fn url_params(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?.to_string();
            let value = parts.next().unwrap_or("").to_string();
            Some((key, value))
        })
        .collect()
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Helper: create a simple RequestContext for testing.
    fn make_ctx<'a>(
        method: &'a Method,
        path: &'a str,
        uri: &'a Uri,
        headers: &'a HeaderMap,
        body: Option<&'a Value>,
    ) -> RequestContext<'a> {
        RequestContext {
            method,
            path,
            uri,
            headers,
            body,
            body_size: body.map(|b| b.to_string().len()).unwrap_or(0),
            agent_name: Some("test-agent"),
            token_id: "tok_abc123",
            token_name: "My Token",
            project_id: "proj_xyz",
            client_ip: Some("10.0.0.42"),
            token_purpose: "llm",
            response_status: None,
            response_body: None,
            response_headers: None,
            usage: HashMap::new(),
        }
    }

    // ── JSON Path Extraction Tests ───────────────────────────

    #[test]
    fn test_extract_simple_field() {
        let json: Value = json!({"amount": 5000, "model": "gpt-4"});
        assert_eq!(
            extract_json_path(&json, "amount"),
            Some(Value::Number(5000.into()))
        );
        assert_eq!(
            extract_json_path(&json, "model"),
            Some(Value::String("gpt-4".to_string()))
        );
    }

    #[test]
    fn test_extract_nested_field() {
        let json: Value = json!({"user": {"name": "alice", "role": "admin"}});
        assert_eq!(
            extract_json_path(&json, "user.name"),
            Some(Value::String("alice".to_string()))
        );
    }

    #[test]
    fn test_extract_deeply_nested() {
        let json: Value = json!({"a": {"b": {"c": {"d": 42}}}});
        assert_eq!(extract_json_path(&json, "a.b.c.d"), Some(json!(42)));
    }

    #[test]
    fn test_extract_array_index() {
        let json: Value = json!({
            "messages": [
                {"role": "system", "content": "You are helpful"},
                {"role": "user", "content": "Hello world"}
            ]
        });
        assert_eq!(
            extract_json_path(&json, "messages[1].content"),
            Some(Value::String("Hello world".to_string()))
        );
    }

    #[test]
    fn test_extract_array_first_element() {
        let json: Value = json!({"items": [10, 20, 30]});
        assert_eq!(extract_json_path(&json, "items[0]"), Some(json!(10)));
    }

    #[test]
    fn test_extract_array_out_of_bounds() {
        let json: Value = json!({"items": [10, 20]});
        assert_eq!(extract_json_path(&json, "items[5]"), None);
    }

    #[test]
    fn test_extract_array_wildcard() {
        let json: Value = json!({
            "messages": [
                {"role": "system", "content": "Be helpful"},
                {"role": "user", "content": "Tell me a secret"}
            ]
        });
        let result = extract_json_path(&json, "messages[*].content").unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], "Be helpful");
        assert_eq!(arr[1], "Tell me a secret");
    }

    #[test]
    fn test_extract_wildcard_with_nested_objects() {
        let json: Value = json!({
            "tools": [
                {"function": {"name": "get_weather"}},
                {"function": {"name": "search_web"}}
            ]
        });
        let result = extract_json_path(&json, "tools[*].function.name").unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr, &[json!("get_weather"), json!("search_web")]);
    }

    #[test]
    fn test_extract_missing_field() {
        let json: Value = json!({"amount": 100});
        assert_eq!(extract_json_path(&json, "missing"), None);
        assert_eq!(extract_json_path(&json, "nested.missing"), None);
    }

    #[test]
    fn test_extract_boolean_and_null() {
        let json: Value = json!({"active": true, "deleted": null});
        assert_eq!(extract_json_path(&json, "active"), Some(json!(true)));
        assert_eq!(extract_json_path(&json, "deleted"), Some(json!(null)));
    }

    // ── URL Param Tests ──────────────────────────────────────

    #[test]
    fn test_url_params() {
        let params = url_params("model=gpt-4&limit=100&verbose");
        assert_eq!(params.len(), 3);
        assert_eq!(params[0], ("model".into(), "gpt-4".into()));
        assert_eq!(params[1], ("limit".into(), "100".into()));
        assert_eq!(params[2], ("verbose".into(), "".into()));
    }

    #[test]
    fn test_url_params_empty() {
        let params = url_params("");
        // Empty string produces one empty-key entry
        assert_eq!(params.len(), 1);
    }

    // ── resolve_field: request.* ─────────────────────────────

    #[test]
    fn test_resolve_request_method() {
        let method = Method::POST;
        let uri: Uri = "/test".parse().unwrap();
        let headers = HeaderMap::new();
        let ctx = make_ctx(&method, "/test", &uri, &headers, None);

        assert_eq!(resolve_field("request.method", &ctx), Some(json!("POST")));
    }

    #[test]
    fn test_resolve_request_path() {
        let method = Method::GET;
        let uri: Uri = "/v1/charges".parse().unwrap();
        let headers = HeaderMap::new();
        let ctx = make_ctx(&method, "/v1/charges", &uri, &headers, None);

        assert_eq!(
            resolve_field("request.path", &ctx),
            Some(json!("/v1/charges"))
        );
    }

    #[test]
    fn test_resolve_request_body_size() {
        let method = Method::POST;
        let uri: Uri = "/api".parse().unwrap();
        let headers = HeaderMap::new();
        let body = json!({"model": "gpt-4", "prompt": "hello"});
        let ctx = make_ctx(&method, "/api", &uri, &headers, Some(&body));

        let result = resolve_field("request.body_size", &ctx).unwrap();
        assert!(result.as_u64().unwrap() > 0);
    }

    #[test]
    fn test_resolve_request_body_field() {
        let method = Method::POST;
        let uri: Uri = "/api".parse().unwrap();
        let headers = HeaderMap::new();
        let body = json!({"model": "gpt-4", "max_tokens": 1024});
        let ctx = make_ctx(&method, "/api", &uri, &headers, Some(&body));

        assert_eq!(
            resolve_field("request.body.model", &ctx),
            Some(json!("gpt-4"))
        );
        assert_eq!(
            resolve_field("request.body.max_tokens", &ctx),
            Some(json!(1024))
        );
        assert_eq!(resolve_field("request.body.missing", &ctx), None);
    }

    #[test]
    fn test_resolve_request_body_when_none() {
        let method = Method::GET;
        let uri: Uri = "/test".parse().unwrap();
        let headers = HeaderMap::new();
        let ctx = make_ctx(&method, "/test", &uri, &headers, None);

        assert_eq!(resolve_field("request.body.anything", &ctx), None);
    }

    #[test]
    fn test_resolve_request_header() {
        let method = Method::GET;
        let uri: Uri = "/test".parse().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("x-custom-id", "agent-007".parse().unwrap());
        headers.insert("content-type", "application/json".parse().unwrap());
        let ctx = make_ctx(&method, "/test", &uri, &headers, None);

        assert_eq!(
            resolve_field("request.headers.x-custom-id", &ctx),
            Some(json!("agent-007"))
        );
        assert_eq!(
            resolve_field("request.headers.content-type", &ctx),
            Some(json!("application/json"))
        );
        assert_eq!(resolve_field("request.headers.missing", &ctx), None);
    }

    #[test]
    fn test_resolve_request_query() {
        let method = Method::GET;
        let uri: Uri = "/api?model=gpt-4&limit=50".parse().unwrap();
        let headers = HeaderMap::new();
        let ctx = make_ctx(&method, "/api", &uri, &headers, None);

        assert_eq!(
            resolve_field("request.query.model", &ctx),
            Some(json!("gpt-4"))
        );
        assert_eq!(
            resolve_field("request.query.limit", &ctx),
            Some(json!("50"))
        );
        assert_eq!(resolve_field("request.query.missing", &ctx), None);
    }

    #[test]
    fn test_resolve_request_query_no_querystring() {
        let method = Method::GET;
        let uri: Uri = "/api".parse().unwrap();
        let headers = HeaderMap::new();
        let ctx = make_ctx(&method, "/api", &uri, &headers, None);

        assert_eq!(resolve_field("request.query.anything", &ctx), None);
    }

    // ── resolve_field: agent.* ───────────────────────────────

    #[test]
    fn test_resolve_agent_name() {
        let method = Method::GET;
        let uri: Uri = "/test".parse().unwrap();
        let headers = HeaderMap::new();
        let ctx = make_ctx(&method, "/test", &uri, &headers, None);

        assert_eq!(resolve_field("agent.name", &ctx), Some(json!("test-agent")));
    }

    #[test]
    fn test_resolve_agent_unknown_field() {
        let method = Method::GET;
        let uri: Uri = "/test".parse().unwrap();
        let headers = HeaderMap::new();
        let ctx = make_ctx(&method, "/test", &uri, &headers, None);

        assert_eq!(resolve_field("agent.role", &ctx), None);
    }

    // ── resolve_field: token.* ───────────────────────────────

    #[test]
    fn test_resolve_token_fields() {
        let method = Method::GET;
        let uri: Uri = "/test".parse().unwrap();
        let headers = HeaderMap::new();
        let ctx = make_ctx(&method, "/test", &uri, &headers, None);

        assert_eq!(resolve_field("token.id", &ctx), Some(json!("tok_abc123")));
        assert_eq!(resolve_field("token.name", &ctx), Some(json!("My Token")));
        assert_eq!(
            resolve_field("token.project_id", &ctx),
            Some(json!("proj_xyz"))
        );
        assert_eq!(resolve_field("token.purpose", &ctx), Some(json!("llm")));
        assert_eq!(resolve_field("token.unknown", &ctx), None);
    }

    // ── resolve_field: context.* ─────────────────────────────

    #[test]
    fn test_resolve_context_ip() {
        let method = Method::GET;
        let uri: Uri = "/test".parse().unwrap();
        let headers = HeaderMap::new();
        let ctx = make_ctx(&method, "/test", &uri, &headers, None);

        assert_eq!(resolve_field("context.ip", &ctx), Some(json!("10.0.0.42")));
    }

    #[test]
    fn test_resolve_context_time_hour() {
        let method = Method::GET;
        let uri: Uri = "/test".parse().unwrap();
        let headers = HeaderMap::new();
        let ctx = make_ctx(&method, "/test", &uri, &headers, None);

        let result = resolve_field("context.time.hour", &ctx).unwrap();
        let hour = result.as_u64().unwrap();
        assert!(hour < 24, "Hour should be 0-23, got {}", hour);
    }

    #[test]
    fn test_resolve_context_time_weekday() {
        let method = Method::GET;
        let uri: Uri = "/test".parse().unwrap();
        let headers = HeaderMap::new();
        let ctx = make_ctx(&method, "/test", &uri, &headers, None);

        let result = resolve_field("context.time.weekday", &ctx).unwrap();
        let day = result.as_str().unwrap();
        let valid = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
        assert!(
            valid.contains(&day),
            "Weekday should be one of {:?}, got {}",
            valid,
            day
        );
    }

    #[test]
    fn test_resolve_context_time_date() {
        let method = Method::GET;
        let uri: Uri = "/test".parse().unwrap();
        let headers = HeaderMap::new();
        let ctx = make_ctx(&method, "/test", &uri, &headers, None);

        let result = resolve_field("context.time.date", &ctx).unwrap();
        let date = result.as_str().unwrap();
        // Should match YYYY-MM-DD format
        assert_eq!(date.len(), 10);
        assert_eq!(&date[4..5], "-");
        assert_eq!(&date[7..8], "-");
    }

    // ── resolve_field: usage.* ───────────────────────────────

    #[test]
    fn test_resolve_usage_counter() {
        let method = Method::GET;
        let uri: Uri = "/test".parse().unwrap();
        let headers = HeaderMap::new();
        let mut ctx = make_ctx(&method, "/test", &uri, &headers, None);
        ctx.usage.insert("spend_today_usd".to_string(), 42.5);
        ctx.usage.insert("requests_this_hour".to_string(), 150.0);

        assert_eq!(
            resolve_field("usage.spend_today_usd", &ctx),
            Some(json!(42.5))
        );
        assert_eq!(
            resolve_field("usage.requests_this_hour", &ctx),
            Some(json!(150.0))
        );
        assert_eq!(resolve_field("usage.unknown_counter", &ctx), None);
    }

    // ── resolve_field: response.* ────────────────────────────

    #[test]
    fn test_resolve_response_status() {
        let method = Method::GET;
        let uri: Uri = "/test".parse().unwrap();
        let headers = HeaderMap::new();
        let mut ctx = make_ctx(&method, "/test", &uri, &headers, None);
        ctx.response_status = Some(200);

        assert_eq!(resolve_field("response.status", &ctx), Some(json!(200)));
    }

    #[test]
    fn test_resolve_response_status_none() {
        let method = Method::GET;
        let uri: Uri = "/test".parse().unwrap();
        let headers = HeaderMap::new();
        let ctx = make_ctx(&method, "/test", &uri, &headers, None);

        assert_eq!(resolve_field("response.status", &ctx), None);
    }

    #[test]
    fn test_resolve_response_body() {
        let method = Method::GET;
        let uri: Uri = "/test".parse().unwrap();
        let headers = HeaderMap::new();
        let resp_body = json!({"error": {"code": "rate_limit_exceeded", "status": 429}});
        let mut ctx = make_ctx(&method, "/test", &uri, &headers, None);
        ctx.response_body = Some(&resp_body);

        assert_eq!(
            resolve_field("response.body.error.code", &ctx),
            Some(json!("rate_limit_exceeded"))
        );
        assert_eq!(
            resolve_field("response.body.error.status", &ctx),
            Some(json!(429))
        );
    }

    #[test]
    fn test_resolve_response_headers() {
        let method = Method::GET;
        let uri: Uri = "/test".parse().unwrap();
        let headers = HeaderMap::new();
        let mut resp_headers = HeaderMap::new();
        resp_headers.insert("x-ratelimit-remaining", "5".parse().unwrap());
        let mut ctx = make_ctx(&method, "/test", &uri, &headers, None);
        ctx.response_headers = Some(&resp_headers);

        assert_eq!(
            resolve_field("response.headers.x-ratelimit-remaining", &ctx),
            Some(json!("5"))
        );
    }

    // ── Edge cases ───────────────────────────────────────────

    #[test]
    fn test_resolve_unknown_prefix() {
        let method = Method::GET;
        let uri: Uri = "/test".parse().unwrap();
        let headers = HeaderMap::new();
        let ctx = make_ctx(&method, "/test", &uri, &headers, None);

        assert_eq!(resolve_field("unknown.field", &ctx), None);
        assert_eq!(resolve_field("jwt.sub", &ctx), None); // JWT not implemented yet
    }

    #[test]
    fn test_resolve_no_dot_returns_none() {
        let method = Method::GET;
        let uri: Uri = "/test".parse().unwrap();
        let headers = HeaderMap::new();
        let ctx = make_ctx(&method, "/test", &uri, &headers, None);

        // No dot separator → split_once returns None
        assert_eq!(resolve_field("nodotshere", &ctx), None);
    }
}
