use axum::http::StatusCode;

/// Roles supported by the RBAC system.
/// Matches the `role` column in the `api_keys` table.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    Admin,
    Editor,
    Viewer,
    Custom(String),
}

impl Role {
    #[allow(dead_code, clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "admin" => Role::Admin,
            "editor" => Role::Editor,
            "viewer" => Role::Viewer,
            other => Role::Custom(other.to_string()),
        }
    }

    /// Check if this role has the required permission level.
    pub fn has_permission(&self, required: &Permission) -> bool {
        match required {
            Permission::Read => true, // all roles can read
            Permission::Write => matches!(self, Role::Admin | Role::Editor),
            Permission::Admin => matches!(self, Role::Admin),
        }
    }
}

/// Permission levels for RBAC enforcement.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Permission {
    Read,
    Write,
    Admin,
}

/// Scope-based access control.
/// Scopes are fine-grained permissions beyond the role level.
/// Format: "resource:action" (e.g., "tokens:write", "projects:read")
#[allow(dead_code)]
pub fn check_scope(scopes: &[String], required_scope: &str) -> bool {
    // Wildcard scope grants all access
    if scopes.iter().any(|s| s == "*") {
        return true;
    }

    // Direct match
    if scopes.iter().any(|s| s == required_scope) {
        return true;
    }

    // Resource wildcard (e.g., "tokens:*" matches "tokens:write")
    let parts: Vec<&str> = required_scope.split(':').collect();
    if parts.len() == 2 {
        let resource_wildcard = format!("{}:*", parts[0]);
        if scopes.iter().any(|s| s == &resource_wildcard) {
            return true;
        }
    }

    false
}

/// RBAC context extracted from the API key authentication.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RbacContext {
    pub role: Role,
    pub scopes: Vec<String>,
    pub org_id: uuid::Uuid,
    pub user_id: Option<String>,
}

impl RbacContext {
    /// Check if this context has the required role-level permission.
    pub fn has_permission(&self, required: &Permission) -> bool {
        self.role.has_permission(required)
    }

    /// Check if this context has the required scope.
    pub fn has_scope(&self, required_scope: &str) -> bool {
        // Admin role bypasses scope checks
        if self.role == Role::Admin {
            return true;
        }
        check_scope(&self.scopes, required_scope)
    }

    /// Check both role permission and scope. Returns 403 error message if denied.
    pub fn require(&self, permission: &Permission, scope: &str) -> Result<(), String> {
        if !self.has_permission(permission) {
            return Err(format!(
                "Insufficient role: {:?} required, but user has {:?}",
                permission, self.role
            ));
        }
        if !self.has_scope(scope) {
            return Err(format!("Insufficient scope: '{}' required", scope));
        }
        Ok(())
    }
}

/// Helper to check RBAC in API handlers and return 403 on failure.
#[allow(dead_code)]
pub fn enforce(
    ctx: &RbacContext,
    permission: &Permission,
    scope: &str,
) -> Result<(), (StatusCode, String)> {
    ctx.require(permission, scope).map_err(|msg| {
        tracing::warn!(
            role = ?ctx.role,
            scope = scope,
            org_id = %ctx.org_id,
            "RBAC access denied: {}",
            msg
        );
        (
            StatusCode::FORBIDDEN,
            serde_json::json!({
                "error": "forbidden",
                "message": msg
            })
            .to_string(),
        )
    })
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_from_str() {
        assert_eq!(Role::from_str("admin"), Role::Admin);
        assert_eq!(Role::from_str("Admin"), Role::Admin);
        assert_eq!(Role::from_str("editor"), Role::Editor);
        assert_eq!(Role::from_str("viewer"), Role::Viewer);
        assert_eq!(
            Role::from_str("custom_role"),
            Role::Custom("custom_role".into())
        );
    }

    #[test]
    fn test_admin_has_all_permissions() {
        let admin = Role::Admin;
        assert!(admin.has_permission(&Permission::Read));
        assert!(admin.has_permission(&Permission::Write));
        assert!(admin.has_permission(&Permission::Admin));
    }

    #[test]
    fn test_editor_has_read_write() {
        let editor = Role::Editor;
        assert!(editor.has_permission(&Permission::Read));
        assert!(editor.has_permission(&Permission::Write));
        assert!(!editor.has_permission(&Permission::Admin));
    }

    #[test]
    fn test_viewer_has_read_only() {
        let viewer = Role::Viewer;
        assert!(viewer.has_permission(&Permission::Read));
        assert!(!viewer.has_permission(&Permission::Write));
        assert!(!viewer.has_permission(&Permission::Admin));
    }

    #[test]
    fn test_check_scope_direct_match() {
        let scopes = vec!["tokens:read".to_string(), "tokens:write".to_string()];
        assert!(check_scope(&scopes, "tokens:read"));
        assert!(check_scope(&scopes, "tokens:write"));
        assert!(!check_scope(&scopes, "projects:write"));
    }

    #[test]
    fn test_check_scope_wildcard() {
        let scopes = vec!["*".to_string()];
        assert!(check_scope(&scopes, "tokens:read"));
        assert!(check_scope(&scopes, "anything:write"));
    }

    #[test]
    fn test_check_scope_resource_wildcard() {
        let scopes = vec!["tokens:*".to_string()];
        assert!(check_scope(&scopes, "tokens:read"));
        assert!(check_scope(&scopes, "tokens:write"));
        assert!(!check_scope(&scopes, "projects:read"));
    }

    #[test]
    fn test_rbac_context_admin_bypasses_scopes() {
        let ctx = RbacContext {
            role: Role::Admin,
            scopes: vec![], // no explicit scopes
            org_id: uuid::Uuid::new_v4(),
            user_id: None,
        };
        assert!(ctx.has_scope("anything:read"));
        assert!(ctx.has_scope("anything:write"));
    }

    #[test]
    fn test_rbac_context_require_success() {
        let ctx = RbacContext {
            role: Role::Editor,
            scopes: vec!["tokens:write".to_string()],
            org_id: uuid::Uuid::new_v4(),
            user_id: None,
        };
        assert!(ctx.require(&Permission::Write, "tokens:write").is_ok());
    }

    #[test]
    fn test_rbac_context_require_insufficient_role() {
        let ctx = RbacContext {
            role: Role::Viewer,
            scopes: vec!["tokens:write".to_string()],
            org_id: uuid::Uuid::new_v4(),
            user_id: None,
        };
        assert!(ctx.require(&Permission::Write, "tokens:write").is_err());
    }

    #[test]
    fn test_rbac_context_require_insufficient_scope() {
        let ctx = RbacContext {
            role: Role::Editor,
            scopes: vec!["tokens:read".to_string()],
            org_id: uuid::Uuid::new_v4(),
            user_id: None,
        };
        assert!(ctx.require(&Permission::Write, "tokens:write").is_err());
    }

    #[test]
    fn test_enforce_returns_403() {
        let ctx = RbacContext {
            role: Role::Viewer,
            scopes: vec![],
            org_id: uuid::Uuid::new_v4(),
            user_id: None,
        };
        let result = enforce(&ctx, &Permission::Write, "tokens:write");
        assert!(result.is_err());
        let (status, body) = result.unwrap_err();
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(body.contains("forbidden"));
    }
}
