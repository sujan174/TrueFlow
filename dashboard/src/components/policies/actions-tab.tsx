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
import { Checkbox } from "@/components/ui/checkbox"
import { Badge } from "@/components/ui/badge"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip"
import {
  Check,
  X,
  Clock,
  Timer,
  Eye,
  Pencil,
  RefreshCw,
  FileText,
  Tag,
  Link,
  type LucideIcon,
} from "lucide-react"
import type {
  Action,
  Condition,
  Rule,
  ActionRateLimit,
  ActionThrottle,
  ActionDeny,
  ActionLog,
  ActionTag,
  ActionWebhook,
  ActionOverride,
  RateLimitKey,
} from "@/lib/types/policy"

// ============================================================================
// Types
// ============================================================================

interface ActionsTabProps {
  value: Rule | null
  onChange: (rule: Rule | null) => void
}

type ActionType = Action['action']

interface ActionConfig {
  type: ActionType
  label: string
  description: string
  icon: LucideIcon
  category: 'control' | 'modify' | 'observe'
}

// ============================================================================
// Action Definitions
// ============================================================================

const ACTION_CONFIGS: ActionConfig[] = [
  // Control actions
  { type: 'allow', label: 'Allow', description: 'Explicitly allow the request', icon: Check, category: 'control' },
  { type: 'deny', label: 'Deny', description: 'Block the request with an error', icon: X, category: 'control' },
  { type: 'rate_limit', label: 'Rate Limit', description: 'Limit requests per time window', icon: Clock, category: 'control' },
  { type: 'throttle', label: 'Throttle', description: 'Add artificial delay', icon: Timer, category: 'control' },
  { type: 'require_approval', label: 'Require Approval', description: 'Human-in-the-loop approval', icon: Eye, category: 'control' },

  // Modify actions
  { type: 'override', label: 'Override', description: 'Modify request body fields', icon: Pencil, category: 'modify' },
  { type: 'transform', label: 'Transform', description: 'Apply transformations to request', icon: RefreshCw, category: 'modify' },

  // Observe actions
  { type: 'log', label: 'Log', description: 'Log request/response details', icon: FileText, category: 'observe' },
  { type: 'tag', label: 'Tag', description: 'Add metadata for audit trails', icon: Tag, category: 'observe' },
  { type: 'webhook', label: 'Webhook', description: 'Send to external webhook', icon: Link, category: 'observe' },
]

const ACTION_CATEGORIES = {
  control: { label: 'Control Flow', description: 'Allow, deny, or delay requests' },
  modify: { label: 'Modify Request', description: 'Transform or override request content' },
  observe: { label: 'Observe & Track', description: 'Log, tag, or send webhooks' },
}

// ============================================================================
// Main Component
// ============================================================================

