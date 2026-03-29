//! Integration tests for secret references API.
//!
//! These tests verify:
//! 1. SecretReference model serialization/deserialization
//! 2. CreateSecretReferenceRequest validation
//! 3. SecretAccessResult enum behavior
//! 4. DTO transformations

use gateway::models::secret_reference::{
    CreateSecretReferenceRequest, SecretAccessResult, SecretReference, SecretReferenceFilter,
    UpdateSecretReferenceRequest,
};
use gateway::vault::VaultBackend;
use chrono::Utc;
use serde_json;
use uuid::Uuid;

// ── Model Serialization Tests ────────────────────────────────────────

/// Test that SecretReference serializes with all expected fields.
#[test]
fn test_secret_reference_serialization() {
    let now = Utc::now();
    let sr = SecretReference {
        id: Uuid::nil(),
        project_id: Uuid::nil(),
        name: "test-secret".to_string(),
        description: Some("Test secret reference".to_string()),
        vault_backend: VaultBackend::AwsSecretsManager,
        external_ref: "arn:aws:secretsmanager:us-east-1:123456789:secret:my-secret".to_string(),
        vault_config_id: None,
        provider: Some("openai".to_string()),
        injection_mode: "bearer".to_string(),
        injection_header: "Authorization".to_string(),
        allowed_team_ids: None,
        allowed_user_ids: None,
        last_accessed_at: Some(now),
        last_rotated_at: None,
        version: Some("v1".to_string()),
        is_active: true,
        created_at: now,
        updated_at: now,
        created_by: None,
    };

    let json = serde_json::to_value(&sr).unwrap();

    // Verify all expected fields are present
    assert!(json.get("id").is_some());
    assert!(json.get("project_id").is_some());
    assert!(json.get("name").is_some());
    assert!(json.get("vault_backend").is_some());
    assert!(json.get("external_ref").is_some());
    assert!(json.get("injection_mode").is_some());
    assert!(json.get("is_active").is_some());

    // Verify values
    assert_eq!(json["name"], "test-secret");
    assert_eq!(json["vault_backend"], "aws_secrets_manager");
    assert_eq!(json["injection_mode"], "bearer");
    assert_eq!(json["is_active"], true);
}

/// Test that SecretReference deserializes correctly from JSON.
#[test]
fn test_secret_reference_deserialization() {
    let json = serde_json::json!({
        "id": "00000000-0000-0000-0000-000000000000",
        "project_id": "00000000-0000-0000-0000-000000000001",
        "name": "vault-secret",
        "description": "HashiCorp Vault secret",
        "vault_backend": "hashicorp_vault_kv",
        "external_ref": "secret/data/openai/api-key",
        "vault_config_id": null,
        "provider": "anthropic",
        "injection_mode": "header",
        "injection_header": "X-API-Key",
        "allowed_team_ids": null,
        "allowed_user_ids": null,
        "last_accessed_at": null,
        "last_rotated_at": null,
        "version": null,
        "is_active": true,
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-01T00:00:00Z",
        "created_by": null
    });

    let sr: SecretReference = serde_json::from_value(json).unwrap();

    assert_eq!(sr.name, "vault-secret");
    assert_eq!(sr.vault_backend, VaultBackend::HashicorpVaultKv);
    assert_eq!(sr.external_ref, "secret/data/openai/api-key");
    assert_eq!(sr.provider, Some("anthropic".to_string()));
    assert_eq!(sr.injection_mode, "header");
    assert_eq!(sr.injection_header, "X-API-Key");
}

// ── Create Request Tests ──────────────────────────────────────────────

/// Test CreateSecretReferenceRequest with minimal required fields.
#[test]
fn test_create_request_minimal_fields() {
    let json = serde_json::json!({
        "name": "minimal-secret",
        "vault_backend": "aws_secrets_manager",
        "external_ref": "arn:aws:secretsmanager:us-east-1:123:secret:minimal"
    });

    let req: CreateSecretReferenceRequest = serde_json::from_value(json).unwrap();

    assert_eq!(req.name, "minimal-secret");
    assert_eq!(req.vault_backend, "aws_secrets_manager");
    assert_eq!(req.external_ref, "arn:aws:secretsmanager:us-east-1:123:secret:minimal");
    assert_eq!(req.description, None);
    assert_eq!(req.provider, None);
    assert_eq!(req.injection_mode, None);
    assert_eq!(req.injection_header, None);
}

