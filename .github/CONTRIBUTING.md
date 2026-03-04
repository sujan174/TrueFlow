# Contributing to TrueFlow

Thank you for your interest in contributing to TrueFlow! This guide will help you get started.

## Getting Started

### Prerequisites

- **Rust** 1.75+ (gateway)
- **Node.js** 20+ (dashboard)
- **Python** 3.10+ (SDK and tests)
- **Docker** & Docker Compose (full stack)
- **PostgreSQL** 16 & **Redis** 7 (or use Docker)

### Development Setup

```bash
# 1. Clone the repo
git clone https://github.com/sujan174/trueflow.git
cd trueflow

# 2. Start dependencies
docker compose up -d postgres redis

# 3. Run the gateway (dev mode)
cd gateway
cargo run

# 4. Run the dashboard (dev mode)
cd dashboard
npm install
npm run dev
```

The gateway runs on `http://localhost:8443` and the dashboard on `http://localhost:3000`.

## Project Structure

| Directory | Language | Description |
|---|---|---|
| `gateway/` | Rust | Core gateway — proxy, policy engine, vault, API |
| `dashboard/` | TypeScript | Next.js admin UI |
| `sdk/python/` | Python | Python client SDK |
| `sdk/typescript/` | TypeScript | TypeScript client SDK |
| `tests/` | Python | E2E, integration, and unit tests |
| `docs/` | Markdown | All documentation |

## How to Contribute

### 🐛 Bug Reports

File an issue with:
- Steps to reproduce
- Expected vs actual behavior
- Gateway version and environment
- Relevant logs (redact any keys!)

### 💡 Feature Requests

Open an issue with:
- The problem you're solving
- Your proposed solution
- Alternatives you considered

### 🔧 Pull Requests

1. **Fork** the repo and create a branch from `main`
2. **Write tests** for any new functionality
3. **Follow existing patterns** — match the code style of the file you're editing
4. **Keep PRs focused** — one feature or fix per PR
5. **Update docs** if you change user-facing behavior
6. **Run tests** before submitting:

```bash
# Rust tests
cargo test

# Python tests
python3 -m pytest tests/unit/ -v

# Full E2E (requires docker compose up)
python3 tests/e2e/test_mock_suite.py
```

### Good First Issues

Look for issues tagged with `good first issue` — these are scoped, well-documented tasks that are a great way to get familiar with the codebase.

## Code Style

### Rust (Gateway)
- Follow standard `rustfmt` formatting
- Use `clippy` lints: `cargo clippy`
- Prefer explicit error handling over `.unwrap()`
- Add inline comments for non-obvious logic

### TypeScript (Dashboard)
- Use the existing component patterns
- Follow Next.js App Router conventions
- Use ShadCN UI components where possible

### Python (SDK)
- Follow PEP 8
- Type hints for all public APIs
- Docstrings for all public methods

## Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(gateway): add model alias resolution to proxy
fix(dashboard): correct chart rendering on small screens
docs: update policy guide with dynamic_route action
chore: remove unused dependencies
```

## License

By contributing, you agree that your contributions will be licensed under the [TrueFlow Proprietary License](../LICENSE).
