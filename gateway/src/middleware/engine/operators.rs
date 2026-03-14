use regex::Regex;
use serde_json::Value;

pub(super) fn values_equal(a: &Value, b: &Value) -> bool {
    // Direct equality
    if a == b {
        return true;
    }
    // String ↔ number coercion
    match (a, b) {
        (Value::String(s), Value::Number(n)) | (Value::Number(n), Value::String(s)) => {
            if let Ok(parsed) = s.parse::<f64>() {
                if let Some(expected) = n.as_f64() {
                    return (parsed - expected).abs() < f64::EPSILON;
                }
            }
            false
        }
        _ => false,
    }
}

/// Compare as f64 using a comparator function.
pub(super) fn compare_numeric(actual: &Value, expected: &Value, cmp: fn(f64, f64) -> bool) -> bool {
    let a = to_f64(actual);
    let b = to_f64(expected);
    match (a, b) {
        (Some(a), Some(b)) => cmp(a, b),
        _ => false,
    }
}

/// Maximum absolute value for numeric comparisons.
/// This prevents overflow issues while accommodating legitimate use cases:
/// - Token counts (even GPT-4's 128K context is well under 1e15)
/// - Timestamps in milliseconds (year 5138 CE at 1e17ms, 1e15ms is year 2001)
/// - Dollar amounts (1e15 = 1 quadrillion, covers enterprise billing)
const NUMERIC_MAX_VALUE: f64 = 1e15;