export function ActionsTab({ value, onChange }: ActionsTabProps) {
  const [selectedActionType, setSelectedActionType] = useState<ActionType | null>(
    value?.then ? (Array.isArray(value.then) ? value.then[0]?.action : value.then.action) : null
  )

  const handleActionTypeSelect = (actionType: ActionType) => {
    setSelectedActionType(actionType)

    // Create a default action of the selected type
    const defaultAction = createDefaultAction(actionType)
    onChange({
      when: value?.when || { always: true },
      then: defaultAction,
    })
  }

  const updateAction = useCallback((actionUpdates: Partial<Action>) => {
    if (!value || !selectedActionType) return

    const currentAction = Array.isArray(value.then) ? value.then[0] : value.then
    const updatedAction = { ...currentAction, ...actionUpdates } as Action

    onChange({
      ...value,
      then: updatedAction,
    })
  }, [value, selectedActionType, onChange])

  return (
    <div className="space-y-6">
      {/* Action Type Selector */}
      <div>
        <Label className="text-sm font-medium mb-3 block">Select Action Type</Label>

        {Object.entries(ACTION_CATEGORIES).map(([categoryKey, category]) => (
          <div key={categoryKey} className="mb-4">
            <div className="flex items-center gap-2 mb-2">
              <h4 className="text-xs font-semibold text-muted-foreground uppercase">
                {category.label}
              </h4>
              <span className="text-xs text-muted-foreground">- {category.description}</span>
            </div>
            <div className="grid grid-cols-3 gap-2">
              {ACTION_CONFIGS.filter(a => a.category === categoryKey).map((action) => {
                const isSelected = selectedActionType === action.type
                const Icon = action.icon
                return (
                  <button
                    key={action.type}
                    type="button"
                    onClick={() => handleActionTypeSelect(action.type)}
                    className={`flex items-center gap-3 p-3 rounded-xl border text-left transition-all ${
                      isSelected
                        ? 'border-primary bg-primary/5 ring-2 ring-primary/20'
                        : 'border-border bg-card hover:bg-muted/50'
                    }`}
                  >
                    <div className={`p-2 rounded-lg ${isSelected ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground'}`}>
                      <Icon className="h-4 w-4" />
                    </div>
                    <div>
                      <span className={`text-sm font-medium ${isSelected ? 'text-foreground' : 'text-muted-foreground'}`}>
                        {action.label}
                      </span>
                      <p className="text-xs text-muted-foreground line-clamp-1">{action.description}</p>
                    </div>
                  </button>
                )
              })}
            </div>
          </div>
        ))}
      </div>

      {/* Action Configuration */}
      {selectedActionType && (
        <div className="p-4 bg-card border rounded-xl">
          <h4 className="text-sm font-medium mb-4">
            Configure {ACTION_CONFIGS.find(a => a.type === selectedActionType)?.label}
          </h4>
          <ActionConfigForm
            actionType={selectedActionType}
            value={Array.isArray(value?.then) ? value?.then[0] : value?.then}
            onChange={updateAction}
          />
        </div>
      )}
    </div>
  )
}

// ============================================================================
// Action Config Form (Dynamic based on action type)
// ============================================================================

interface ActionConfigFormProps {
  actionType: ActionType
  value: Action | undefined
  onChange: (updates: Partial<Action>) => void
}

function ActionConfigForm({ actionType, value, onChange }: ActionConfigFormProps) {
  switch (actionType) {
    case 'rate_limit':
      return <RateLimitConfig value={value as ActionRateLimit} onChange={onChange} />
    case 'throttle':
      return <ThrottleConfig value={value as ActionThrottle} onChange={onChange} />
    case 'deny':
      return <DenyConfig value={value as ActionDeny} onChange={onChange} />
    case 'log':
      return <LogConfig value={value as ActionLog} onChange={onChange} />
    case 'tag':
      return <TagConfig value={value as ActionTag} onChange={onChange} />
    case 'webhook':
      return <WebhookConfig value={value as ActionWebhook} onChange={onChange} />
    case 'override':
      return <OverrideConfig value={value as ActionOverride} onChange={onChange} />
    case 'allow':
      return (
        <p className="text-sm text-muted-foreground">
          This action explicitly allows the request to proceed. No configuration needed.
        </p>
      )
    default:
      return (
        <p className="text-sm text-muted-foreground">
          Configuration for this action type is available in JSON mode.
        </p>
      )
  }
}

// ============================================================================
// Individual Action Config Components
// ============================================================================

