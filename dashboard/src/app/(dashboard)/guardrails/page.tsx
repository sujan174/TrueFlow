"use client"

import { useEffect, useState } from "react"
import { Shield, AlertTriangle, Lock, ExternalLink, MoreHorizontal, Eye, Edit, Plus } from "lucide-react"
import Link from "next/link"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  listPolicies,
  type PolicyRow,
  type Action,
  type ActionContentFilter,
  type ActionExternalGuardrail,
} from "@/lib/api"
import { isGuardrailAction, getActionDisplayName } from "@/lib/types/policy"
import { GuardrailPresetDialog } from "@/components/guardrails/guardrail-preset-dialog"

interface GuardrailInfo {
  policyId: string
  policyName: string
  policyMode: string
  policyPhase: string
  guardrailType: "content_filter" | "external_guardrail"
  action: ActionContentFilter | ActionExternalGuardrail
  ruleIndex: number
}

function formatRelativeTime(dateString: string): string {
  const date = new Date(dateString)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffMins = Math.floor(diffMs / 60000)
  const diffHours = Math.floor(diffMs / 3600000)
  const diffDays = Math.floor(diffMs / 86400000)

  if (diffMins < 1) return "just now"
  if (diffMins < 60) return `${diffMins}m ago`
  if (diffHours < 24) return `${diffHours}h ago`
  if (diffDays < 30) return `${diffDays}d ago`
  return date.toLocaleDateString()
}

function extractGuardrails(policies: PolicyRow[]): GuardrailInfo[] {
  const guardrails: GuardrailInfo[] = []

  for (const policy of policies) {
    for (let i = 0; i < policy.rules.length; i++) {
      const rule = policy.rules[i]
      const thenArray = Array.isArray(rule.then) ? rule.then : [rule.then]

      for (const action of thenArray) {
        if (action.action === "content_filter") {
          guardrails.push({
            policyId: policy.id,
            policyName: policy.name,
            policyMode: policy.mode,
            policyPhase: policy.phase,
            guardrailType: "content_filter",
            action: action as ActionContentFilter,
            ruleIndex: i,
          })
        } else if (action.action === "external_guardrail") {
          guardrails.push({
            policyId: policy.id,
            policyName: policy.name,
            policyMode: policy.mode,
            policyPhase: policy.phase,
            guardrailType: "external_guardrail",
            action: action as ActionExternalGuardrail,
            ruleIndex: i,
          })
        }
      }
    }
  }

  return guardrails
}

function ContentFilterCategories({ action }: { action: ActionContentFilter }) {
  const categories: { key: keyof ActionContentFilter; label: string }[] = [
    { key: "block_jailbreak", label: "Jailbreak" },
    { key: "block_harmful", label: "Harmful" },
    { key: "block_code_injection", label: "Code Injection" },
    { key: "block_profanity", label: "Profanity" },
    { key: "block_bias", label: "Bias" },
    { key: "block_competitor_mention", label: "Competitors" },
    { key: "block_sensitive_topics", label: "Sensitive Topics" },
    { key: "block_gibberish", label: "Gibberish" },
    { key: "block_contact_info", label: "Contact Info" },
    { key: "block_ip_leakage", label: "IP Leakage" },
  ]

  const enabled = categories.filter((c) => action[c.key] === true)

  if (enabled.length === 0) {
    return <span className="text-muted-foreground">No categories enabled</span>
  }

  return (
    <div className="flex flex-wrap gap-1">
      {enabled.slice(0, 3).map((c) => (
        <Badge key={c.key} variant="outline" className="text-[10px]">
          {c.label}
        </Badge>
      ))}
      {enabled.length > 3 && (
        <Badge variant="outline" className="text-[10px]">
          +{enabled.length - 3}
        </Badge>
      )}
    </div>
  )
}

function ExternalVendorBadge({ vendor }: { vendor: string }) {
  const labels: Record<string, string> = {
    azure_content_safety: "Azure Content Safety",
    aws_comprehend: "AWS Comprehend",
    llama_guard: "LlamaGuard",
    palo_alto_airs: "Palo Alto AIRS",
    prompt_security: "Prompt Security",
  }

  return (
    <Badge variant="secondary" className="text-[10px]">
      {labels[vendor] || vendor}
    </Badge>
  )
}

