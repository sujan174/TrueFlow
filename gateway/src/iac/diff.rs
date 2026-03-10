//! Diff engine — compares a local config file against live gateway state.

use std::collections::BTreeMap;
use std::fmt;

use super::schema::{ConfigDoc, PolicySpec, TokenSpec};

/// A single change in the plan.
#[derive(Debug)]
pub struct Change {
    pub kind: ChangeKind,
    pub resource: ResourceType,
    pub name: String,
    pub detail: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Create,
    Update,
    /// Resource exists on server but not in the file (informational warning).
    Drift,
}

#[derive(Debug)]
pub enum ResourceType {
    Policy,
    Token,
    SpendCap,
}

/// The complete plan: a list of changes to apply.
pub struct Plan {
    pub changes: Vec<Change>,
}

impl Plan {
    /// Returns true if there are no changes to apply.
    pub fn is_empty(&self) -> bool {
        self.changes.iter().all(|c| c.kind == ChangeKind::Drift)
    }

    /// Count of creates.
    pub fn creates(&self) -> usize {
        self.changes
            .iter()
            .filter(|c| c.kind == ChangeKind::Create)
            .count()
    }

    /// Count of updates.
    pub fn updates(&self) -> usize {
        self.changes
            .iter()
            .filter(|c| c.kind == ChangeKind::Update)
            .count()
    }

    /// Count of drift warnings.
    pub fn drifts(&self) -> usize {
        self.changes
            .iter()
            .filter(|c| c.kind == ChangeKind::Drift)
            .count()
    }
}

impl fmt::Display for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.changes.is_empty() {
            return writeln!(f, "No changes. Live state matches the config file.");
        }

        for change in &self.changes {
            let symbol = match change.kind {
                ChangeKind::Create => "+",
                ChangeKind::Update => "~",
                ChangeKind::Drift => "?",
            };
            let rtype = match change.resource {
                ResourceType::Policy => "Policy",
                ResourceType::Token => "Token",
                ResourceType::SpendCap => "SpendCap",
            };
            if change.detail.is_empty() {
                writeln!(f, "  {} {:>10} {:30}", symbol, rtype, change.name)?;
            } else {
                writeln!(
                    f,
                    "  {} {:>10} {:30} ({})",
                    symbol, rtype, change.name, change.detail
                )?;
            }
        }

        writeln!(f)?;

        let creates = self.creates();
        let updates = self.updates();
        let drifts = self.drifts();

        write!(f, "{} to create, {} to update", creates, updates)?;
        if drifts > 0 {
            write!(
                f,
                ", {} on server but not in file (no action)",
                drifts
            )?;
        }
        writeln!(f, ".")
    }
}

