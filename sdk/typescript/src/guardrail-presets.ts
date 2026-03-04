/**
 * Guardrail preset constants for IDE autocompletion.
 *
 * @example
 * ```ts
 * import { PRESET_PROMPT_INJECTION, PRESET_PII_ENTERPRISE } from "@trueflow/sdk";
 *
 * await admin.guardrails.enable("tok_abc", [PRESET_PROMPT_INJECTION, PRESET_PII_ENTERPRISE]);
 * ```
 *
 * @module
 */

/** Block DAN jailbreaks, harmful content, and code injection (35+ patterns, risk threshold 0.3). */
export const PRESET_PROMPT_INJECTION = "prompt_injection" as const;

/** Block SQL injection, shell commands, Python exec/eval, JS eval, data exfiltration. */
export const PRESET_CODE_INJECTION = "code_injection" as const;

/** Silently redact 8 PII types (SSN, email, credit card, phone, API key, IBAN, DOB, IP). */
export const PRESET_PII_REDACTION = "pii_redaction" as const;

/** Enterprise-grade: redact all 12 PII types including passport, AWS key, driver's license, MRN. */
export const PRESET_PII_ENTERPRISE = "pii_enterprise" as const;

/** Block (HTTP 400) requests containing PII — for strict no-PII policies. */
export const PRESET_PII_BLOCK = "pii_block" as const;

/** Healthcare: redact SSN, email, phone, date-of-birth, MRN. */
export const PRESET_HIPAA = "hipaa" as const;

/** Payment Card Industry: redact credit card numbers and API keys. */
export const PRESET_PCI = "pci" as const;

/** Restrict agents to specific topics. Requires topicAllowlist or topicDenylist. */
export const PRESET_TOPIC_FENCE = "topic_fence" as const;

/** Block requests with content exceeding 50,000 characters. */
export const PRESET_LENGTH_LIMIT = "length_limit" as const;

/** Union of all built-in guardrail preset names. */
export type GuardrailPreset =
    | typeof PRESET_PROMPT_INJECTION
    | typeof PRESET_CODE_INJECTION
    | typeof PRESET_PII_REDACTION
    | typeof PRESET_PII_ENTERPRISE
    | typeof PRESET_PII_BLOCK
    | typeof PRESET_HIPAA
    | typeof PRESET_PCI
    | typeof PRESET_TOPIC_FENCE
    | typeof PRESET_LENGTH_LIMIT;