function RateLimitConfig({ value, onChange }: { value: ActionRateLimit | undefined; onChange: (updates: Partial<Action>) => void }) {
  const config = value || { action: 'rate_limit' as const, window: '1m', max_requests: 100 }

  return (
    <div className="grid grid-cols-3 gap-4">
      <div>
        <Label className="text-xs text-muted-foreground">Time Window</Label>
        <Select
          value={config.window}
          onValueChange={(v) => onChange({ window: v })}
        >
          <SelectTrigger className="mt-1">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="1s">1 second</SelectItem>
            <SelectItem value="1m">1 minute</SelectItem>
            <SelectItem value="5m">5 minutes</SelectItem>
            <SelectItem value="1h">1 hour</SelectItem>
            <SelectItem value="1d">1 day</SelectItem>
          </SelectContent>
        </Select>
      </div>
      <div>
        <Label className="text-xs text-muted-foreground">Max Requests</Label>
        <Input
          type="number"
          value={config.max_requests}
          onChange={(e) => onChange({ max_requests: parseInt(e.target.value) || 100 })}
          className="mt-1"
        />
      </div>
      <div>
        <Label className="text-xs text-muted-foreground">Rate Limit Key</Label>
        <Select
          value={config.key || 'token'}
          onValueChange={(v) => onChange({ key: v as RateLimitKey })}
        >
          <SelectTrigger className="mt-1">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="token">Per Token</SelectItem>
            <SelectItem value="ip">Per IP</SelectItem>
            <SelectItem value="user">Per User</SelectItem>
            <SelectItem value="global">Global</SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>
  )
}

function ThrottleConfig({ value, onChange }: { value: ActionThrottle | undefined; onChange: (updates: Partial<Action>) => void }) {
  const config = value || { action: 'throttle' as const, delay_ms: 1000 }

  return (
    <div className="grid grid-cols-2 gap-4">
      <div>
        <Label className="text-xs text-muted-foreground">Delay (milliseconds)</Label>
        <Input
          type="number"
          value={config.delay_ms}
          onChange={(e) => onChange({ delay_ms: parseInt(e.target.value) || 1000 })}
          className="mt-1"
        />
      </div>
      <div className="flex items-end">
        <p className="text-xs text-muted-foreground pb-2">
          Adds an artificial delay before processing the request. Useful for rate limiting without explicit counts.
        </p>
      </div>
    </div>
  )
}

function DenyConfig({ value, onChange }: { value: ActionDeny | undefined; onChange: (updates: Partial<Action>) => void }) {
  const config = value || { action: 'deny' as const }

  return (
    <div className="grid grid-cols-2 gap-4">
      <div>
        <Label className="text-xs text-muted-foreground">HTTP Status Code</Label>
        <Input
          type="number"
          value={config.status || 403}
          onChange={(e) => onChange({ status: parseInt(e.target.value) || 403 })}
          className="mt-1"
        />
      </div>
      <div>
        <Label className="text-xs text-muted-foreground">Error Message</Label>
        <Input
          value={config.message || ''}
          onChange={(e) => onChange({ message: e.target.value })}
          placeholder="Request denied by policy"
          className="mt-1"
        />
      </div>
    </div>
  )
}

function LogConfig({ value, onChange }: { value: ActionLog | undefined; onChange: (updates: Partial<Action>) => void }) {
  const config = value || { action: 'log' as const }
  const tagsJson = config.tags ? JSON.stringify(config.tags) : ''

  return (
    <div className="grid grid-cols-2 gap-4">
      <div>
        <Label className="text-xs text-muted-foreground">Log Level</Label>
        <Select
          value={config.level || 'info'}
          onValueChange={(v) => onChange({ level: v })}
        >
          <SelectTrigger className="mt-1">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="debug">Debug</SelectItem>
            <SelectItem value="info">Info</SelectItem>
            <SelectItem value="warn">Warn</SelectItem>
            <SelectItem value="error">Error</SelectItem>
          </SelectContent>
        </Select>
      </div>
      <div>
        <Label className="text-xs text-muted-foreground">Tags (JSON)</Label>
        <Input
          value={tagsJson}
          placeholder='{"key": "value"}'
          className="mt-1 font-mono text-sm"
          onChange={(e) => {
            try {
              const tags = JSON.parse(e.target.value)
              onChange({ tags })
            } catch {
              // Invalid JSON, ignore
            }
          }}
        />
      </div>
    </div>
  )
}

