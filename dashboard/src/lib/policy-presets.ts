import type { Rule, PolicyPreset } from "@/lib/types/policy"

// ============================================================================
// Policy Presets Library
// ============================================================================

// Note: Icons are string identifiers that the UI will map to Lucide icons
// Using descriptive names that match the action/purpose

export const POLICY_PRESETS: PolicyPreset[] = [
  {
    id: 'strict-safety',
    name: 'Strict Safety',
    description: 'Comprehensive protection against all harmful content types',
    icon: 'shield',
    rules: [
      {
        when: { always: true },
        then: [
          {
            action: 'content_filter',
            block_jailbreak: true,
            block_harmful: true,
            block_code_injection: true,
            block_profanity: true,
            block_bias: true,
            block_competitor_mention: true,
            block_sensitive_topics: true,
            block_gibberish: true,
            block_contact_info: true,
            block_ip_leakage: true,
            risk_threshold: 0.5,
          },
        ],
      },
    ],
  },
  {
    id: 'balanced-protection',
    name: 'Balanced Protection',
    description: 'Common protections for most production use cases',
    icon: 'scale',
    rules: [
      {
        when: { always: true },
        then: [
          {
            action: 'content_filter',
            block_jailbreak: true,
            block_harmful: true,
            block_code_injection: true,
            block_ip_leakage: true,
            risk_threshold: 0.6,
          },
        ],
      },
    ],
  },
  {
    id: 'minimal-safety',
    name: 'Minimal Safety',
    description: 'Essential jailbreak and harmful content protection',
    icon: 'shield-check',
    rules: [
      {
        when: { always: true },
        then: [
          {
            action: 'content_filter',
            block_jailbreak: true,
            block_harmful: true,
            risk_threshold: 0.7,
          },
        ],
      },
    ],
  },
  {
    id: 'enterprise-compliance',
    name: 'Enterprise Compliance',
    description: 'PII protection, audit logging, and security controls',
    icon: 'building',
    rules: [
      {
        when: { always: true },
        then: [
          {
            action: 'content_filter',
            block_jailbreak: true,
            block_harmful: true,
            block_code_injection: true,
            block_ip_leakage: true,
            block_contact_info: true,
            risk_threshold: 0.5,
          },
          {
            action: 'redact',
            direction: 'both',
            patterns: ['ssn', 'credit_card', 'email', 'phone'],
            on_match: 'redact',
          },
          {
            action: 'log',
            level: 'info',
            tags: { audit: 'true' },
          },
        ],
      },
    ],
  },
  {
    id: 'rate-limiting',
    name: 'Rate Limiting',
    description: 'Protect against abuse with request rate limits',
    icon: 'clock',
    rules: [
      {
        when: { always: true },
        then: [
          {
            action: 'rate_limit',
            window: '1m',
            max_requests: 100,
            key: 'token',
          },
        ],
      },
    ],
  },
  {
    id: 'pii-redaction',
    name: 'PII Redaction',
    description: 'Automatically redact sensitive personal information',
    icon: 'lock',
    rules: [
      {
        when: { always: true },
        then: [
          {
            action: 'redact',
            direction: 'both',
            patterns: ['ssn', 'credit_card', 'email', 'phone'],
            on_match: 'redact',
          },
        ],
      },
    ],
  },
  {
    id: 'model-downgrade',
    name: 'Cost Control',
    description: 'Force cheaper models for cost optimization',
    icon: 'dollar-sign',
    rules: [
      {
        when: { always: true },
        then: [
          {
            action: 'override',
            set_body_fields: { model: 'gpt-4o-mini' },
          },
        ],
      },
    ],
  },
  {
    id: 'human-approval',
    name: 'Human Approval',
    description: 'Require human review before processing requests',
    icon: 'eye',
    rules: [
      {
        when: { always: true },
        then: [
          {
            action: 'require_approval',
            timeout: '30m',
            fallback: 'deny',
          },
        ],
      },
    ],
  },
  {
    id: 'tool-restrictions',
    name: 'Tool Restrictions',
    description: 'Control which tools agents can access',
    icon: 'wrench',
    rules: [
      {
        when: { always: true },
        then: [
          {
            action: 'tool_scope',
            allowed_tools: ['read_*', 'search_*'],
            blocked_tools: ['write_*', 'delete_*'],
            deny_message: 'Tool not authorized for this agent',
          },
        ],
      },
    ],
  },
  {
    id: 'audit-logging',
    name: 'Audit Logging',
    description: 'Comprehensive logging for compliance and debugging',
    icon: 'file-text',
    rules: [
      {
        when: { always: true },
        then: [
          {
            action: 'log',
            level: 'info',
            tags: { audit: 'true', compliance: 'enabled' },
          },
          {
            action: 'tag',
            key: 'logged_at',
            value: new Date().toISOString(),
          },
        ],
      },
    ],
  },
]

// ============================================================================
// Helper Functions
// ============================================================================

export function getPresetById(id: string): PolicyPreset | undefined {
  return POLICY_PRESETS.find(p => p.id === id)
}

export function getPresetRules(presetId: string): Rule[] | undefined {
  const preset = getPresetById(presetId)
  return preset?.rules
}

// Map preset icon strings to Lucide icon names
export const PRESET_ICONS: Record<string, string> = {
  'shield': 'Shield',
  'scale': 'Scale',
  'shield-check': 'ShieldCheck',
  'building': 'Building',
  'clock': 'Clock',
  'lock': 'Lock',
  'dollar-sign': 'DollarSign',
  'eye': 'Eye',
  'wrench': 'Wrench',
  'file-text': 'FileText',
}

export default POLICY_PRESETS