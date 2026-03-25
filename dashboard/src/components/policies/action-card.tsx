"use client"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
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
  Shield,
  Route,
  Lock,
  Trash2,
  ChevronDown,
  ChevronUp,
  type LucideIcon,
} from "lucide-react"
import { useState } from "react"
import type { Action, ActionType } from "@/lib/types/policy"
import { getActionDisplayName } from "@/lib/types/policy"

// ============================================================================
// Types
// ============================================================================

interface ActionCardProps {
  action: Action
  onEdit: () => void
  onRemove: () => void
  isExpanded?: boolean
  onToggleExpand?: () => void
}

// ============================================================================
// Icon Mapping
// ============================================================================

const ACTION_ICONS: Record<ActionType, LucideIcon> = {
  allow: Check,
  deny: X,
  rate_limit: Clock,
  throttle: Timer,
  require_approval: Eye,
  override: Pencil,
  transform: RefreshCw,
  log: FileText,
  tag: Tag,
  webhook: Link,
  content_filter: Shield,
  split: Route,
  dynamic_route: Route,
  validate_schema: FileText,
  conditional_route: Route,
  external_guardrail: Shield,
  redact: Lock,
  tool_scope: Lock,
}

// ============================================================================
// Summary Generators
// ============================================================================

function getActionSummary(action: Action): string {
  switch (action.action) {
    case 'rate_limit':
      return `${action.max_requests} req/${action.window} per ${action.key || 'token'}`
    case 'throttle':
      return `${action.delay_ms}ms delay`
    case 'deny':
      return action.message || `HTTP ${action.status || 403}`
    case 'redact':
      const patterns = action.patterns?.join(', ') || 'none'
      return `${action.direction || 'both'}: ${patterns}`
    case 'content_filter':
      const enabled = [
        action.block_jailbreak && 'jailbreak',
        action.block_harmful && 'harmful',
        action.block_code_injection && 'injection',
        action.block_profanity && 'profanity',
        action.block_bias && 'bias',
        action.block_ip_leakage && 'ip_leakage',
      ].filter(Boolean)
      return enabled.length > 0 ? enabled.slice(0, 3).join(', ') + (enabled.length > 3 ? '...' : '') : 'none enabled'
    case 'dynamic_route':
      return action.strategy.replace(/_/g, ' ')
    case 'log':
      return action.level || 'info'
    case 'tag':
      return `${action.key}=${action.value}`
    case 'webhook':
      try {
        const url = new URL(action.url)
        return url.hostname
      } catch {
        return 'configure URL'
      }
    case 'override':
      return Object.keys(action.set_body_fields || {}).join(', ') || 'no fields'
    case 'transform':
      return `${action.operations?.length || 0} operations`
    case 'require_approval':
      return `timeout: ${action.timeout || '30m'}`
    case 'allow':
      return 'always allow'
    default:
      return 'configured'
  }
}