export default function GuardrailsPage() {
  const [guardrails, setGuardrails] = useState<GuardrailInfo[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [filter, setFilter] = useState<"all" | "content_filter" | "external_guardrail">("all")
  const [presetDialogOpen, setPresetDialogOpen] = useState(false)

  useEffect(() => {
    async function fetchGuardrails() {
      try {
        const policies = await listPolicies({ limit: 1000 })
        const extracted = extractGuardrails(policies)
        setGuardrails(extracted)
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load guardrails")
      } finally {
        setLoading(false)
      }
    }
    fetchGuardrails()
  }, [])

  const loadGuardrails = async () => {
    try {
      const policies = await listPolicies({ limit: 1000 })
      const extracted = extractGuardrails(policies)
      setGuardrails(extracted)
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load guardrails")
    }
  }

  const filteredGuardrails = filter === "all"
    ? guardrails
    : guardrails.filter((g) => g.guardrailType === filter)

  const contentFilterCount = guardrails.filter((g) => g.guardrailType === "content_filter").length
  const externalGuardrailCount = guardrails.filter((g) => g.guardrailType === "external_guardrail").length

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex flex-col gap-1">
            <h1 className="text-2xl lg:text-3xl font-bold tracking-tight">
              Guardrails
            </h1>
            <p className="text-sm text-muted-foreground">
              Content safety and external guardrail policies
            </p>
          </div>
          <div className="flex gap-2">
            <Button
              variant="outline"
              className="gap-2"
              onClick={() => setPresetDialogOpen(true)}
            >
              <Plus className="h-4 w-4" />
              Quick Enable
            </Button>
            <Link href="/policies/new">
              <Button className="gap-2">
                <Shield className="h-4 w-4" />
                Create Guardrail
              </Button>
            </Link>
          </div>
        </div>

        {/* Filters */}
        <div className="flex gap-2">
          <Button
            variant={filter === "all" ? "default" : "outline"}
            size="sm"
            onClick={() => setFilter("all")}
          >
            All ({guardrails.length})
          </Button>
          <Button
            variant={filter === "content_filter" ? "default" : "outline"}
            size="sm"
            onClick={() => setFilter("content_filter")}
          >
            <Lock className="h-3 w-3 mr-1" />
            Built-in ({contentFilterCount})
          </Button>
          <Button
            variant={filter === "external_guardrail" ? "default" : "outline"}
            size="sm"
            onClick={() => setFilter("external_guardrail")}
          >
            <ExternalLink className="h-3 w-3 mr-1" />
            External ({externalGuardrailCount})
          </Button>
        </div>

        {/* Table */}
        <div className="bg-card border rounded-xl shadow-sm overflow-hidden">
          {loading ? (
            <div className="p-8 text-center text-muted-foreground">
              Loading guardrails...
            </div>
          ) : error ? (
            <div className="p-8 text-center text-destructive">{error}</div>
          ) : filteredGuardrails.length === 0 ? (
            <div className="p-8 text-center">
              <Shield className="h-12 w-12 mx-auto text-muted-foreground/50 mb-4" />
              <p className="text-muted-foreground">No guardrails configured</p>
              <p className="text-sm text-muted-foreground/70 mt-1">
                Create a policy with ContentFilter or ExternalGuardrail action
              </p>
              <Link href="/policies/new">
                <Button className="mt-4 gap-2">
                  <Shield className="h-4 w-4" />
                  Create Guardrail
                </Button>
              </Link>
            </div>
          ) : (
            <table className="w-full">
              <thead className="bg-muted/50 border-b">
                <tr className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                  <th className="px-4 py-3 text-left">Type</th>
                  <th className="px-4 py-3 text-left">Policy</th>
                  <th className="px-4 py-3 text-left">Configuration</th>
                  <th className="px-4 py-3 text-left">Threshold</th>
                  <th className="px-4 py-3 text-left">Mode</th>
                  <th className="px-4 py-3 text-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {filteredGuardrails.map((guardrail) => (
                  <tr
                    key={`${guardrail.policyId}-${guardrail.ruleIndex}`}
                    className="border-b last:border-0 hover:bg-muted/30 transition-colors"
                  >
                    <td className="px-4 py-3">
                      {guardrail.guardrailType === "content_filter" ? (
                        <div className="flex items-center gap-2">
                          <Lock className="h-4 w-4 text-muted-foreground" />
                          <span className="text-sm">Built-in</span>
                        </div>
                      ) : (
                        <div className="flex items-center gap-2">
                          <ExternalLink className="h-4 w-4 text-muted-foreground" />
                          <ExternalVendorBadge
                            vendor={(guardrail.action as ActionExternalGuardrail).vendor}
                          />
                        </div>
                      )}
                    </td>
                    <td className="px-4 py-3">
                      <Link
                        href={`/policies/${guardrail.policyId}`}
                        className="text-sm font-medium hover:underline"
                      >
                        {guardrail.policyName}
                      </Link>
                      <p className="text-xs text-muted-foreground">
                        Rule #{guardrail.ruleIndex + 1}
                      </p>
                    </td>
                    <td className="px-4 py-3">
                      {guardrail.guardrailType === "content_filter" ? (
                        <ContentFilterCategories action={guardrail.action as ActionContentFilter} />
                      ) : (
                        <div className="text-sm text-muted-foreground">
                          {(guardrail.action as ActionExternalGuardrail).endpoint.slice(0, 40)}...
                        </div>
                      )}
                    </td>
                    <td className="px-4 py-3">
                      <Badge variant="outline" className="text-[10px]">
                        Risk ≥ {("risk_threshold" in guardrail.action ? guardrail.action.risk_threshold || 0.5 : 0.5).toFixed(1)}
                      </Badge>
                    </td>
                    <td className="px-4 py-3">
                      <Badge
                        variant={guardrail.policyMode === "enforce" ? "default" : "secondary"}
                        className="text-[10px]"
                      >
                        {guardrail.policyMode}
                      </Badge>
                    </td>
                    <td className="px-4 py-3 text-right">
                      <DropdownMenu>
                        <DropdownMenuTrigger>
                          <Button variant="ghost" size="icon-sm">
                            <MoreHorizontal className="h-4 w-4" />
                          </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem>
                            <Link href={`/policies/${guardrail.policyId}`} className="flex items-center">
                              <Edit className="h-4 w-4 mr-2" />
                              Edit Policy
                            </Link>
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>

        {/* Info Card */}
        <div className="p-4 border rounded-lg bg-muted/30">
          <div className="flex items-start gap-3">
            <AlertTriangle className="h-5 w-5 text-muted-foreground mt-0.5" />
            <div>
              <h3 className="text-sm font-medium">About Guardrails</h3>
              <p className="text-xs text-muted-foreground mt-1">
                Guardrails are policy actions that filter or validate content. Built-in guardrails
                detect jailbreaks, harmful content, PII, and more. External guardrails integrate
                with third-party services like Azure Content Safety and LlamaGuard.
              </p>
            </div>
          </div>
        </div>
      </div>

      {/* Quick Enable Dialog */}
      <GuardrailPresetDialog
        open={presetDialogOpen}
        onOpenChange={setPresetDialogOpen}
        onSuccess={loadGuardrails}
      />
    </div>
  )
}