mod actions;
mod evaluate;
mod operators;

#[cfg(test)]
mod tests;

use crate::models::policy::{Action, EvalOutcome, Phase, Policy, PolicyMode, TriggeredAction};

use super::fields::RequestContext;

use self::actions::action_name;
pub use self::actions::{evaluate_tool_scope, extract_tool_names};
pub use self::evaluate::evaluate_condition;
#[allow(unused_imports)] // Used by binary crate (smart_router.rs)
pub(crate) use self::operators::{compile_cached_regex, glob_match};

/// Evaluate all policies against a request context.
///
/// Returns an `EvalOutcome` containing:
/// - `actions`         — blocking rules to execute before responding
/// - `async_triggered` — async rules (rule.async_check=true) to run after responding
/// - `shadow_violations` — shadow-mode matches (logged, not enforced)
///
/// # Action Ordering and Priority (MED-6)
///
/// Actions are evaluated and collected in a specific order with the following rules:
///
/// 1. **Rule Order**: Rules are processed in the order they appear in each policy's `rules` array.
///    Earlier rules are evaluated before later ones.
///
/// 2. **Action Order**: Within a single rule, actions in `then` are collected in the order
///    specified. This matters when multiple actions modify the request (e.g., redact + route).
///
/// 3. **Policy Order**: Policies are processed in the order they appear in the `policies` array.
///    Actions from multiple policies can accumulate.
///
/// 4. **Deny Short-Circuit**: When a `Deny` action is encountered, processing stops immediately
///    after collecting that rule's actions. No further rules or policies are evaluated.
///    This ensures deny is applied efficiently and prevents unnecessary processing.
///
/// 5. **Shadow Mode**: In shadow mode, actions are logged but not executed. All rules are
///    evaluated (no short-circuit) to provide complete visibility.
///
/// ## Example Evaluation Order
///
/// ```text
/// Policy A, Rule 1: [Redact, Route]  → collected
/// Policy A, Rule 2: [Deny]            → collected, then short-circuit
/// Policy B, Rule 1: [RateLimit]       → NOT evaluated (after deny)
/// ```
///
/// The final action list would be: [Redact, Route, Deny]
///
/// HIGH-4: Short-circuits when a deny action is triggered to avoid processing
/// unnecessary rules. This ensures deny is applied immediately and efficiently.
pub fn evaluate_policies(
    policies: &[Policy],
    ctx: &RequestContext<'_>,
    phase: &Phase,
) -> EvalOutcome {
    let mut outcome = EvalOutcome::default();
    let mut has_deny = false;

    for policy in policies {
        // Skip policies not matching the current phase
        if policy.phase != *phase {
            continue;
        }

        // HIGH-4: Stop processing if we already have a deny
        if has_deny {
            break;
        }

        for (rule_idx, rule) in policy.rules.iter().enumerate() {
            let matched = evaluate_condition(&rule.when, ctx);

            if matched {
                match policy.mode {
                    PolicyMode::Enforce => {
                        for action in &rule.then {
                            // HIGH-4: Track deny for short-circuit
                            if matches!(action, Action::Deny { .. }) {
                                has_deny = true;
                            }
                            let ta = TriggeredAction {
                                policy_id: policy.id,
                                policy_name: policy.name.clone(),
                                rule_index: rule_idx,
                                action: action.clone(),
                            };
                            if rule.async_check {
                                // Non-blocking: queue for background evaluation
                                outcome.async_triggered.push(ta);
                            } else {
                                // Blocking: execute before returning the response
                                outcome.actions.push(ta);
                            }
                        }
                        // HIGH-4: Short-circuit after processing this rule's actions
                        // if it contains a deny
                        if has_deny {
                            break;
                        }
                    }
                    PolicyMode::Shadow => {
                        let desc = format!(
                            "policy '{}' rule #{}: would trigger {:?}",
                            policy.name,
                            rule_idx,
                            rule.then.iter().map(action_name).collect::<Vec<_>>()
                        );
                        tracing::info!(
                            shadow = true,
                            policy = %policy.name,
                            rule_index = rule_idx,
                            "{}", desc
                        );
                        outcome.shadow_violations.push(desc);
                    }
                }
            }
        }
    }

    outcome
}

// ── Condition Evaluation ─────────────────────────────────────
