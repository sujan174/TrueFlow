mod actions;
mod evaluate;
mod operators;

#[cfg(test)]
mod tests;

use crate::models::policy::{EvalOutcome, Phase, Policy, PolicyMode, TriggeredAction};

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
pub fn evaluate_policies(
    policies: &[Policy],
    ctx: &RequestContext<'_>,
    phase: &Phase,
) -> EvalOutcome {
    let mut outcome = EvalOutcome::default();

    for policy in policies {
        // Skip policies not matching the current phase
        if policy.phase != *phase {
            continue;
        }

        for (rule_idx, rule) in policy.rules.iter().enumerate() {
            let matched = evaluate_condition(&rule.when, ctx);

            if matched {
                match policy.mode {
                    PolicyMode::Enforce => {
                        for action in &rule.then {
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
