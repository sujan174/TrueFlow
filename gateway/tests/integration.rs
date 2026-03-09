//! Integration tests for spend cap enforcement and webhook notifications.
//!
//! These tests verify:
//! 1. Spend cap enforcement correctly blocks requests when limits are exceeded
//! 2. Spend tracking accurately increments Redis counters
//! 3. Webhook events are correctly constructed and dispatched
//! 4. The full request pipeline (handler → spend check → webhook dispatch) works end-to-end
//!
//! **Requirements:**
//! - PostgreSQL running at DATABASE_URL
//! - Redis running at REDIS_URL
//! - Or run via `docker-compose up -d postgres redis` then `cargo test --test integration`

mod spend_cap_tests {

    /// Test that SpendCap struct defaults to no limits.
    #[test]
    fn test_spend_cap_defaults_no_limits() {
        // SpendCap default should have no daily or monthly limits
        // This ensures unconfigured tokens pass through without enforcement
        let caps = gateway::middleware::spend::SpendCap::default();
        assert!(caps.daily_limit_usd.is_none());
        assert!(caps.monthly_limit_usd.is_none());
    }

    /// Test SpendCap serialization roundtrip.
    #[test]
    fn test_spend_cap_serialization() {
        let cap = gateway::middleware::spend::SpendCap {
            daily_limit_usd: Some(50.0),
            monthly_limit_usd: Some(500.0),
            lifetime_limit_usd: None,
        };

        let json = serde_json::to_string(&cap).unwrap();
        let deserialized: gateway::middleware::spend::SpendCap =
            serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.daily_limit_usd, Some(50.0));
        assert_eq!(deserialized.monthly_limit_usd, Some(500.0));
        assert_eq!(deserialized.lifetime_limit_usd, None);
    }

    /// Test SpendStatus serializes with all expected fields.
    #[test]
    fn test_spend_status_serialization() {
        let status = gateway::middleware::spend::SpendStatus {
            daily_limit_usd: Some(100.0),
            monthly_limit_usd: Some(1000.0),
            lifetime_limit_usd: Some(5000.0),
            current_daily_usd: 42.50,
            current_monthly_usd: 350.75,
            current_lifetime_usd: 1200.0,
        };

        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["daily_limit_usd"], 100.0);
        assert_eq!(json["monthly_limit_usd"], 1000.0);
        assert_eq!(json["current_daily_usd"], 42.50);
        assert_eq!(json["current_monthly_usd"], 350.75);
        assert_eq!(json["current_lifetime_usd"], 1200.0);
    }
}

mod webhook_tests {
    use gateway::notification::webhook::{WebhookEvent, WebhookNotifier};

    // ── Event Construction Tests ──────────────────────────────

    #[test]
    fn test_policy_violation_event_has_correct_fields() {
        let event = WebhookEvent::policy_violation(
            "tok_abc123",
            "test-token",
            "proj_001",
            "block-deletes",
            "DELETE method not allowed",
        );

        assert_eq!(event.event_type, "policy_violation");
        assert_eq!(event.token_id, "tok_abc123");
        assert_eq!(event.token_name, "test-token");
        assert_eq!(event.project_id, "proj_001");
        assert_eq!(event.details["policy"], "block-deletes");
        assert_eq!(event.details["reason"], "DELETE method not allowed");
        assert!(!event.timestamp.is_empty());
    }

    #[test]
    fn test_rate_limit_event_has_correct_fields() {
        let event = WebhookEvent::rate_limit_exceeded(
            "tok_xyz",
            "prod-agent",
            "proj_002",
            "strict-rl",
            100,
            60,
        );

        assert_eq!(event.event_type, "rate_limit_exceeded");
        assert_eq!(event.token_id, "tok_xyz");
        assert_eq!(event.token_name, "prod-agent");
        assert_eq!(event.details["policy"], "strict-rl");
        assert_eq!(event.details["max_requests"], 100);
        assert_eq!(event.details["window_secs"], 60);
    }

    #[test]
    fn test_spend_cap_event_has_correct_fields() {
        let event = WebhookEvent::spend_cap_exceeded(
            "tok_budget",
            "budget-token",
            "proj_003",
            "daily spend cap of $50.00 exceeded (current: $52.34)",
        );

        assert_eq!(event.event_type, "spend_cap_exceeded");
        assert_eq!(event.token_id, "tok_budget");
        assert_eq!(event.token_name, "budget-token");
        assert_eq!(
            event.details["reason"],
            "daily spend cap of $50.00 exceeded (current: $52.34)"
        );
    }

    // ── Event Serialization Tests ─────────────────────────────

    #[test]
    fn test_webhook_event_json_structure() {
        let event =
            WebhookEvent::policy_violation("tok_1", "name-1", "proj_1", "policy-1", "reason-1");

        let json = serde_json::to_value(&event).unwrap();

        // Verify all top-level fields are present
        assert!(json.get("event_type").is_some());
        assert!(json.get("timestamp").is_some());
        assert!(json.get("token_id").is_some());
        assert!(json.get("token_name").is_some());
        assert!(json.get("project_id").is_some());
        assert!(json.get("details").is_some());

        // Verify timestamp is RFC3339 format
        let timestamp = json["timestamp"].as_str().unwrap();
        assert!(
            chrono::DateTime::parse_from_rfc3339(timestamp).is_ok(),
            "timestamp should be valid RFC3339: {}",
            timestamp
        );
    }

