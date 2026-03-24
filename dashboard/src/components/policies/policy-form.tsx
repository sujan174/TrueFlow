"use client"

import { useState } from "react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Badge } from "@/components/ui/badge"
import type { PolicyRow, Rule, CreatePolicyRequest, UpdatePolicyRequest } from "@/lib/types/policy"

interface PolicyFormProps {
  initialData?: PolicyRow | null
  onSubmit: (data: CreatePolicyRequest | UpdatePolicyRequest) => Promise<void>
  isSubmitting: boolean
}

// Preset templates
const POLICY_TEMPLATES: Record<string, { name: string; description: string; rules: Rule[] }> = {
  rate_limit: {
    name: "Rate Limiting",
    description: "Limit requests per time window",
    rules: [
      {
        when: { always: true },
        then: [{ action: "rate_limit", window: "1m", max_requests: 100, key: "token" }],
      },
    ],
  },
  content_filter: {
    name: "Content Safety",
    description: "Block harmful content, jailbreaks, and injections",
    rules: [
      {
        when: { always: true },
        then: [
          {
            action: "content_filter",
            block_jailbreak: true,
            block_harmful: true,
            block_code_injection: true,
            block_profanity: false,
            risk_threshold: 0.5,
          },
        ],
      },
    ],
  },
  pii_redact: {
    name: "PII Redaction",
    description: "Redact sensitive data (SSN, emails, phones)",
    rules: [
      {
        when: { always: true },
        then: [
          {
            action: "redact",
            direction: "both",
            patterns: ["ssn", "email", "phone", "credit_card"],
            on_match: "redact",
          },
        ],
      },
    ],
  },
  model_override: {
    name: "Model Downgrade",
    description: "Force cheaper model for cost control",
    rules: [
      {
        when: { always: true },
        then: [
          {
            action: "override",
            set_body_fields: { model: "gpt-4o-mini" },
          },
        ],
      },
    ],
  },
  hitl_approval: {
    name: "Human Approval",
    description: "Require human approval for requests",
    rules: [
      {
        when: { always: true },
        then: [
          {
            action: "require_approval",
            timeout: "30m",
            fallback: "deny",
          },
        ],
      },
    ],
  },
  tool_scope: {
    name: "Tool Restrictions",
    description: "Control which tools agents can use",
    rules: [
      {
        when: { always: true },
        then: [
          {
            action: "tool_scope",
            allowed_tools: ["read_*", "search_*"],
            blocked_tools: ["write_*", "delete_*"],
            deny_message: "Tool not authorized for this agent",
          },
        ],
      },
    ],
  },
}

export function PolicyForm({ initialData, onSubmit, isSubmitting }: PolicyFormProps) {
  const [name, setName] = useState(initialData?.name || "")
  const [mode, setMode] = useState<string>(initialData?.mode || "enforce")
  const [phase, setPhase] = useState<string>(initialData?.phase || "pre")
  const [rulesJson, setRulesJson] = useState<string>(
    initialData ? JSON.stringify(initialData.rules, null, 2) : "[]"
  )
  const [jsonError, setJsonError] = useState<string | null>(null)

  const handleTemplateSelect = (templateKey: string) => {
    const template = POLICY_TEMPLATES[templateKey]
    if (template) {
      setRulesJson(JSON.stringify(template.rules, null, 2))
      setJsonError(null)
    }
  }

  const validateJson = (json: string): Rule[] | null => {
    try {
      const parsed = JSON.parse(json)
      if (!Array.isArray(parsed)) {
        setJsonError("Rules must be an array")
        return null
      }
      setJsonError(null)
      return parsed
    } catch (e) {
      setJsonError(e instanceof Error ? e.message : "Invalid JSON")
      return null
    }
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()

    const rules = validateJson(rulesJson)
    if (!rules) return

    const data = initialData
      ? { name, mode: mode as "enforce" | "shadow", phase: phase as "pre" | "post", rules }
      : { name, mode: mode as "enforce" | "shadow", phase: phase as "pre" | "post", rules }

    await onSubmit(data)
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-6">
      {/* Basic Info */}
      <div className="grid gap-4 md:grid-cols-3">
        <div className="space-y-2">
          <Label htmlFor="name">Policy Name</Label>
          <Input
            id="name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g., rate-limit-policy"
            required
          />
        </div>

        <div className="space-y-2">
          <Label htmlFor="mode">Mode</Label>
          <Select value={mode} onValueChange={(value) => value && setMode(value)}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="enforce">
                <div className="flex items-center gap-2">
                  <Badge variant="default">Enforce</Badge>
                  <span className="text-muted-foreground">Block violations</span>
                </div>
              </SelectItem>
              <SelectItem value="shadow">
                <div className="flex items-center gap-2">
                  <Badge variant="secondary">Shadow</Badge>
                  <span className="text-muted-foreground">Log only, don&apos;t block</span>
                </div>
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div className="space-y-2">
          <Label htmlFor="phase">Phase</Label>
          <Select value={phase} onValueChange={(value) => value && setPhase(value)}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="pre">
                <div className="flex flex-col">
                  <span>Pre-flight</span>
                  <span className="text-xs text-muted-foreground">Before upstream request</span>
                </div>
              </SelectItem>
              <SelectItem value="post">
                <div className="flex flex-col">
                  <span>Post-flight</span>
                  <span className="text-xs text-muted-foreground">After upstream response</span>
                </div>
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      {/* Templates */}
      <div className="space-y-2">
        <Label>Quick Templates</Label>
        <div className="flex flex-wrap gap-2">
          {Object.entries(POLICY_TEMPLATES).map(([key, template]) => (
            <Button
              key={key}
              type="button"
              variant="outline"
              size="sm"
              onClick={() => handleTemplateSelect(key)}
            >
              {template.name}
            </Button>
          ))}
        </div>
        <p className="text-xs text-muted-foreground">
          Click a template to load a pre-configured rule set
        </p>
      </div>

      {/* Rules JSON Editor */}
      <div className="space-y-2">
        <Label htmlFor="rules">Rules (JSON)</Label>
        <div className="relative">
          <textarea
            id="rules"
            value={rulesJson}
            onChange={(e) => {
              setRulesJson(e.target.value)
              validateJson(e.target.value)
            }}
            className={`w-full h-64 p-4 font-mono text-sm border rounded-lg bg-muted/30 focus:outline-none focus:ring-2 focus:ring-ring ${
              jsonError ? "border-destructive" : ""
            }`}
            placeholder='[
  {
    "when": { "always": true },
    "then": [{ "action": "rate_limit", "window": "1m", "max_requests": 100 }]
  }
]'
          />
        </div>
        {jsonError && (
          <p className="text-sm text-destructive">{jsonError}</p>
        )}
        <p className="text-xs text-muted-foreground">
          Define rules with condition → action pairs. Each rule has a &quot;when&quot; condition and &quot;then&quot; actions.
        </p>
      </div>

      {/* Actions */}
      <div className="flex gap-3">
        <Button type="submit" disabled={isSubmitting || !name || !!jsonError}>
          {isSubmitting ? "Saving..." : initialData ? "Update Policy" : "Create Policy"}
        </Button>
        <Button type="button" variant="outline" onClick={() => window.history.back()}>
          Cancel
        </Button>
      </div>
    </form>
  )
}