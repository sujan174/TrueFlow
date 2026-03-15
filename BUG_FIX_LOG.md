# Guardrails & PII Pipeline Bug Fix Log

**Last Updated:** 2026-03-14
**Status:** Tracking medium/low priority bugs for future sprints

---

## FIXED Issues

### CRIT-1: SSRF Vulnerability in External Guardrail Endpoints
- **File:** `gateway/src/middleware/external_guardrail.rs`
- **Issue:** Endpoints not validated for SSRF protection
- **Fix:** Added `is_safe_webhook_url()` validation before all vendor HTTP calls
- **Status:** 🟢 Fixed (2026-03-14)

### CRIT-2: Risk Score Accumulation Bug
- **File:** `gateway/src/middleware/guardrail/mod.rs:159`
- **Issue:** Code injection SET risk_score instead of ADDING to it
- **Fix:** Changed `=` to `+=` for accumulation
- **Status:** 🟢 Fixed (2026-03-14)

### HIGH-1: LlamaGuard Prompt Injection
- **File:** `gateway/src/middleware/external_guardrail.rs:290-299`
- **Issue:** User text interpolated without escaping
- **Fix:** Added `escape_llama_guard_text()` function to strip special tokens
- **Status:** 🟢 Fixed (2026-03-14)

### HIGH-2: Content Extraction Missed tool_calls
- **File:** `gateway/src/middleware/guardrail/mod.rs:377-410`
- **Issue:** `extract_text_content` missed tool_calls, function_call, tools
- **Fix:** Added extraction for all three fields
- **Status:** 🟢 Fixed (2026-03-14)

### MED-6: Base64 Pattern Matches Legitimate JWT/API Keys
- **File:** `gateway/src/middleware/guardrail/patterns.rs:198`
- **Issue:** 60+ char base64 pattern matches JWTs, API keys, data URIs
- **Fix:** Increased threshold from 60 to 80 chars to avoid matching JWTs (3 parts ~45 chars each) and typical API keys
- **Status:** 🟢 Fixed (2026-03-14)

### LOW-3: Repeated Character Patterns Incomplete
- **File:** `gateway/src/middleware/guardrail/patterns.rs:205-210`
- **Issue:** Only 5 character types checked (A, X, ., !, 0)
- **Fix:** Note: Regex crate doesn't support backreferences. Current patterns cover common padding chars. Added tests for gibberish detection.
- **Status:** ⚪ Won't Fix (regex limitation, patterns sufficient for detection)

### Additional Fix: Contact Info Regex Syntax Error
- **File:** `gateway/src/middleware/guardrail/patterns.rs:235`
- **Issue:** Social media handle pattern used negative lookahead `(?!\s*@[a-z])` which isn't supported by Rust's regex crate
- **Fix:** Simplified pattern to `r"(?i)@[a-z0-9_]{3,15}\b"` without lookahead
- **Status:** 🟢 Fixed (2026-03-14)

### HIGH-3: X-TrueFlow-Guardrails Header Authorization Bypass
- **File:** `gateway/src/proxy/handler/core.rs:311-393`
- **Issue:** Any authenticated client could inject guardrail actions via the `X-TrueFlow-Guardrails` header, potentially enabling DoS or bypassing configured policies
- **Fix:** Added `guardrail_header_mode` token-level setting with options: `disabled` (default, security), `append`, `override`. Header is now ignored by default.
- **Files Changed:**
  - `migrations/041_token_guardrail_header_mode.sql` - Database migration
  - `src/store/postgres/types.rs` - TokenRow struct
  - `src/store/postgres/tokens.rs` - SQL queries
  - `src/proxy/handler/core.rs` - Header processing logic
- **Status:** 🟢 Fixed (2026-03-14)

### HIGH-4: External Guardrail Fail-Open Default
- **File:** `gateway/src/proxy/handler/core.rs:1032-1039`
- **Issue:** External guardrail vendor failures (timeout, network error) always failed open with no option to fail closed for security-sensitive deployments
- **Fix:** Added `on_error` field to `ExternalGuardrail` action with options: `allow` (default, fail-open), `deny` (fail-closed). Security-sensitive deployments can now block requests when vendors are unavailable.
- **Files Changed:**
  - `src/models/policy.rs` - ExternalGuardrail action definition
  - `src/proxy/handler/core.rs` - Pre-flight and post-flight error handling
  - `src/proxy/post_flight.rs` - Post-flight error handling
- **Status:** 🟢 Fixed (2026-03-14)

---

## MEDIUM Priority Bugs

### MED-1: Error Messages Leak Raw API Responses
- **File:** `gateway/src/middleware/external_guardrail.rs`
- **Lines:** 170, 241, 325, 414, 509
- **Issue:** Error messages include complete raw response body which may contain sensitive data
- **Fix:** Redact/truncate raw responses in error messages
- **Status:** 🔴 Open