function getDetailedConfig(action: Action): string[] {
  const details: string[] = []

  switch (action.action) {
    case 'rate_limit':
      details.push(`Max Requests: ${action.max_requests}`)
      details.push(`Window: ${action.window}`)
      details.push(`Key: ${action.key || 'token'}`)
      break
    case 'content_filter':
      if (action.block_jailbreak) details.push('Block jailbreak attempts')
      if (action.block_harmful) details.push('Block harmful content')
      if (action.block_code_injection) details.push('Block code injection')
      if (action.block_profanity) details.push('Block profanity')
      if (action.block_bias) details.push('Block bias')
      if (action.block_competitor_mention) details.push('Block competitor mentions')
      if (action.block_sensitive_topics) details.push('Block sensitive topics')
      if (action.block_gibberish) details.push('Block gibberish')
      if (action.block_contact_info) details.push('Block contact info')
      if (action.block_ip_leakage) details.push('Block IP leakage')
      if (action.risk_threshold) details.push(`Risk threshold: ${action.risk_threshold}`)
      break
    case 'redact':
      details.push(`Direction: ${action.direction || 'both'}`)
      if (action.patterns?.length) details.push(`Patterns: ${action.patterns.join(', ')}`)
      if (action.fields?.length) details.push(`Fields: ${action.fields.join(', ')}`)
      details.push(`On match: ${action.on_match || 'redact'}`)
      break
    case 'dynamic_route':
      details.push(`Strategy: ${action.strategy}`)
      if (action.pool?.length) {
        details.push(`Models: ${action.pool.map(p => p.model).join(', ')}`)
      }
      break
    case 'deny':
      details.push(`Status: ${action.status || 403}`)
      if (action.message) details.push(`Message: ${action.message}`)
      break
    case 'throttle':
      details.push(`Delay: ${action.delay_ms}ms`)
      break
    case 'log':
      details.push(`Level: ${action.level || 'info'}`)
      if (action.tags) details.push(`Tags: ${JSON.stringify(action.tags)}`)
      break
    case 'tag':
      details.push(`Key: ${action.key}`)
      details.push(`Value: ${action.value}`)
      break
    case 'webhook':
      details.push(`URL: ${action.url}`)
      details.push(`Timeout: ${action.timeout_ms || 5000}ms`)
      break
    case 'override':
      details.push(`Fields: ${JSON.stringify(action.set_body_fields, null, 2)}`)
      break
    case 'transform':
      action.operations?.forEach((op, i) => {
        details.push(`${i + 1}. ${op.type}`)
      })
      break
    case 'require_approval':
      details.push(`Timeout: ${action.timeout || '30m'}`)
      details.push(`Fallback: ${action.fallback || 'deny'}`)
      break
    default:
      break
  }

  return details
}

// ============================================================================
// Main Component
// ============================================================================

export function ActionCard({ action, onEdit, onRemove, isExpanded = false, onToggleExpand }: ActionCardProps) {
  const Icon = ACTION_ICONS[action.action] || Shield
  const summary = getActionSummary(action)
  const details = isExpanded ? getDetailedConfig(action) : []

  return (
    <div className="bg-card border rounded-xl overflow-hidden transition-all">
      {/* Header */}
      <div className="flex items-center gap-3 p-3">
        <div className="p-2 rounded-lg bg-primary/10 text-primary">
          <Icon className="h-4 w-4" />
        </div>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium truncate">
              {getActionDisplayName(action)}
            </span>
            <Badge variant="secondary" className="text-xs">
              {action.action}
            </Badge>
          </div>
          <p className="text-xs text-muted-foreground truncate">{summary}</p>
        </div>
        <div className="flex items-center gap-1">
          {onToggleExpand && details.length > 0 && (
            <TooltipProvider>
              <Tooltip>
                <TooltipTrigger>
                  <Button variant="ghost" size="icon-sm" onClick={onToggleExpand}>
                    {isExpanded ? (
                      <ChevronUp className="h-4 w-4" />
                    ) : (
                      <ChevronDown className="h-4 w-4" />
                    )}
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  {isExpanded ? 'Collapse' : 'Show details'}
                </TooltipContent>
              </Tooltip>
            </TooltipProvider>
          )}
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger>
                <Button variant="ghost" size="icon-sm" onClick={onEdit}>
                  <Pencil className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Edit action</TooltipContent>
            </Tooltip>
          </TooltipProvider>
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  className="text-muted-foreground hover:text-destructive"
                  onClick={onRemove}
                >
                  <Trash2 className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Remove action</TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
      </div>

      {/* Expanded Details */}
      {isExpanded && details.length > 0 && (
        <div className="px-4 pb-3 pt-0 border-t bg-muted/30">
          <div className="pt-3 space-y-1">
            {details.map((detail, i) => (
              <p key={i} className="text-xs text-muted-foreground">
                {detail}
              </p>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

export default ActionCard