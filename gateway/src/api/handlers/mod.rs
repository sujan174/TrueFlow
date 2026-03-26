mod analytics;
mod approvals;
mod audit;
mod auth;
mod credentials;
pub mod dtos;
mod helpers;
mod model_access;
mod notifications;
mod policies;
mod pricing;
mod projects;
mod services;
mod sessions;
mod settings;
mod spend_caps;
mod teams;
mod tokens;
mod users;
mod webhooks;

// ── Re-exports: DTOs ────────────────────────────────────────
pub use self::dtos::*;

// ── Re-exports: Helpers ─────────────────────────────────────
pub use self::helpers::verify_project_ownership;

// ── Re-exports: Projects ────────────────────────────────────
pub use self::projects::{
    create_project, delete_project, list_projects, purge_project_data, update_project,
};

// ── Re-exports: Tokens ──────────────────────────────────────
pub use self::tokens::{
    bulk_create_tokens, bulk_revoke_tokens, create_token, get_circuit_breaker, get_token_usage,
    list_tokens, revoke_token, update_circuit_breaker,
};

// ── Re-exports: Approvals ───────────────────────────────────
pub use self::approvals::{decide_approval, list_approvals};

// ── Re-exports: Audit ───────────────────────────────────────
pub use self::audit::{get_audit_log, list_audit_logs, stream_audit_logs};

// ── Re-exports: Sessions ────────────────────────────────────
pub use self::sessions::{
    get_session, get_session_entity, list_sessions, set_session_spend_cap, update_session_status,
};

// ── Re-exports: Policies ────────────────────────────────────
pub use self::policies::{
    create_policy, delete_policy, list_policies, list_policy_versions, update_policy,
};

// ── Re-exports: Credentials ─────────────────────────────────
pub use self::credentials::{create_credential, delete_credential, list_credentials};

// ── Re-exports: Notifications ───────────────────────────────
pub use self::notifications::{
    count_unread_notifications, list_notifications, mark_all_notifications_read,
    mark_notification_read,
};

// ── Re-exports: Services ────────────────────────────────────
pub use self::services::{create_service, delete_service, list_services};

// ── Re-exports: Auth / API Keys ─────────────────────────────
pub use self::auth::{create_api_key, list_api_keys, revoke_api_key, update_api_key, whoami};

// ── Re-exports: Analytics ───────────────────────────────────
pub use self::analytics::{
    get_analytics_experiments, get_analytics_summary, get_analytics_timeseries,
    get_budget_burn_rate, get_budget_health, get_cache_hit_rate_timeseries, get_cache_latency_comparison,
    get_cache_summary, get_cost_efficiency, get_cost_latency_scatter, get_data_residency,
    get_error_breakdown, get_error_logs, get_error_summary, get_error_timeseries,
    get_guardrail_triggers, get_hitl_latency, get_hitl_rejection_reasons, get_hitl_summary,
    get_hitl_volume, get_latency_timeseries, get_model_cache_efficiency, get_model_error_rates,
    get_model_latency, get_model_stats, get_model_usage_timeseries, get_org_usage, get_pii_breakdown,
    get_policy_actions, get_requests_per_user, get_security_summary, get_shadow_policies,
    get_spend_breakdown, get_spend_timeseries, get_token_alerts, get_token_analytics, get_token_latency,
    get_token_spend, get_token_status, get_token_volume, get_top_cached_queries, get_traffic_timeseries,
    get_upstream_health, get_user_engagement, get_user_growth, get_user_spend,
};

// ── Re-exports: Spend Caps ──────────────────────────────────
pub use self::spend_caps::{delete_spend_cap, get_spend_caps, upsert_spend_cap};

// ── Re-exports: Webhooks ────────────────────────────────────
pub use self::webhooks::{create_webhook, delete_webhook, list_webhooks, test_webhook};

// ── Re-exports: Pricing ─────────────────────────────────────
pub use self::pricing::{delete_pricing, list_pricing, upsert_pricing};

// ── Re-exports: Settings ────────────────────────────────────
pub use self::settings::{
    flush_cache, get_anomaly_events, get_cache_stats, get_settings, rehydrate_pii_tokens,
    update_settings,
};

// ── Re-exports: Model Access Groups ─────────────────────────
pub use self::model_access::{
    create_model_access_group, delete_model_access_group, list_model_access_groups,
    update_model_access_group,
};

// ── Re-exports: Teams ───────────────────────────────────────
pub use self::teams::{
    add_team_member, create_team, delete_team, get_team_spend, list_team_members, list_teams,
    remove_team_member, update_team,
};

// ── Re-exports: Users (Supabase Auth) ───────────────────────
pub use self::users::{get_current_user, get_user, list_users, sync_user, update_last_project, update_user_role};