/// Test CreateSecretReferenceRequest with all fields.
#[test]
fn test_create_request_all_fields() {
    let team_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let vault_config_id = Uuid::new_v4();

    let json = serde_json::json!({
        "name": "full-secret",
        "description": "A complete secret reference",
        "vault_backend": "hashicorp_vault_kv",
        "external_ref": "secret/data/api/keys",
        "vault_config_id": vault_config_id,
        "provider": "openai",
        "injection_mode": "bearer",
        "injection_header": "Authorization",
        "allowed_team_ids": [team_id],
        "allowed_user_ids": [user_id]
    });

    let req: CreateSecretReferenceRequest = serde_json::from_value(json).unwrap();

    assert_eq!(req.name, "full-secret");
    assert_eq!(req.description, Some("A complete secret reference".to_string()));
    assert_eq!(req.vault_backend, "hashicorp_vault_kv");
    assert_eq!(req.vault_config_id, Some(vault_config_id));
    assert_eq!(req.provider, Some("openai".to_string()));
    assert_eq!(req.injection_mode, Some("bearer".to_string()));
    assert_eq!(req.injection_header, Some("Authorization".to_string()));
    assert_eq!(req.allowed_team_ids, Some(vec![team_id]));
    assert_eq!(req.allowed_user_ids, Some(vec![user_id]));
}

// ── Update Request Tests ──────────────────────────────────────────────

/// Test UpdateSecretReferenceRequest with partial fields.
#[test]
fn test_update_request_partial_fields() {
    let json = serde_json::json!({
        "name": "updated-name",
        "is_active": false
    });

    let req: UpdateSecretReferenceRequest = serde_json::from_value(json).unwrap();

    assert_eq!(req.name, Some("updated-name".to_string()));
    assert_eq!(req.is_active, Some(false));
    assert_eq!(req.description, None);
    assert_eq!(req.external_ref, None);
    assert_eq!(req.provider, None);
}

/// Test UpdateSecretReferenceRequest defaults to all None.
#[test]
fn test_update_request_defaults() {
    let req = UpdateSecretReferenceRequest::default();

    assert!(req.name.is_none());
    assert!(req.description.is_none());
    assert!(req.external_ref.is_none());
    assert!(req.vault_config_id.is_none());
    assert!(req.provider.is_none());
    assert!(req.injection_mode.is_none());
    assert!(req.injection_header.is_none());
    assert!(req.allowed_team_ids.is_none());
    assert!(req.allowed_user_ids.is_none());
    assert!(req.version.is_none());
    assert!(req.is_active.is_none());
}

// ── Filter Tests ──────────────────────────────────────────────────────

/// Test SecretReferenceFilter defaults to no filters.
#[test]
fn test_filter_defaults() {
    let filter = SecretReferenceFilter::default();

    assert!(filter.vault_backend.is_none());
    assert!(filter.provider.is_none());
    assert!(filter.is_active.is_none());
}

/// Test SecretReferenceFilter with all fields.
#[test]
fn test_filter_with_fields() {
    let json = serde_json::json!({
        "vault_backend": "azure_key_vault",
        "provider": "google",
        "is_active": true
    });

    let filter: SecretReferenceFilter = serde_json::from_value(json).unwrap();

    assert_eq!(filter.vault_backend, Some("azure_key_vault".to_string()));
    assert_eq!(filter.provider, Some("google".to_string()));
    assert_eq!(filter.is_active, Some(true));
}

// ── Access Control Tests ──────────────────────────────────────────────

/// Test SecretAccessResult enum variants.
#[test]
fn test_secret_access_result_variants() {
    // Verify the three access states
    let granted = SecretAccessResult::Granted;
    let denied = SecretAccessResult::Denied;
    let not_found = SecretAccessResult::NotFound;

    // Test equality
    assert_eq!(granted, SecretAccessResult::Granted);
    assert_eq!(denied, SecretAccessResult::Denied);
    assert_eq!(not_found, SecretAccessResult::NotFound);

    // Test inequality
    assert_ne!(granted, denied);
    assert_ne!(denied, not_found);
    assert_ne!(not_found, granted);
}

