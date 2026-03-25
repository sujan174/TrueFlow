"use client"

import { useState } from "react"
import { Button } from "@/components/ui/button"
import { Label } from "@/components/ui/label"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"
import { Badge } from "@/components/ui/badge"
import {
  Plus,
  Shield,
  Route,
  Lock,
  Clock,
  ChevronDown,
  ChevronRight,
} from "lucide-react"
import { ActionCard } from "./action-card"
import type { Action, ActionType, PolicyEditorTab } from "@/lib/types/policy"

// ============================================================================
// Types
// ============================================================================

interface ActionRegistryProps {
  actions: Action[]
  onEditAction: (actionType: ActionType) => void
  onRemoveAction: (actionType: ActionType) => void
  onAddAction: (category: ActionCategory) => void
}

type ActionCategory = 'guardrails' | 'routing' | 'pii' | 'actions'

// ============================================================================
// Category Definitions
// ============================================================================

const ACTION_CATEGORIES: Record<ActionCategory, {
  label: string
  icon: React.ReactNode
  actionTypes: ActionType[]
  tab: PolicyEditorTab
}> = {
  guardrails: {
    label: 'Guardrails',
    icon: <Shield className="h-4 w-4" />,
    actionTypes: ['content_filter', 'external_guardrail'],
    tab: 'guardrails',
  },
  routing: {
    label: 'Routing',
    icon: <Route className="h-4 w-4" />,
    actionTypes: ['dynamic_route', 'conditional_route', 'split'],
    tab: 'routing',
  },
  pii: {
    label: 'PII & Privacy',
    icon: <Lock className="h-4 w-4" />,
    actionTypes: ['redact'],
    tab: 'pii',
  },
  actions: {
    label: 'Actions',
    icon: <Clock className="h-4 w-4" />,
    actionTypes: [
      'rate_limit', 'throttle', 'deny', 'allow', 'require_approval',
      'log', 'tag', 'webhook', 'override', 'transform', 'validate_schema', 'tool_scope'
    ],
    tab: 'actions',
  },
}

// ============================================================================
// Helper Functions
// ============================================================================

function getActionCategory(actionType: ActionType): ActionCategory {
  for (const [category, config] of Object.entries(ACTION_CATEGORIES)) {
    if (config.actionTypes.includes(actionType)) {
      return category as ActionCategory
    }
  }
  return 'actions'
}

// ============================================================================
// Main Component
// ============================================================================

export function ActionRegistry({
  actions,
  onEditAction,
  onRemoveAction,
  onAddAction,
}: ActionRegistryProps) {
  const [expandedActions, setExpandedActions] = useState<Set<ActionType>>(new Set())
  const [isRegistryExpanded, setIsRegistryExpanded] = useState(true)

  const toggleActionExpand = (actionType: ActionType) => {
    setExpandedActions(prev => {
      const next = new Set(prev)
      if (next.has(actionType)) {
        next.delete(actionType)
      } else {
        next.add(actionType)
      }
      return next
    })
  }

  // Group actions by category
  const actionsByCategory: Record<ActionCategory, Action[]> = {
    guardrails: [],
    routing: [],
    pii: [],
    actions: [],
  }

  actions.forEach(action => {
    const category = getActionCategory(action.action)
    actionsByCategory[category].push(action)
  })

  const totalActions = actions.length
  const hasActions = totalActions > 0

  return (
    <div className="space-y-3">
      {/* Header */}
      <Collapsible open={isRegistryExpanded} onOpenChange={setIsRegistryExpanded}>
        <CollapsibleTrigger asChild>
          <button className="flex items-center gap-2 w-full text-left">
            {isRegistryExpanded ? (
              <ChevronDown className="h-4 w-4 text-muted-foreground" />
            ) : (
              <ChevronRight className="h-4 w-4 text-muted-foreground" />
            )}
            <Label className="text-sm font-medium cursor-pointer">
              Active Actions
            </Label>
            <Badge variant={hasActions ? "default" : "secondary"}>
              {totalActions}
            </Badge>
          </button>
        </CollapsibleTrigger>

        <CollapsibleContent>
          {/* Actions Grid */}
          {hasActions ? (
            <div className="mt-3 space-y-4">
              {Object.entries(actionsByCategory).map(([category, categoryActions]) => {
                if (categoryActions.length === 0) return null

                const config = ACTION_CATEGORIES[category as ActionCategory]
                return (
                  <div key={category}>
                    <div className="flex items-center gap-2 mb-2">
                      <div className="p-1.5 rounded bg-muted">
                        {config.icon}
                      </div>
                      <span className="text-xs font-medium text-muted-foreground uppercase">
                        {config.label}
                      </span>
                      <Badge variant="outline" className="text-xs">
                        {categoryActions.length}
                      </Badge>
                    </div>
                    <div className="grid gap-2 md:grid-cols-2">
                      {categoryActions.map((action, index) => (
                        <ActionCard
                          key={`${action.action}-${index}`}
                          action={action}
                          onEdit={() => onEditAction(action.action)}
                          onRemove={() => onRemoveAction(action.action)}
                          isExpanded={expandedActions.has(action.action)}
                          onToggleExpand={() => toggleActionExpand(action.action)}
                        />
                      ))}
                    </div>
                  </div>
                )
              })}
            </div>
          ) : (
            <div className="mt-3 p-6 bg-muted/30 rounded-xl text-center">
              <p className="text-sm text-muted-foreground">
                No actions configured. Add actions using the buttons below or switch to a tab.
              </p>
            </div>
          )}

          {/* Add Action Buttons */}
          <div className="mt-4 flex flex-wrap gap-2">
            {Object.entries(ACTION_CATEGORIES).map(([category, config]) => {
              const hasExisting = actionsByCategory[category as ActionCategory].length > 0
              return (
                <Button
                  key={category}
                  variant="outline"
                  size="sm"
                  onClick={() => onAddAction(category as ActionCategory)}
                  className="gap-1.5"
                >
                  <Plus className="h-3.5 w-3.5" />
                  {config.icon}
                  Add {config.label}
                  {hasExisting && (
                    <Badge variant="secondary" className="ml-1 text-xs">
                      {actionsByCategory[category as ActionCategory].length}
                    </Badge>
                  )}
                </Button>
              )
            })}
          </div>
        </CollapsibleContent>
      </Collapsible>
    </div>
  )
}

export default ActionRegistry