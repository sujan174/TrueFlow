//! Tests for security audit fixes: regex false-positive reduction.
//!
//! Covers:
//! - BUG-04: Credit card regex false positive reduction
//! - BUG-05: Passport/DL regex false positive reduction
//!
//! Note: SSRF and header-redaction tests are in handler.rs #[cfg(test)].
//! Note: slugify and render_variables tests are in prompt_handlers.rs #[cfg(test)].

// ── BUG-04: Credit card regex ──────────────────────────────────────────

mod cc_regex_tests {
    use gateway::middleware::sanitize::sanitize_stream_content;

    #[test]
    fn test_cc_regex_matches_grouped_visa() {
        let result = sanitize_stream_content("My card is 4111 1111 1111 1111 thanks");
        let output = String::from_utf8(result.body).unwrap();
        assert!(
            !output.contains("4111 1111 1111 1111"),
            "Grouped Visa number should be redacted, got: {}",
            output
        );
        assert!(result.redacted_types.contains(&"credit_card".to_string()));
    }

    #[test]
    fn test_cc_regex_matches_dashed() {
        let result = sanitize_stream_content("Card: 4111-1111-1111-1111");
        let output = String::from_utf8(result.body).unwrap();
        assert!(
            !output.contains("4111-1111-1111-1111"),
            "Dashed card number should be redacted, got: {}",
            output
        );
    }

    #[test]
    fn test_cc_regex_no_false_positive_on_timestamp() {
        let input = "Timestamp: 1709234567890 is the event time";
        let result = sanitize_stream_content(input);
        let output = String::from_utf8(result.body).unwrap();
        assert_eq!(output, input, "13-digit timestamp should NOT be redacted");
    }

    #[test]
    fn test_cc_regex_no_false_positive_on_scattered_digits() {
        let input = "ID 1 2 3 4 5 6 7 8 9 0 1 2 3 end";
        let result = sanitize_stream_content(input);
        let output = String::from_utf8(result.body).unwrap();
        assert_eq!(
            output, input,
            "Scattered single digits should NOT be redacted as CC"
        );
    }
}

// ── BUG-05: Passport / DL regex ────────────────────────────────────────

mod pii_regex_tests {
    use gateway::middleware::redact::redact_for_logging;

    #[test]
    fn test_passport_matches_valid_format() {
        let body = Some(serde_json::json!({"msg": "Passport: AB1234567 is valid"}));
        let result = redact_for_logging(&body).unwrap();
        assert!(
            !result.contains("AB1234567"),
            "Valid passport (2 letters + 7 digits) should be redacted, got: {}",
            result
        );
    }

    #[test]
    fn test_version_code_correctly_caught_as_dl_when_8_digits() {
        // V12345678 = 1 letter + 8 digits → matches DL pattern (correct behavior).
        // The BUG-05 fix raised the minimum from 4 to 7 digits, so V1234 won't match
        // but V12345678 (8 digits) legitimately looks like a DL number.
        let body = Some(serde_json::json!({"msg": "Version V12345678 is the latest release"}));
        let result = redact_for_logging(&body).unwrap();
        assert!(
            !result.contains("V12345678"),
            "V + 8 digits legitimately matches DL pattern, should be redacted, got: {}",
            result
        );
    }

    #[test]
    fn test_dl_matches_valid_format() {
        let body = Some(serde_json::json!({"msg": "License: A12345678 on file"}));
        let result = redact_for_logging(&body).unwrap();
        assert!(
            !result.contains("A12345678"),
            "Valid DL (1 letter + 8 digits) should be redacted, got: {}",
            result
        );
    }

    #[test]
    fn test_dl_no_false_positive_on_model_name() {
        let body = Some(serde_json::json!({"msg": "The NVIDIA A100 GPU is powerful"}));
        let result = redact_for_logging(&body).unwrap();
        assert!(
            result.contains("A100"),
            "GPU model name A100 should NOT be redacted as DL, got: {}",
            result
        );
    }

    #[test]
    fn test_dl_no_false_positive_on_short_code() {
        let body = Some(serde_json::json!({"msg": "Product code B2345 is in stock"}));
        let result = redact_for_logging(&body).unwrap();
        assert!(
            result.contains("B2345"),
            "Short code should NOT be redacted as DL, got: {}",
            result
        );
    }
}