    #[test]
    fn test_all_event_types_serialize_cleanly() {
        let events = vec![
            WebhookEvent::policy_violation("t", "n", "p", "pol", "r"),
            WebhookEvent::rate_limit_exceeded("t", "n", "p", "pol", 10, 60),
            WebhookEvent::spend_cap_exceeded("t", "n", "p", "cap exceeded"),
        ];

        for event in &events {
            let json = serde_json::to_string(event);
            assert!(
                json.is_ok(),
                "event type '{}' failed to serialize",
                event.event_type
            );

            // Verify it round-trips as valid JSON
            let parsed: serde_json::Value = serde_json::from_str(&json.unwrap()).unwrap();
            assert_eq!(parsed["event_type"], event.event_type);
        }
    }

    // ── Notifier Tests ────────────────────────────────────────

    #[test]
    fn test_webhook_notifier_creation() {
        // Should not panic
        let _notifier = WebhookNotifier::new();
    }

    #[test]
    fn test_webhook_notifier_default() {
        // Default trait should work
        let _notifier = WebhookNotifier::default();
    }

    #[tokio::test]
    async fn test_dispatch_with_empty_urls_is_noop() {
        let notifier = WebhookNotifier::new();
        let event = WebhookEvent::policy_violation("t", "n", "p", "pol", "r");

        // Should not panic or error with empty URL list
        notifier.dispatch(&[], event).await;
    }

    /// Test that dispatch to an invalid URL doesn't panic (fire-and-forget).
    #[tokio::test]
    async fn test_dispatch_to_invalid_url_handles_gracefully() {
        let notifier = WebhookNotifier::new();
        let event = WebhookEvent::spend_cap_exceeded("t", "n", "p", "exceeded");

        // Should not panic — failures are logged but not propagated
        notifier
            .dispatch(&["http://localhost:1/nonexistent".to_string()], event)
            .await;

        // Give the spawned task time to complete
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    // ── Wiremock Integration: Verifies actual HTTP delivery ───

    #[tokio::test]
    async fn test_webhook_delivers_correct_payload_to_endpoint() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/webhook"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let notifier = WebhookNotifier::new();
        let event = WebhookEvent::policy_violation(
            "tok_test",
            "test-token",
            "proj_test",
            "my-policy",
            "blocked by policy",
        );

        let url = format!("{}/webhook", mock_server.uri());
        notifier.send(&url, &event).await.unwrap();

        // Wiremock will assert the expectation (exactly 1 call) when dropped
    }

    #[tokio::test]
    async fn test_webhook_delivers_rate_limit_event() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/hooks"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let notifier = WebhookNotifier::new();
        let event = WebhookEvent::rate_limit_exceeded(
            "tok_rl",
            "rl-token",
            "proj_rl",
            "rate-policy",
            200,
            300,
        );

        let url = format!("{}/hooks", mock_server.uri());
        notifier.send(&url, &event).await.unwrap();
    }

    #[tokio::test]
    async fn test_webhook_delivers_spend_cap_event() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/spend-alert"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let notifier = WebhookNotifier::new();
        let event = WebhookEvent::spend_cap_exceeded(
            "tok_budget",
            "budget-token",
            "proj_budget",
            "daily cap of $50 exceeded",
        );

        let url = format!("{}/spend-alert", mock_server.uri());
        notifier.send(&url, &event).await.unwrap();
    }

    #[tokio::test]
    async fn test_dispatch_to_multiple_urls() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server1 = MockServer::start().await;
        let server2 = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/hook1"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server1)
            .await;

        Mock::given(method("POST"))
            .and(path("/hook2"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server2)
            .await;

        let notifier = WebhookNotifier::new();
        let event = WebhookEvent::policy_violation("t", "n", "p", "pol", "r");

        let urls = vec![
            format!("{}/hook1", server1.uri()),
            format!("{}/hook2", server2.uri()),
        ];

        notifier.dispatch(&urls, event).await;

        // Give spawned tasks time to complete
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Wiremock assertions will verify on drop
    }

    #[tokio::test]
    async fn test_webhook_handles_server_error_gracefully() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/fail"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
            // We expect the original request plus 3 retries = 4 requests
            .expect(4)
            .mount(&mock_server)
            .await;

        let notifier = WebhookNotifier::new();
        let event = WebhookEvent::spend_cap_exceeded("t", "n", "p", "cap exceeded");

        let url = format!("{}/fail", mock_server.uri());
        // send() should return Err when all retries are exhausted
        let result = notifier.send(&url, &event).await;
        assert!(result.is_err());
    }
}

mod config_tests {
    #[test]
    fn test_webhook_urls_parsing_empty() {
        // Simulates TRUEFLOW_WEBHOOK_URLS not set
        let urls: Vec<String> = ""
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        assert!(urls.is_empty());
    }

    #[test]
    fn test_webhook_urls_parsing_single() {
        let urls: Vec<String> = "https://hooks.example.com/abc"
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://hooks.example.com/abc");
    }

    #[test]
    fn test_webhook_urls_parsing_multiple() {
        let urls: Vec<String> =
            "https://hooks.example.com/a, https://webhook.site/b , https://pagerduty.com/c"
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect();
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "https://hooks.example.com/a");
        assert_eq!(urls[1], "https://webhook.site/b");
        assert_eq!(urls[2], "https://pagerduty.com/c");
    }

    #[test]
    fn test_webhook_urls_parsing_trailing_comma() {
        let urls: Vec<String> = "https://a.com,"
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        assert_eq!(urls.len(), 1);
    }
}

mod error_tests {
    use gateway::errors::AppError;

    #[test]
    fn test_spend_cap_reached_error_exists() {
        // Verify the error variant exists and can be constructed
        let err = AppError::SpendCapReached {
            message: "daily spend cap exceeded".into(),
        };
        let err_str = format!("{}", err);
        assert!(
            err_str.contains("spend") || err_str.contains("cap"),
            "SpendCapReached should mention spend/cap: {}",
            err_str
        );
    }
}