### MED-2: AWS Comprehend Authentication Design Confusion
- **File:** `gateway/src/middleware/external_guardrail.rs:204-232`
- **Issue:** Code comments mention SigV4 but implementation uses Bearer token for proxy
- **Fix:** Rename `api_key` to `proxy_token`, add validation, improve docs
- **Status:** 🔴 Open

### MED-3: Inconsistent Client Timeout Configuration
- **File:** `gateway/src/middleware/external_guardrail.rs`
- **Issue:** Some vendors have explicit timeouts (10s, 30s), others rely on outer wrapper (5s)
- **Fix:** Standardize timeout handling across all vendors
- **Status:** 🔴 Open

### MED-4: Cache Eviction Clears ALL 256 Entries
- **File:** `gateway/src/middleware/guardrail/mod.rs:325-327`
- **Issue:** When cache reaches 256 entries, all are cleared causing recompilation
- **Fix:** Implement LRU eviction instead of clear-all
- **Status:** 🔴 Open

### MED-5: Invalid Custom Patterns Silently Ignored
- **File:** `gateway/src/middleware/guardrail/mod.rs:321-324`
- **Issue:** Invalid regex patterns fail silently with no logging
- **Fix:** Add warning log for invalid patterns
- **Status:** 🔴 Open

### MED-7: Risk Score Weighting Inconsistency
- **File:** `gateway/src/middleware/guardrail/mod.rs`
- **Issue:** Profanity (0.7) scores higher than jailbreak (0.5) per match
- **Fix:** Re-evaluate scoring weights by severity
- **Status:** 🔴 Open

### MED-8: Bias Pattern False Positives on Technical Terms
- **File:** `gateway/src/middleware/guardrail/patterns.rs:130`
- **Issue:** "All women are eligible" matches bias pattern (legitimate medical text)
- **Fix:** Add negation patterns for legitimate contexts
- **Status:** 🔴 Open

### MED-9: Shadow Mode Only Logs, No Alerting
- **File:** `gateway/src/middleware/engine/mod.rs:58-73`
- **Issue:** Shadow violations only logged, no webhook/alert mechanism
- **Fix:** Add configurable alerting for shadow violations
- **Status:** 🔴 Open

### MED-10: Credit Card Pattern Gaps
- **File:** `gateway/src/middleware/redact.rs:36-37`
- **Issue:** Misses Amex format (4-6-5), Diners Club (14 digits)
- **Fix:** Add Amex-specific pattern, support 14-16 digit range
- **Status:** 🔴 Open

---

## LOW Priority Bugs

### LOW-1: Threshold Ignored for LlamaGuard
- **File:** `gateway/src/middleware/external_guardrail.rs:285`
- **Issue:** `_threshold` parameter ignored, all "unsafe" blocks
- **Fix:** Document clearly or support confidence thresholds
- **Status:** 🔴 Open

### LOW-2: No Rate Limiting for Vendor API Calls
- **File:** `gateway/src/middleware/external_guardrail.rs`
- **Issue:** No client-side rate limiting for external vendor calls
- **Fix:** Consider connection pooling with limits
- **Status:** 🔴 Open

### LOW-4: Phone Number Pattern US-Centric
- **File:** `gateway/src/middleware/guardrail/patterns.rs:197-199`
- **Issue:** Only US and E.164 formats supported
- **Fix:** Add international formats if needed
- **Status:** 🔴 Open

### LOW-5: UK Postcode Pattern May Match Non-UK Text
- **File:** `gateway/src/middleware/guardrail/patterns.rs:209`
- **Issue:** Broad pattern matches version strings, product codes
- **Fix:** Add UK-specific validation
- **Status:** 🔴 Open

### LOW-6: Entity Text Bounds Errors Silently Skipped
- **File:** `gateway/src/middleware/pii/presidio.rs:126-139`
- **Issue:** Invalid entity bounds silently skipped without logging
- **Fix:** Log warning when bounds are invalid
- **Status:** 🔴 Open

---

## Changelog

| Date | Bug ID | Action | Notes |
|------|--------|--------|-------|
| 2026-03-14 | Initial | Created | Bug list from security audit |
| 2026-03-14 | MED-6 | Fixed | Increased base64 threshold from 60 to 80 chars |
| 2026-03-14 | LOW-3 | Won't Fix | Regex limitation, patterns sufficient |
| 2026-03-14 | Contact Regex | Fixed | Removed unsupported negative lookahead |
| 2026-03-14 | HIGH-3 | Fixed | Added token-level guardrail_header_mode control |
| 2026-03-14 | HIGH-4 | Fixed | Added on_error field to ExternalGuardrail for fail-closed option |

---

## Legend

- 🔴 Open - Not yet addressed
- 🟡 In Progress - Currently being worked on
- 🟢 Fixed - Resolved and tested
- ⚪ Won't Fix - Documented as acceptable tradeoff