pub(super) fn to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => {
            let val = n.as_f64()?;
            // Validate: reject NaN, Infinity, and extremely large values
            if val.is_finite() && val.abs() < NUMERIC_MAX_VALUE {
                Some(val)
            } else {
                None
            }
        }
        Value::String(s) => {
            let val: f64 = s.parse().ok()?;
            // Validate: reject NaN, Infinity, and extremely large values
            if val.is_finite() && val.abs() < NUMERIC_MAX_VALUE {
                Some(val)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Check if `actual` is contained in the `expected` array.
pub(super) fn check_in(actual: &Value, expected: &Value) -> bool {
    match expected {
        Value::Array(arr) => arr.iter().any(|v| values_equal(actual, v)),
        _ => false,
    }
}

/// Glob pattern matching (supports `*` and `?`).
pub(super) fn check_glob(actual: &Value, pattern: &Value) -> bool {
    let actual_str = value_as_str(actual);
    let pattern_str = value_as_str(pattern);
    match (actual_str, pattern_str) {
        (Some(a), Some(p)) => glob_match(&p, &a),
        _ => false,
    }
}

/// Maximum iterations for glob matching to prevent DoS via backtracking.
/// This is generous enough for patterns/texts up to ~10KB each with many wildcards,
/// but prevents exponential blowup from pathological inputs.
const GLOB_MAX_ITERATIONS: u64 = 100_000;

/// Simple glob matching: `*` matches any sequence, `?` matches one char.
/// SEC: bounded iteration prevents DoS via backtracking on pathological patterns.
pub(crate) fn glob_match(pattern: &str, text: &str) -> bool {
    if pattern == "*" || pattern == "/*" {
        return true;
    }

    let mut p_chars = pattern.chars().peekable();
    let mut t_chars = text.chars().peekable();
    let mut p_stack: Vec<(
        std::iter::Peekable<std::str::Chars>,
        std::iter::Peekable<std::str::Chars>,
    )> = Vec::new();
    let mut iterations: u64 = 0;

    loop {
        iterations += 1;
        if iterations > GLOB_MAX_ITERATIONS {
            tracing::warn!(
                pattern = %pattern,
                text_len = text.len(),
                "glob_match: iteration limit exceeded, returning false"
            );
            return false;
        }

        match (p_chars.peek(), t_chars.peek()) {
            (Some('*'), _) => {
                p_chars.next();
                // Save state for backtracking
                p_stack.push((p_chars.clone(), t_chars.clone()));
            }
            (Some('?'), Some(_)) => {
                p_chars.next();
                t_chars.next();
            }
            (Some(pc), Some(tc)) if *pc == *tc => {
                p_chars.next();
                t_chars.next();
            }
            (None, None) => return true,
            _ => {
                // Backtrack to last * position
                if let Some((saved_p, mut saved_t)) = p_stack.pop() {
                    if saved_t.peek().is_none() {
                        return false; // Can't advance text anymore
                    }
                    saved_t.next(); // Consume one more char from text
                    p_chars = saved_p;
                    t_chars = saved_t;
                    // Re-push for further backtracking if needed
                    p_stack.push((p_chars.clone(), t_chars.clone()));
                } else {
                    return false;
                }
            }
        }
    }
}

/// Regex matching against a string value.
/// SEC: all user-supplied patterns compiled with a 1MB size limit to prevent ReDoS.
/// PERF: compiled regexes are cached in a thread-local map (max 256 entries)
///       to avoid per-request recompilation.
pub(super) fn check_regex(actual: &Value, pattern: &Value) -> bool {
    let actual_str = value_as_str(actual);
    let pattern_str = value_as_str(pattern);

    match (actual_str, pattern_str) {
        (Some(text), Some(pat)) => {
            // For array values (from wildcard extraction), check any element
            if let Value::Array(arr) = actual {
                return arr.iter().any(|elem| {
                    value_as_str(elem)
                        .and_then(|s| compile_cached_regex(&pat).map(|re| re.is_match(&s)))
                        .unwrap_or(false)
                });
            }
            compile_cached_regex(&pat)
                .map(|re| re.is_match(&text))
                .unwrap_or(false)
        }
        _ => {
            // Handle array actual with string pattern
            if let Value::Array(arr) = actual {
                if let Some(pat) = value_as_str(pattern) {
                    return arr.iter().any(|elem| {
                        value_as_str(elem)
                            .and_then(|s| compile_cached_regex(&pat).map(|re| re.is_match(&s)))
                            .unwrap_or(false)
                    });
                }
            }
            false
        }
    }
}

/// Compile a regex pattern with thread-local caching and size limit.
/// SEC: patterns are compiled with a 1MB size limit to prevent ReDoS during compilation.
/// PERF: compiled regexes are cached in a thread-local map (max 256 entries)
///       to avoid per-request recompilation.
/// Returns None if the pattern is invalid or too complex.
pub(crate) fn compile_cached_regex(pattern: &str) -> Option<Regex> {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        /// Thread-local cache: pattern string → compiled Regex (None = invalid/too-complex).
        /// Bounded at 256 entries to prevent unbounded memory growth from malicious policies.
        static REGEX_CACHE: RefCell<HashMap<String, Option<Regex>>> =
            RefCell::new(HashMap::with_capacity(64));
    }

    REGEX_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(cached) = cache.get(pattern) {
            return cached.clone();
        }
        let compiled = regex::RegexBuilder::new(pattern)
            .size_limit(1_000_000) // 1MB limit prevents catastrophic backtracking
            .build()
            .ok();
        // Bound cache size: clear if over limit (simple eviction strategy)
        if cache.len() >= 256 {
            cache.clear();
        }
        cache.insert(pattern.to_string(), compiled.clone());
        compiled
    })
}

/// Check if actual contains the expected value (substring or array membership).
pub(super) fn check_contains(actual: &Value, expected: &Value) -> bool {
    match actual {
        Value::String(s) => {
            if let Some(needle) = value_as_str(expected) {
                return s.contains(&needle);
            }
            false
        }
        Value::Array(arr) => {
            // Check if any element matches or contains the expected value
            if let Some(needle) = value_as_str(expected) {
                return arr.iter().any(|elem| {
                    value_as_str(elem)
                        .map(|s| s.contains(&needle))
                        .unwrap_or(false)
                });
            }
            arr.iter().any(|elem| values_equal(elem, expected))
        }
        _ => false,
    }
}

pub(super) fn check_starts_with(actual: &Value, expected: &Value) -> bool {
    match (value_as_str(actual), value_as_str(expected)) {
        (Some(a), Some(e)) => a.starts_with(&e),
        _ => false,
    }
}

pub(super) fn check_ends_with(actual: &Value, expected: &Value) -> bool {
    match (value_as_str(actual), value_as_str(expected)) {
        (Some(a), Some(e)) => a.ends_with(&e),
        _ => false,
    }
}

pub(super) fn value_as_str(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}
