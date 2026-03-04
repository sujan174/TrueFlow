#!/usr/bin/env bash
# scripts/ci_security_check.sh
# ------------------------------------------------------------
# TrueFlow CI Security Check
# Runs cargo audit + npm audit and fails the build on HIGH/CRITICAL issues.
# Add to your CI pipeline: bash scripts/ci_security_check.sh
# ------------------------------------------------------------

set -euo pipefail

GATEWAY_DIR="$(dirname "$0")/../gateway"
DASHBOARD_DIR="$(dirname "$0")/../dashboard"

RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m'

echo "═══════════════════════════════════════════════════════"
echo "  TrueFlow Security Audit — CI Check"
echo "═══════════════════════════════════════════════════════"
echo ""

# ── 1. Rust / Cargo Audit ───────────────────────────────────
echo "▶ Running cargo audit (gateway)…"
cd "$GATEWAY_DIR"

if ! command -v cargo-audit &>/dev/null; then
    echo -e "${YELLOW}  cargo-audit not installed. Installing…${NC}"
    cargo install cargo-audit --quiet
fi

CARGO_AUDIT_OUT=$(cargo audit 2>&1)
VULNERABILITIES=$(echo "$CARGO_AUDIT_OUT" | grep -c "^Crate:" || true)
WARNINGS=$(echo "$CARGO_AUDIT_OUT" | grep -c "^Warning:" || true)

echo "$CARGO_AUDIT_OUT"

if echo "$CARGO_AUDIT_OUT" | grep -q "^error\["; then
    echo ""
    echo -e "${RED}✖  cargo audit: VULNERABILITIES FOUND — blocking CI.${NC}"
    exit 1
fi

if [ "$VULNERABILITIES" -gt 0 ]; then
    echo ""
    echo -e "${RED}✖  cargo audit: $VULNERABILITIES CVE(s) detected. Fix before merging.${NC}"
    exit 1
else
    echo -e "${GREEN}✔  cargo audit: no exploitable vulnerabilities.${NC}"
fi

if [ "$WARNINGS" -gt 0 ]; then
    echo -e "${YELLOW}⚠  cargo audit: $WARNINGS unmaintained crate warning(s). Track for future action.${NC}"
fi

# ── 2. npm Audit (dashboard) ────────────────────────────────
echo ""
echo "▶ Running npm audit (dashboard)…"
cd "$DASHBOARD_DIR"

NPM_AUDIT_OUT=$(npm audit --json 2>/dev/null || true)
CRITICAL=$(echo "$NPM_AUDIT_OUT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('metadata',{}).get('vulnerabilities',{}).get('critical',0))" 2>/dev/null || echo "0")
HIGH=$(echo "$NPM_AUDIT_OUT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('metadata',{}).get('vulnerabilities',{}).get('high',0))" 2>/dev/null || echo "0")
MODERATE=$(echo "$NPM_AUDIT_OUT" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('metadata',{}).get('vulnerabilities',{}).get('moderate',0))" 2>/dev/null || echo "0")

echo "  Critical: $CRITICAL | High: $HIGH | Moderate: $MODERATE"

# Fail ONLY on critical or high vulnerabilities in production dependencies
if [ "$CRITICAL" -gt 0 ] || [ "$HIGH" -gt 0 ]; then
    # Check if they are in production or devDependencies
    PROD_ISSUES=$(npm audit --production 2>&1 | grep -c "found [1-9]" || true)
    if [ "$PROD_ISSUES" -gt 0 ]; then
        echo -e "${RED}✖  npm audit: CRITICAL/HIGH vulnerabilities in PRODUCTION dependencies!${NC}"
        npm audit --production
        exit 1
    else
        echo -e "${YELLOW}⚠  npm audit: Critical/High findings are in devDependencies only — not blocking.${NC}"
    fi
else
    echo -e "${GREEN}✔  npm audit: no critical/high vulnerabilities.${NC}"
fi

# ── 3. Check for hardcoded secrets ─────────────────────────
echo ""
echo "▶ Scanning for hardcoded secrets…"
ROOT_DIR="$(dirname "$0")/.."

# Patterns that should NEVER appear in production source files
SECRET_PATTERNS=(
    'sk-[a-zA-Z0-9]{20,}'       # OpenAI-style keys
    'AIza[a-zA-Z0-9_-]{35}'     # Google API keys
    'AKIA[A-Z0-9]{16}'          # AWS access key IDs
    'ghp_[a-zA-Z0-9]{36}'       # GitHub personal access tokens
    'xoxb-[0-9-]+'              # Slack bot tokens
)

FOUND_SECRETS=0
for pattern in "${SECRET_PATTERNS[@]}"; do
    MATCHES=$(grep -rEn --include="*.rs" --include="*.ts" --include="*.tsx" --include="*.py" \
        --exclude-dir=".git" --exclude-dir="target" --exclude-dir="node_modules" --exclude-dir="venv" \
        "$pattern" "$ROOT_DIR" 2>/dev/null || true)
    if [ -n "$MATCHES" ]; then
        echo -e "${RED}✖  Potential hardcoded secret found:${NC}"
        echo "$MATCHES"
        FOUND_SECRETS=$((FOUND_SECRETS + 1))
    fi
done

if [ "$FOUND_SECRETS" -eq 0 ]; then
    echo -e "${GREEN}✔  No hardcoded secrets detected.${NC}"
else
    echo -e "${RED}✖  $FOUND_SECRETS potential secret pattern(s) found. Review and remove.${NC}"
    exit 1
fi

echo ""
echo "═══════════════════════════════════════════════════════"
echo -e "${GREEN}  ✔ All security checks passed.${NC}"
echo "═══════════════════════════════════════════════════════"