function TagConfig({ value, onChange }: { value: ActionTag | undefined; onChange: (updates: Partial<Action>) => void }) {
  const config = value || { action: 'tag' as const, key: '', value: '' }

  return (
    <div className="grid grid-cols-2 gap-4">
      <div>
        <Label className="text-xs text-muted-foreground">Tag Key</Label>
        <Input
          value={config.key}
          onChange={(e) => onChange({ key: e.target.value })}
          placeholder="environment"
          className="mt-1"
        />
      </div>
      <div>
        <Label className="text-xs text-muted-foreground">Tag Value</Label>
        <Input
          value={config.value}
          onChange={(e) => onChange({ value: e.target.value })}
          placeholder="production"
          className="mt-1"
        />
      </div>
    </div>
  )
}

function WebhookConfig({ value, onChange }: { value: ActionWebhook | undefined; onChange: (updates: Partial<Action>) => void }) {
  const config = value || { action: 'webhook' as const, url: '' }

  return (
    <div className="grid grid-cols-2 gap-4">
      <div className="col-span-2">
        <Label className="text-xs text-muted-foreground">Webhook URL</Label>
        <Input
          value={config.url}
          onChange={(e) => onChange({ url: e.target.value })}
          placeholder="https://example.com/webhook"
          className="mt-1"
        />
      </div>
      <div>
        <Label className="text-xs text-muted-foreground">Timeout (ms)</Label>
        <Input
          type="number"
          value={config.timeout_ms || 5000}
          onChange={(e) => onChange({ timeout_ms: parseInt(e.target.value) || 5000 })}
          className="mt-1"
        />
      </div>
    </div>
  )
}

function OverrideConfig({ value, onChange }: { value: ActionOverride | undefined; onChange: (updates: Partial<Action>) => void }) {
  const config = value || { action: 'override' as const, set_body_fields: {} }
  const [fieldsJson, setFieldsJson] = useState(JSON.stringify(config.set_body_fields, null, 2))

  // Sync local state when props change
  useEffect(() => {
    if (value?.set_body_fields) {
      setFieldsJson(JSON.stringify(value.set_body_fields, null, 2))
    }
  }, [value?.set_body_fields])

  return (
    <div>
      <Label className="text-xs text-muted-foreground">Body Fields to Override (JSON)</Label>
      <textarea
        className="w-full h-32 p-3 text-sm font-mono border rounded-xl bg-background mt-1 focus:outline-none focus:ring-2 focus:ring-ring"
        value={fieldsJson}
        onChange={(e) => {
          setFieldsJson(e.target.value)
          try {
            const fields = JSON.parse(e.target.value)
            onChange({ set_body_fields: fields })
          } catch {
            // Invalid JSON, ignore
          }
        }}
        placeholder='{"model": "gpt-4o-mini"}'
      />
      <p className="text-xs text-muted-foreground mt-2">
        Override fields in the request body. Useful for forcing model downgrades or setting defaults.
      </p>
    </div>
  )
}

// ============================================================================
// Helper Functions
// ============================================================================

function createDefaultAction(actionType: ActionType): Action {
  switch (actionType) {
    case 'allow':
      return { action: 'allow' }
    case 'deny':
      return { action: 'deny', status: 403, message: 'Request denied by policy' }
    case 'rate_limit':
      return { action: 'rate_limit', window: '1m', max_requests: 100, key: 'token' }
    case 'throttle':
      return { action: 'throttle', delay_ms: 1000 }
    case 'require_approval':
      return { action: 'require_approval', timeout: '30m', fallback: 'deny' }
    case 'override':
      return { action: 'override', set_body_fields: {} }
    case 'transform':
      return { action: 'transform', operations: [] }
    case 'log':
      return { action: 'log', level: 'info' }
    case 'tag':
      return { action: 'tag', key: '', value: '' }
    case 'webhook':
      return { action: 'webhook', url: '', timeout_ms: 5000 }
    default:
      return { action: 'allow' }
  }
}

export default ActionsTab