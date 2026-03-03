# Security Policy

## Reporting a Vulnerability

**Please do NOT open a public GitHub issue for security vulnerabilities.**

Instead, report vulnerabilities by emailing:

📧 **security@ailink.dev**

Include:
- Description of the vulnerability
- Steps to reproduce
- Impact assessment
- Any suggested fixes

## Response Timeline

| Stage | Timeline |
|---|---|
| Acknowledgment | Within 48 hours |
| Initial assessment | Within 1 week |
| Fix & disclosure | Within 30 days |

## Supported Versions

| Version | Supported |
|---|---|
| 0.8.x (latest) | ✅ |
| < 0.8.0 | ❌ |

## Security Measures

AILink implements the following security controls:

- **AES-256-GCM** envelope encryption for all stored credentials
- **Argon2id** key derivation from the master key
- **SSRF protection** — private IP ranges blocked on upstream URLs
- **Header redaction** — `Authorization`, `X-Admin-Key`, and API keys are never logged
- **Rate limiting** — configurable per-token and global rate limits
- **Input validation** — strict schema validation on all API inputs
- **SQL injection prevention** — parameterized queries throughout
- **XSS prevention** — CSP headers, output encoding, HttpOnly cookies

For a detailed security design, see [docs/reference/security.md](docs/reference/security.md).

## Scope

The following are in scope for security reports:

- Authentication / authorization bypasses
- Credential leakage
- Injection vulnerabilities (SQL, command, header)
- Cryptographic weaknesses
- SSRF in credential/upstream handling
- Privilege escalation

The following are out of scope:

- DOS attacks on self-hosted instances
- Social engineering
- Issues in third-party dependencies (report upstream instead)
- Missing security headers on `localhost` dev setups
