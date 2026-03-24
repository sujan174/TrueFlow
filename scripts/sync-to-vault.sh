#!/bin/bash
# Sync docs from AILink project to vault

SOURCE="/Users/sujan/Developer/AILink"
VAULT="/Users/sujan/Developer/trueflow-vault"

echo "Syncing TrueFlow docs to vault..."

# Root docs
cp "$SOURCE/README.md" "$VAULT/README.md"
cp "$SOURCE/CLAUDE.md" "$VAULT/CLAUDE.md"
cp "$SOURCE/SECURITY.md" "$VAULT/SECURITY.md"

# Features
cp "$SOURCE/GATEWAY_FEATURES.md" "$VAULT/features/"

# Dashboards
cp "$SOURCE/DASHBOARD_INVENTORY.md" "$VAULT/dashboards/"
cp "$SOURCE/ANALYTICS_FEASIBILITY_REPORT.md" "$VAULT/dashboards/" 2>/dev/null || true

# Architecture
cp "$SOURCE/docs/gateway-architecture-deep-dive.md" "$VAULT/architecture/"
cp "$SOURCE/docs/reference/architecture.md" "$VAULT/architecture/System-Architecture.md"
cp "$SOURCE/docs/reference/security.md" "$VAULT/architecture/"

# API Reference
cp "$SOURCE/docs/reference/api.md" "$VAULT/api-reference/"

# Guides
cp "$SOURCE/docs/guides/"*.md "$VAULT/guides/"
cp "$SOURCE/docs/getting-started/"*.md "$VAULT/guides/"

# Deployment
cp "$SOURCE/docs/deployment/"*.md "$VAULT/deployment/"

# SDKs
cp "$SOURCE/docs/sdks/"*.md "$VAULT/sdks/"
cp "$SOURCE/sdk/python/README.md" "$VAULT/sdks/Python-SDK.md"
cp "$SOURCE/sdk/typescript/README.md" "$VAULT/sdks/TypeScript-SDK.md"

echo "✓ Docs synced to vault"
echo "Run: open -a Obsidian '$VAULT'"