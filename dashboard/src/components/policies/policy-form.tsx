"use client"

import { useState, useCallback, useEffect } from "react"
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Badge } from "@/components/ui/badge"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip"
import {
  Shield,
  Scale,
  ShieldCheck,
  Building,
  Clock,
  Lock,
  DollarSign,
  Eye,
  Wrench,
  FileText,
  Code,
  ArrowLeft,
  Play,
  Save,
  Loader2,
} from "lucide-react"
import type {
  PolicyRow,
  Rule,
  Condition,
  CreatePolicyRequest,
  UpdatePolicyRequest,
  PolicyEditorMode,
  PolicyEditorTab,
  Action,
} from "@/lib/types/policy"
import { POLICY_PRESETS } from "@/lib/policy-presets"

// Tab Components
import { ConditionBuilder } from "./condition-builder"
import { GuardrailsTab } from "./guardrails-tab"
import { RoutingTab } from "./routing-tab"
import { PIITab } from "./pii-tab"
import { ActionsTab } from "./actions-tab"

// ============================================================================
// Types
// ============================================================================

interface PolicyFormProps {
  initialData?: PolicyRow | null
  onSubmit: (data: CreatePolicyRequest | UpdatePolicyRequest) => Promise<void>
  isSubmitting: boolean
}

// ============================================================================
// Icon mapping for presets
// ============================================================================

const PRESET_ICON_COMPONENTS: Record<string, React.ReactNode> = {
  'shield': <Shield className="h-4 w-4" />,
  'scale': <Scale className="h-4 w-4" />,
  'shield-check': <ShieldCheck className="h-4 w-4" />,
  'building': <Building className="h-4 w-4" />,
  'clock': <Clock className="h-4 w-4" />,
  'lock': <Lock className="h-4 w-4" />,
  'dollar-sign': <DollarSign className="h-4 w-4" />,
  'eye': <Eye className="h-4 w-4" />,
  'wrench': <Wrench className="h-4 w-4" />,
  'file-text': <FileText className="h-4 w-4" />,
}

// ============================================================================
// Main Component
// ============================================================================