/// Test SecretAccessResult for use in access control logic.
#[test]
fn test_secret_access_result_matching() {
    fn check_access(result: SecretAccessResult) -> bool {
        matches!(result, SecretAccessResult::Granted)
    }

    assert!(check_access(SecretAccessResult::Granted));
    assert!(!check_access(SecretAccessResult::Denied));
    assert!(!check_access(SecretAccessResult::NotFound));
}

// ── Vault Backend Tests ───────────────────────────────────────────────

/// Test VaultBackend serialization for supported backends.
#[test]
fn test_vault_backend_serialization() {
    let backends = vec![
        (VaultBackend::AwsSecretsManager, "aws_secrets_manager"),
        (VaultBackend::HashicorpVaultKv, "hashicorp_vault_kv"),
        (VaultBackend::AzureKeyVault, "azure_key_vault"),
    ];

    for (backend, expected_str) in backends {
        let json = serde_json::to_string(&backend).unwrap();
        assert!(json.contains(expected_str), "Expected {} in {}", expected_str, json);

        let deserialized: VaultBackend = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, backend);
    }
}

/// Test VaultBackend deserialization from string values.
#[test]
fn test_vault_backend_deserialization() {
    let test_cases = vec![
        ("\"aws_secrets_manager\"", VaultBackend::AwsSecretsManager),
        ("\"hashicorp_vault_kv\"", VaultBackend::HashicorpVaultKv),
        ("\"azure_key_vault\"", VaultBackend::AzureKeyVault),
    ];

    for (json_str, expected) in test_cases {
        let backend: VaultBackend = serde_json::from_str(json_str).unwrap();
        assert_eq!(backend, expected);
    }
}

// ── Validation Tests ───────────────────────────────────────────────────

/// Test that valid injection modes are recognized.
#[test]
fn test_valid_injection_modes() {
    let valid_modes = vec!["bearer", "header", "query", "none"];

    for mode in valid_modes {
        // In production, this validation happens in the handler
        // Here we verify the values are valid strings
        let json = serde_json::json!({
            "name": format!("{}-secret", mode),
            "vault_backend": "aws_secrets_manager",
            "external_ref": "test-ref",
            "injection_mode": mode
        });

        let req: CreateSecretReferenceRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.injection_mode, Some(mode.to_string()));
    }
}

/// Test that valid vault backends are recognized.
#[test]
fn test_valid_vault_backends() {
    let valid_backends = vec![
        "aws_secrets_manager",
        "hashicorp_vault",
        "hashicorp_vault_kv",
        "azure_key_vault",
    ];

    for backend in valid_backends {
        let json = serde_json::json!({
            "name": format!("{}-secret", backend),
            "vault_backend": backend,
            "external_ref": "test-ref"
        });

        let req: CreateSecretReferenceRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.vault_backend, backend);
    }
}

/// Test allowed_team_ids and allowed_user_ids serialization.
#[test]
fn test_access_allowlists() {
    let team1 = Uuid::new_v4();
    let team2 = Uuid::new_v4();
    let user1 = Uuid::new_v4();

    let json = serde_json::json!({
        "name": "restricted-secret",
        "vault_backend": "aws_secrets_manager",
        "external_ref": "restricted-ref",
        "allowed_team_ids": [team1, team2],
        "allowed_user_ids": [user1]
    });

    let req: CreateSecretReferenceRequest = serde_json::from_value(json).unwrap();

    assert_eq!(req.allowed_team_ids, Some(vec![team1, team2]));
    assert_eq!(req.allowed_user_ids, Some(vec![user1]));
}

/// Test that empty allowlists are valid (meaning all teams/users allowed).
#[test]
fn test_empty_allowlists_mean_all_allowed() {
    let json = serde_json::json!({
        "name": "open-secret",
        "vault_backend": "aws_secrets_manager",
        "external_ref": "open-ref",
        "allowed_team_ids": [],
        "allowed_user_ids": []
    });

    let req: CreateSecretReferenceRequest = serde_json::from_value(json).unwrap();

    // Empty vectors are valid - they mean "no restrictions"
    assert_eq!(req.allowed_team_ids, Some(vec![]));
    assert_eq!(req.allowed_user_ids, Some(vec![]));
}