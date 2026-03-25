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
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
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
  Merge,
  Replace,
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
  ActionType,
} from "@/lib/types/policy"
import { POLICY_PRESETS } from "@/lib/policy-presets"

// Tab Components
import { ConditionBuilder } from "./condition-builder"
import { GuardrailsTab } from "./guardrails-tab"
import { RoutingTab } from "./routing-tab"
import { PIITab } from "./pii-tab"
import { ActionsTab } from "./actions-tab"
import { ActionRegistry } from "./action-registry"

// ============================================================================
// Types
// ============================================================================

interface PolicyFormProps {
  initialData?: PolicyRow | null
  onSubmit: (data: CreatePolicyRequest | UpdatePolicyRequest) => Promise<void>
  isSubmitting: boolean
}

type ActionCategory = 'guardrails' | 'routing' | 'pii' | 'actions'

type PresetMode = 'merge' | 'replace'

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
// Category to Tab Mapping
// ============================================================================

const CATEGORY_TO_TAB: Record<ActionCategory, PolicyEditorTab> = {
  guardrails: 'guardrails',
  routing: 'routing',
  pii: 'pii',
  actions: 'actions',
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

  // Preset confirmation state
  const [presetDialogOpen, setPresetDialogOpen] = useState(false)
  const [pendingPresetId, setPendingPresetId] = useState<string | null>(null)
  const [presetMode, setPresetMode] = useState<PresetMode>('merge')

  // Current rule being edited (simplified - just the first rule for now)
  const currentRule = rules[0] || { when: { always: true }, then: [] }
  const currentActions = Array.isArray(currentRule.then) ? currentRule.then : [currentRule.then]

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

  // ============================================================================
  // Action Management Methods (NEW)
  // ============================================================================

  const getActions = useCallback((): Action[] => {
    const then = currentRule.then
    if (!then) return []
    return Array.isArray(then) ? then : [then]
  }, [currentRule.then])

  const addAction = useCallback((action: Action) => {
    setRules(prev => {
      const newRules = [...prev]
      const currentThen = Array.isArray(newRules[0].then) ? newRules[0].then :
                          newRules[0].then ? [newRules[0].then] : []

      // Check if action type already exists
      const existingIndex = currentThen.findIndex(a => a.action === action.action)

      if (existingIndex >= 0) {
        // Update existing action of same type
        const updated = [...currentThen]
        updated[existingIndex] = action
        newRules[0] = { ...newRules[0], then: updated }
      } else {
        // Add new action
        newRules[0] = { ...newRules[0], then: [...currentThen, action] }
      }

      return newRules
    })
  }, [])

  const removeAction = useCallback((actionType: ActionType) => {
    setRules(prev => {
      const newRules = [...prev]
      const currentThen = Array.isArray(newRules[0].then) ? newRules[0].then :
                          newRules[0].then ? [newRules[0].then] : []

      const filtered = currentThen.filter(a => a.action !== actionType)
      newRules[0] = { ...newRules[0], then: filtered }

      return newRules
    })
  }, [])

  const updateAction = useCallback((actionType: ActionType, updates: Partial<Action>) => {
    setRules(prev => {
      const newRules = [...prev]
      const currentThen = Array.isArray(newRules[0].then) ? newRules[0].then :
                          newRules[0].then ? [newRules[0].then] : []

      const updated = currentThen.map(a =>
        a.action === actionType ? { ...a, ...updates } as Action : a
      )

      newRules[0] = { ...newRules[0], then: updated }
      return newRules
    })
  }, [])

  const hasAction = useCallback((actionType: ActionType): boolean => {
    return currentActions.some(a => a.action === actionType)
  }, [currentActions])

  const getAction = useCallback(<T extends Action>(actionType: ActionType): T | undefined => {
    return currentActions.find(a => a.action === actionType) as T | undefined
  }, [currentActions])

  // ============================================================================
  // Condition Handler
  // ============================================================================

  const handleConditionChange = useCallback((condition: Condition) => {
    setRules(prev => {
      const newRules = [...prev]
      newRules[0] = { ...newRules[0], when: condition }
      return newRules
    })
  }, [])

  // ============================================================================
  // Tab-Specific Handlers (Updated to use new action management)
  // ============================================================================

  const handleGuardrailsChange = useCallback((action: Action | null) => {
    if (!action) {
      removeAction('content_filter')
    } else {
      addAction(action)
    }
  }, [addAction, removeAction])

  const handleRoutingChange = useCallback((action: Action | null) => {
    if (!action) {
      removeAction('dynamic_route')
    } else {
      addAction(action)
    }
  }, [addAction, removeAction])

  const handlePIIChange = useCallback((action: Action | null) => {
    if (!action) {
      removeAction('redact')
    } else {
      addAction(action)
    }
  }, [addAction, removeAction])

  const handleActionsChange = useCallback((rule: Rule | null) => {
    if (!rule) return
    // This is for the old ActionsTab - we'll update it to use new methods
    setRules([rule])
  }, [])

  // ============================================================================
  // Preset Handling (NEW: Merge vs Replace)
  // ============================================================================

  const handlePresetClick = (presetId: string) => {
    const currentActionCount = getActions().length

    if (currentActionCount > 0) {
      // Show confirmation dialog
      setPendingPresetId(presetId)
      setPresetDialogOpen(true)
    } else {
      // No existing actions, just apply
      applyPreset(presetId, 'merge')
    }
  }

  const applyPreset = (presetId: string, mode: PresetMode) => {
    const preset = POLICY_PRESETS.find(p => p.id === presetId)
    if (!preset) return

    if (mode === 'replace') {
      setRules(preset.rules)
    } else {
      // Merge preset actions with existing
      setRules(prev => {
        const existingThen = Array.isArray(prev[0].then) ? prev[0].then :
                            prev[0].then ? [prev[0].then] : []
        const presetThen = Array.isArray(preset.rules[0].then) ? preset.rules[0].then :
                          preset.rules[0].then ? [preset.rules[0].then] : []

        // Remove duplicates by action type, then add new ones
        const existingTypes = new Set(existingThen.map(a => a.action))
        const newActions = presetThen.filter(a => !existingTypes.has(a.action))

        return [{
          ...prev[0],
          then: [...existingThen, ...newActions]
        }]
      })
    }

    if (!name) {
      setName(preset.name)
    }
  }

  // ============================================================================
  // Action Registry Handlers
  // ============================================================================

  const handleEditAction = useCallback((actionType: ActionType) => {
    // Find which category this action belongs to and switch to that tab
    const category = getActionCategory(actionType)
    setActiveTab(CATEGORY_TO_TAB[category])
  }, [])

  const handleAddFromRegistry = useCallback((category: ActionCategory) => {
    setActiveTab(CATEGORY_TO_TAB[category])
  }, [])

  // ============================================================================
  // Helper Functions
  // ============================================================================

  function getActionCategory(actionType: ActionType): ActionCategory {
    const guardrailsTypes: ActionType[] = ['content_filter', 'external_guardrail']
    const routingTypes: ActionType[] = ['dynamic_route', 'conditional_route', 'split']
    const piiTypes: ActionType[] = ['redact']

    if (guardrailsTypes.includes(actionType)) return 'guardrails'
    if (routingTypes.includes(actionType)) return 'routing'
    if (piiTypes.includes(actionType)) return 'pii'
    return 'actions'
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
  const guardrailAction = getAction<Action>('content_filter')
  const routingAction = getAction<Action>('dynamic_route')
  const piiAction = getAction<Action>('redact')

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
              <TooltipTrigger>
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
          <div className="flex items-center gap-2">
            <Label>Quick Presets</Label>
            <Badge variant="outline" className="text-xs">
              {presetMode === 'merge' ? 'Merge mode' : 'Replace mode'}
            </Badge>
          </div>
          <div className="flex flex-wrap gap-2">
            {POLICY_PRESETS.map((preset) => (
              <TooltipProvider key={preset.id}>
                <Tooltip>
                  <TooltipTrigger>
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      onClick={() => handlePresetClick(preset.id)}
                    >
                      {PRESET_ICON_COMPONENTS[preset.icon] || <Shield className="h-4 w-4" />}
                      <span className="ml-1.5">{preset.name}</span>
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>
                    <p className="font-medium">{preset.name}</p>
                    <p className="text-xs text-muted-foreground">{preset.description}</p>
                    <p className="text-xs text-muted-foreground mt-1">
                      {presetMode === 'merge' ? 'Click to merge with existing' : 'Click to replace all actions'}
                    </p>
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
        <div className="space-y-4">
          {/* Active Actions Registry (NEW) */}
          <div className="p-4 bg-muted/30 rounded-xl border">
            <ActionRegistry
              actions={currentActions}
              onEditAction={handleEditAction}
              onRemoveAction={removeAction}
              onAddAction={handleAddFromRegistry}
            />
          </div>

          {/* Tabs */}
          <Tabs value={activeTab} onValueChange={(v) => setActiveTab(v as PolicyEditorTab)}>
            <TabsList className="grid w-full grid-cols-5">
              <TabsTrigger value="conditions">Conditions</TabsTrigger>
              <TabsTrigger value="guardrails">
                Guardrails
                {guardrailAction && (
                  <Badge variant="default" className="ml-1.5 h-4 w-4 p-0 flex items-center justify-center text-[10px]">
                    1
                  </Badge>
                )}
              </TabsTrigger>
              <TabsTrigger value="routing">
                Routing
                {routingAction && (
                  <Badge variant="default" className="ml-1.5 h-4 w-4 p-0 flex items-center justify-center text-[10px]">
                    1
                  </Badge>
                )}
              </TabsTrigger>
              <TabsTrigger value="pii">
                PII
                {piiAction && (
                  <Badge variant="default" className="ml-1.5 h-4 w-4 p-0 flex items-center justify-center text-[10px]">
                    1
                  </Badge>
                )}
              </TabsTrigger>
              <TabsTrigger value="actions">
                Actions
                {currentActions.filter(a =>
                  !['content_filter', 'external_guardrail', 'dynamic_route', 'conditional_route', 'split', 'redact'].includes(a.action)
                ).length > 0 && (
                  <Badge variant="default" className="ml-1.5 h-4 w-4 p-0 flex items-center justify-center text-[10px]">
                    {currentActions.filter(a =>
                      !['content_filter', 'external_guardrail', 'dynamic_route', 'conditional_route', 'split', 'redact'].includes(a.action)
                    ).length}
                  </Badge>
                )}
              </TabsTrigger>
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
        </div>
      )}

      {/* Preset Confirmation Dialog */}
      <AlertDialog open={presetDialogOpen} onOpenChange={setPresetDialogOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Apply Preset</AlertDialogTitle>
            <AlertDialogDescription>
              You have {getActions().length} action(s) already configured. How would you like to apply this preset?
            </AlertDialogDescription>
          </AlertDialogHeader>
          <div className="flex flex-col gap-2 py-4">
            <Button
              variant={presetMode === 'merge' ? 'default' : 'outline'}
              className="justify-start"
              onClick={() => setPresetMode('merge')}
            >
              <Merge className="h-4 w-4 mr-2" />
              <div className="text-left">
                <div className="font-medium">Merge with existing</div>
                <div className="text-xs text-muted-foreground">Add preset actions to your current configuration</div>
              </div>
            </Button>
            <Button
              variant={presetMode === 'replace' ? 'default' : 'outline'}
              className="justify-start"
              onClick={() => setPresetMode('replace')}
            >
              <Replace className="h-4 w-4 mr-2" />
              <div className="text-left">
                <div className="font-medium">Replace all</div>
                <div className="text-xs text-muted-foreground">Remove all current actions and use only the preset</div>
              </div>
            </Button>
          </div>
          <AlertDialogFooter>
            <AlertDialogCancel onClick={() => setPendingPresetId(null)}>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (pendingPresetId) {
                  applyPreset(pendingPresetId, presetMode)
                  setPendingPresetId(null)
                }
              }}
            >
              Apply Preset
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

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