/// Compare a local config document against live gateway state.
pub fn compute_plan(local: &ConfigDoc, live: &ConfigDoc) -> Plan {
    let mut changes = Vec::new();

    // ── Policies ──────────────────────────────────────────────────
    let live_policies: BTreeMap<&str, &PolicySpec> =
        live.policies.iter().map(|p| (p.name.as_str(), p)).collect();

    for local_policy in &local.policies {
        if let Some(live_policy) = live_policies.get(local_policy.name.as_str()) {
            // Exists — check for differences
            let diffs = diff_policy(local_policy, live_policy);
            if !diffs.is_empty() {
                changes.push(Change {
                    kind: ChangeKind::Update,
                    resource: ResourceType::Policy,
                    name: local_policy.name.clone(),
                    detail: diffs.join(", "),
                });
            }
        } else {
            changes.push(Change {
                kind: ChangeKind::Create,
                resource: ResourceType::Policy,
                name: local_policy.name.clone(),
                detail: String::new(),
            });
        }
    }

    // Policies on server but not in file
    let local_policy_names: std::collections::HashSet<&str> =
        local.policies.iter().map(|p| p.name.as_str()).collect();
    for live_policy in &live.policies {
        if !local_policy_names.contains(live_policy.name.as_str()) {
            changes.push(Change {
                kind: ChangeKind::Drift,
                resource: ResourceType::Policy,
                name: live_policy.name.clone(),
                detail: "exists on server, not in file".into(),
            });
        }
    }

    // ── Tokens ────────────────────────────────────────────────────
    let live_tokens: BTreeMap<&str, &TokenSpec> =
        live.tokens.iter().map(|t| (t.name.as_str(), t)).collect();

    for local_token in &local.tokens {
        if let Some(live_token) = live_tokens.get(local_token.name.as_str()) {
            // Token config diffs
            let diffs = diff_token(local_token, live_token);
            if !diffs.is_empty() {
                changes.push(Change {
                    kind: ChangeKind::Update,
                    resource: ResourceType::Token,
                    name: local_token.name.clone(),
                    detail: diffs.join(", "),
                });
            }

            // Spend cap diffs
            diff_spend_caps(
                &local_token.name,
                &local_token.spend_caps,
                &live_token.spend_caps,
                &mut changes,
            );
        } else {
            changes.push(Change {
                kind: ChangeKind::Create,
                resource: ResourceType::Token,
                name: local_token.name.clone(),
                detail: String::new(),
            });
            // Also plan spend caps for new tokens
            for (period, limit) in &local_token.spend_caps {
                changes.push(Change {
                    kind: ChangeKind::Create,
                    resource: ResourceType::SpendCap,
                    name: format!("{}:{}", local_token.name, period),
                    detail: format!("${:.2}", limit),
                });
            }
        }
    }

    // Tokens on server but not in file
    let local_token_names: std::collections::HashSet<&str> =
        local.tokens.iter().map(|t| t.name.as_str()).collect();
    for live_token in &live.tokens {
        if !local_token_names.contains(live_token.name.as_str()) {
            changes.push(Change {
                kind: ChangeKind::Drift,
                resource: ResourceType::Token,
                name: live_token.name.clone(),
                detail: "exists on server, not in file".into(),
            });
        }
    }

    Plan { changes }
}

// ── Helpers ──────────────────────────────────────────────────────

fn diff_policy(local: &PolicySpec, live: &PolicySpec) -> Vec<String> {
    let mut diffs = Vec::new();
    if local.mode != live.mode {
        diffs.push(format!("mode: {} → {}", live.mode, local.mode));
    }
    if local.phase != live.phase {
        diffs.push(format!("phase: {} → {}", live.phase, local.phase));
    }
    if local.rules != live.rules {
        diffs.push("rules changed".into());
    }
    if local.retry != live.retry {
        diffs.push("retry changed".into());
    }
    diffs
}

fn diff_token(local: &TokenSpec, live: &TokenSpec) -> Vec<String> {
    let mut diffs = Vec::new();
    if local.upstream_url != live.upstream_url {
        diffs.push(format!(
            "upstream: {} → {}",
            live.upstream_url, local.upstream_url
        ));
    }
    let mut local_policies = local.policies.clone();
    let mut live_policies = live.policies.clone();
    local_policies.sort();
    live_policies.sort();
    if local_policies != live_policies {
        diffs.push("policies changed".into());
    }
    if local.log_level != live.log_level {
        diffs.push(format!(
            "log_level: {} → {}",
            live.log_level.as_deref().unwrap_or("default"),
            local.log_level.as_deref().unwrap_or("default")
        ));
    }
    diffs
}

fn diff_spend_caps(
    token_name: &str,
    local_caps: &BTreeMap<String, f64>,
    live_caps: &BTreeMap<String, f64>,
    changes: &mut Vec<Change>,
) {
    for (period, &local_limit) in local_caps {
        if let Some(&live_limit) = live_caps.get(period) {
            if (local_limit - live_limit).abs() > 0.001 {
                changes.push(Change {
                    kind: ChangeKind::Update,
                    resource: ResourceType::SpendCap,
                    name: format!("{}:{}", token_name, period),
                    detail: format!("${:.2} → ${:.2}", live_limit, local_limit),
                });
            }
        } else {
            changes.push(Change {
                kind: ChangeKind::Create,
                resource: ResourceType::SpendCap,
                name: format!("{}:{}", token_name, period),
                detail: format!("${:.2}", local_limit),
            });
        }
    }
}