export function PolicyForm({ initialData, onSubmit, isSubmitting }: PolicyFormProps) {
  // Basic fields
  const [name, setName] = useState(initialData?.name || "")
  const [mode, setMode] = useState<string>(initialData?.mode || "enforce")
  const [phase, setPhase] = useState<string>(initialData?.phase || "pre")

  // Rules state
  const [rules, setRules] = useState<Rule[]>(
    initialData?.rules || [{ when: { always: true }, then: [] }]
  )

  // Editor state
  const [editorMode, setEditorMode] = useState<PolicyEditorMode>("visual")
  const [activeTab, setActiveTab] = useState<PolicyEditorTab>("conditions")
  const [rulesJson, setRulesJson] = useState<string>(
    initialData ? JSON.stringify(initialData.rules, null, 2) : "[]"
  )
  const [jsonError, setJsonError] = useState<string | null>(null)

  // Current rule being edited (simplified - just the first rule for now)
  const currentRule = rules[0] || { when: { always: true }, then: [] }

  // Sync JSON when rules change
  useEffect(() => {
    if (editorMode === "visual") {
      setRulesJson(JSON.stringify(rules, null, 2))
    }
  }, [rules, editorMode])

  // Sync rules when JSON changes
  useEffect(() => {
    if (editorMode === "json") {
      try {
        const parsed = JSON.parse(rulesJson)
        if (Array.isArray(parsed)) {
          setRules(parsed)
          setJsonError(null)
        }
      } catch (e) {
        setJsonError(e instanceof Error ? e.message : "Invalid JSON")
      }
    }
  }, [rulesJson, editorMode])

  // Handlers
  const handleConditionChange = useCallback((condition: Condition) => {
    setRules(prev => {
      const newRules = [...prev]
      newRules[0] = { ...newRules[0], when: condition }
      return newRules
    })
  }, [])

  const handleGuardrailsChange = useCallback((action: Action | null) => {
    if (!action) return
    setRules(prev => {
      const newRules = [...prev]
      const currentThen = Array.isArray(newRules[0].then) ? newRules[0].then : [newRules[0].then]
      // Remove any existing content_filter action
      const filteredActions = currentThen.filter(a => a.action !== 'content_filter')
      newRules[0] = {
        ...newRules[0],
        then: [...filteredActions, action],
      }
      return newRules
    })
  }, [])

  const handleRoutingChange = useCallback((action: Action | null) => {
    if (!action) return
    setRules(prev => {
      const newRules = [...prev]
      const currentThen = Array.isArray(newRules[0].then) ? newRules[0].then : [newRules[0].then]
      const filteredActions = currentThen.filter(a => a.action !== 'dynamic_route')
      newRules[0] = {
        ...newRules[0],
        then: [...filteredActions, action],
      }
      return newRules
    })
  }, [])

  const handlePIIChange = useCallback((action: Action | null) => {
    if (!action) return
    setRules(prev => {
      const newRules = [...prev]
      const currentThen = Array.isArray(newRules[0].then) ? newRules[0].then : [newRules[0].then]
      const filteredActions = currentThen.filter(a => a.action !== 'redact')
      newRules[0] = {
        ...newRules[0],
        then: [...filteredActions, action],
      }
      return newRules
    })
  }, [])

  const handleActionsChange = useCallback((rule: Rule | null) => {
    if (!rule) return
    setRules([rule])
  }, [])

  const handlePresetSelect = (presetId: string) => {
    const preset = POLICY_PRESETS.find(p => p.id === presetId)
    if (preset) {
      setRules(preset.rules)
      if (!name) {
        setName(preset.name)
      }
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

    const finalRules = editorMode === "json" ? validateJson(rulesJson) : rules
    if (!finalRules) return

    const data = initialData
      ? { name, mode: mode as "enforce" | "shadow", phase: phase as "pre" | "post", rules: finalRules }
      : { name, mode: mode as "enforce" | "shadow", phase: phase as "pre" | "post", rules: finalRules }

    await onSubmit(data)
  }

  // Get current actions for each tab
  const currentActions = Array.isArray(currentRule.then) ? currentRule.then : [currentRule.then]
  const guardrailAction = currentActions.find(a => a.action === 'content_filter')
  const routingAction = currentActions.find(a => a.action === 'dynamic_route')
  const piiAction = currentActions.find(a => a.action === 'redact')

  return (
    <form onSubmit={handleSubmit} className="space-y-6">
      {/* Header with Mode Toggle */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Button
            type="button"
            variant={editorMode === "visual" ? "default" : "outline"}
            size="sm"
            onClick={() => setEditorMode("visual")}
          >
            Visual
          </Button>
          <Button
            type="button"
            variant={editorMode === "json" ? "default" : "outline"}
            size="sm"
            onClick={() => setEditorMode("json")}
          >
            <Code className="h-4 w-4 mr-1" />
            JSON
          </Button>
        </div>
        <div className="flex items-center gap-2">
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button type="button" variant="outline" size="sm">
                  <Play className="h-4 w-4 mr-1" />
                  Preview
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                Test this policy against sample requests (coming soon)
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
          <Button type="submit" disabled={isSubmitting || !name || !!jsonError}>
            {isSubmitting ? (
              <>
                <Loader2 className="h-4 w-4 mr-1 animate-spin" />
                Saving...
              </>
            ) : (
              <>
                <Save className="h-4 w-4 mr-1" />
                {initialData ? "Update Policy" : "Create Policy"}
              </>
            )}
          </Button>
        </div>
      </div>

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

      {/* Quick Presets */}
      {editorMode === "visual" && (
        <div className="space-y-2">
          <Label>Quick Presets</Label>
          <div className="flex flex-wrap gap-2">
            {POLICY_PRESETS.map((preset) => (
              <TooltipProvider key={preset.id}>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => handlePresetSelect(preset.id)}
                    >
                      {PRESET_ICON_COMPONENTS[preset.icon] || <Shield className="h-4 w-4" />}
                      <span className="ml-1.5">{preset.name}</span>
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>
                    <p className="font-medium">{preset.name}</p>
                    <p className="text-xs text-muted-foreground">{preset.description}</p>
                  </TooltipContent>
                </Tooltip>
              </TooltipProvider>
            ))}
          </div>
        </div>
      )}

      {/* Editor Content */}
      {editorMode === "json" ? (
        <div className="space-y-2">
          <Label htmlFor="rules">Rules (JSON)</Label>
          <textarea
            id="rules"
            value={rulesJson}
            onChange={(e) => {
              setRulesJson(e.target.value)
              validateJson(e.target.value)
            }}
            className={`w-full h-96 p-4 font-mono text-sm border rounded-xl bg-muted/30 focus:outline-none focus:ring-2 focus:ring-ring ${
              jsonError ? "border-destructive" : ""
            }`}
            placeholder='[
  {
    "when": { "always": true },
    "then": [{ "action": "rate_limit", "window": "1m", "max_requests": 100 }]
  }
]'
          />
          {jsonError && (
            <p className="text-sm text-destructive">{jsonError}</p>
          )}
          <p className="text-xs text-muted-foreground">
            Define rules with condition → action pairs. Each rule has a &quot;when&quot; condition and &quot;then&quot; actions.
          </p>
        </div>
      ) : (
        <Tabs value={activeTab} onValueChange={(v) => setActiveTab(v as PolicyEditorTab)}>
          <TabsList className="grid w-full grid-cols-5">
            <TabsTrigger value="conditions">Conditions</TabsTrigger>
            <TabsTrigger value="guardrails">Guardrails</TabsTrigger>
            <TabsTrigger value="routing">Routing</TabsTrigger>
            <TabsTrigger value="pii">PII</TabsTrigger>
            <TabsTrigger value="actions">Actions</TabsTrigger>
          </TabsList>

          <TabsContent value="conditions" className="mt-4 space-y-4">
            <div>
              <h3 className="text-sm font-medium mb-1">When should this policy apply?</h3>
              <p className="text-xs text-muted-foreground mb-4">
                Build conditions that determine when this policy is evaluated. Use nested AND/OR groups for complex logic.
              </p>
              <ConditionBuilder
                value={currentRule.when}
                onChange={handleConditionChange}
              />
            </div>
          </TabsContent>

          <TabsContent value="guardrails" className="mt-4 space-y-4">
            <div>
              <h3 className="text-sm font-medium mb-1">Content Guardrails</h3>
              <p className="text-xs text-muted-foreground mb-4">
                Configure content filtering to block harmful, dangerous, or unwanted content.
              </p>
              <GuardrailsTab
                value={guardrailAction as any}
                onChange={handleGuardrailsChange}
              />
            </div>
          </TabsContent>

          <TabsContent value="routing" className="mt-4 space-y-4">
            <div>
              <h3 className="text-sm font-medium mb-1">Dynamic Routing</h3>
              <p className="text-xs text-muted-foreground mb-4">
                Configure load balancing and model routing strategies.
              </p>
              <RoutingTab
                value={routingAction as any}
                onChange={handleRoutingChange}
              />
            </div>
          </TabsContent>

          <TabsContent value="pii" className="mt-4 space-y-4">
            <div>
              <h3 className="text-sm font-medium mb-1">PII Redaction</h3>
              <p className="text-xs text-muted-foreground mb-4">
                Detect and redact personally identifiable information from requests and responses.
              </p>
              <PIITab
                value={piiAction as any}
                onChange={handlePIIChange}
              />
            </div>
          </TabsContent>

          <TabsContent value="actions" className="mt-4 space-y-4">
            <div>
              <h3 className="text-sm font-medium mb-1">Additional Actions</h3>
              <p className="text-xs text-muted-foreground mb-4">
                Configure rate limiting, logging, webhooks, and other actions.
              </p>
              <ActionsTab
                value={currentRule}
                onChange={handleActionsChange}
              />
            </div>
          </TabsContent>
        </Tabs>
      )}

      {/* Cancel Button */}
      <div className="flex gap-3 pt-4 border-t">
        <Button type="button" variant="outline" onClick={() => window.history.back()}>
          <ArrowLeft className="h-4 w-4 mr-1" />
          Cancel
        </Button>
      </div>
    </form>
  )
}

export default PolicyForm