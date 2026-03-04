# TrueFlow — Remaining Engineering Roadmap


| # | Gap | Why Critical | Effort |
|---|-----|-------------|--------|
| **8** | **KMS Integration** — AWS KMS + HashiCorp Vault backends for `SecretStore` trait | Enterprise: "Can I use my own KMS?" `vault/mod.rs` trait is already defined — just implement new backends. Won't close sales before SOC-2, but needed for the pipeline after SOC-2. | ~1.5 weeks |
| **9** | **NLP-Backed PII Redaction** — optional Presidio/spaCy sidecar as `PiiDetector` backend | Kong's Apr 2025 plugin covers 20+ categories in 12 languages via NLP. Our regex covers English-primary well enough for now. Revisit when a multilingual customer asks for it. | ~2–3 weeks |
| **11** | **Cache Streaming** — stream cached responses chunk-by-chunk instead of returning full blob | Portkey does this; small UX improvement for cached responses. | ~0.5 day |

