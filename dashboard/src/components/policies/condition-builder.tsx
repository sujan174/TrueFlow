"use client"

import { useState, useCallback } from "react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Badge } from "@/components/ui/badge"
import { Label } from "@/components/ui/label"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip"
import { Plus, Trash2, Parentheses, Minus } from "lucide-react"
import type {
  Condition,
  ConditionCheck,
  ConditionAll,
  ConditionAny,
  ConditionNot,
  ConditionOperator,
} from "@/lib/types/policy"
import {
  CONDITION_FIELDS as FIELDS,
  OPERATOR_INFO as OPERATORS,
} from "@/lib/types/policy"

// ============================================================================
// Types
// ============================================================================

interface ConditionBuilderProps {
  value: Condition
  onChange: (condition: Condition) => void
}

interface ConditionGroupProps {
  value: ConditionAll | ConditionAny | ConditionNot
  onChange: (condition: ConditionAll | ConditionAny | ConditionNot) => void
  depth?: number
}

interface ConditionRowProps {
  value: ConditionCheck
  onChange: (condition: ConditionCheck) => void
  onRemove: () => void
}

// ============================================================================
// Helper Functions
// ============================================================================

function isConditionCheck(c: Condition): c is ConditionCheck {
  return 'field' in c && 'op' in c
}

function isConditionAll(c: Condition): c is ConditionAll {
  return 'all' in c
}

function isConditionAny(c: Condition): c is ConditionAny {
  return 'any' in c
}

function isConditionNot(c: Condition): c is ConditionNot {
  return 'not' in c
}

function createEmptyCondition(): ConditionCheck {
  return { field: 'request.path', op: 'eq', value: '' }
}

function createEmptyGroup(logic: 'AND' | 'OR'): ConditionAll | ConditionAny {
  if (logic === 'AND') {
    return { all: [createEmptyCondition()] }
  }
  return { any: [createEmptyCondition()] }
}

// Group fields by category for the dropdown
const FIELDS_BY_CATEGORY = {
  request: FIELDS.filter(f => f.category === 'request'),
  response: FIELDS.filter(f => f.category === 'response'),
  token: FIELDS.filter(f => f.category === 'token'),
  agent: FIELDS.filter(f => f.category === 'agent'),
  context: FIELDS.filter(f => f.category === 'context'),
  usage: FIELDS.filter(f => f.category === 'usage'),
}

// ============================================================================
// Main Component
// ============================================================================

export function ConditionBuilder({ value, onChange }: ConditionBuilderProps) {
  const handleRootChange = useCallback((newCondition: Condition) => {
    onChange(newCondition)
  }, [onChange])

  const wrapInGroup = (logic: 'AND' | 'OR') => {
    const group = createEmptyGroup(logic)
    if (isConditionAll(group)) {
      group.all = [value]
    } else {
      group.any = [value]
    }
    onChange(group)
  }

  // If it's a NOT condition, handle separately
  if (isConditionNot(value)) {
    return (
      <div className="bg-card border rounded-xl p-4 border-l-2 border-l-amber-500">
        <div className="flex items-center gap-2 mb-3">
          <Badge variant="secondary">NOT</Badge>
          <span className="text-sm text-muted-foreground">Negate the condition below</span>
        </div>
        <ConditionBuilder
          value={value.not}
          onChange={(newCondition) => onChange({ not: newCondition })}
        />
      </div>
    )
  }

  // If it's a group, render the group component
  if (isConditionAll(value) || isConditionAny(value)) {
    return (
      <ConditionGroup
        value={value}
        onChange={handleRootChange as (c: ConditionAll | ConditionAny) => void}
        depth={0}
      />
    )
  }

  // If it's a leaf condition, wrap in a container
  return (
    <div className="space-y-4">
      <div className="bg-card border rounded-xl p-4">
        <div className="flex items-center justify-between mb-4">
          <span className="text-sm font-medium">Condition</span>
          <div className="flex gap-2">
            <TooltipProvider>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button variant="outline" size="sm" onClick={() => wrapInGroup('AND')}>
                    <Parentheses className="h-4 w-4 mr-1" />
                    Wrap in AND
                  </Button>
                </TooltipTrigger>
                <TooltipContent>Group this condition with others using AND logic</TooltipContent>
              </Tooltip>
            </TooltipProvider>
            <Button variant="outline" size="sm" onClick={() => wrapInGroup('OR')}>
              <Parentheses className="h-4 w-4 mr-1" />
              Wrap in OR
            </Button>
          </div>
        </div>
        <ConditionRow
          value={value}
          onChange={handleRootChange}
          onRemove={() => onChange(createEmptyCondition())}
        />
      </div>
    </div>
  )
}

// ============================================================================
// Condition Group (AND/OR/NOT)
// ============================================================================

function ConditionGroup({ value, onChange, depth = 0 }: ConditionGroupProps) {
  const isAnd = isConditionAll(value)
  const isOr = isConditionAny(value)
  const isNot = isConditionNot(value)
  const logic: 'AND' | 'OR' = isAnd ? 'AND' : isOr ? 'OR' : 'AND'
  const children: Condition[] = isNot
    ? [value.not]
    : isAnd
      ? value.all
      : (value as ConditionAny).any

  const toggleLogic = () => {
    if (isAnd) {
      onChange({ any: value.all })
    } else if (isOr) {
      onChange({ all: (value as ConditionAny).any })
    }
  }

  const addChild = () => {
    const newChild = createEmptyCondition()
    if (isAnd) {
      onChange({ all: [...value.all, newChild] })
    } else if (isOr) {
      onChange({ any: [...(value as ConditionAny).any, newChild] })
    }
  }

  const addGroup = (groupLogic: 'AND' | 'OR') => {
    const newGroup = createEmptyGroup(groupLogic)
    if (isAnd) {
      onChange({ all: [...value.all, newGroup] })
    } else if (isOr) {
      onChange({ any: [...(value as ConditionAny).any, newGroup] })
    }
  }

  const updateChild = (index: number, newChild: Condition) => {
    if (isAnd) {
      const newChildren = [...value.all]
      newChildren[index] = newChild
      onChange({ all: newChildren })
    } else if (isOr) {
      const newChildren = [...(value as ConditionAny).any]
      newChildren[index] = newChild
      onChange({ any: newChildren })
    }
  }

  const removeChild = (index: number) => {
    if (isAnd) {
      const newChildren = value.all.filter((_, i) => i !== index)
      if (newChildren.length === 0) {
        onChange({ all: [createEmptyCondition()] })
      } else if (newChildren.length === 1 && (isConditionAll(newChildren[0]) || isConditionAny(newChildren[0]))) {
        onChange(newChildren[0])
      } else {
        onChange({ all: newChildren })
      }
    } else if (isOr) {
      const newChildren = (value as ConditionAny).any.filter((_, i) => i !== index)
      if (newChildren.length === 0) {
        onChange({ any: [createEmptyCondition()] })
      } else if (newChildren.length === 1 && (isConditionAll(newChildren[0]) || isConditionAny(newChildren[0]))) {
        onChange(newChildren[0])
      } else {
        onChange({ any: newChildren })
      }
    }
  }

  return (
    <div
      className={`bg-card border rounded-xl p-4 ${depth > 0 ? 'ml-4 border-l-2 border-l-primary/30' : ''}`}
    >
      {/* Group Header */}
      <div className="flex items-center gap-3 mb-4">
        <Select value={logic} onValueChange={(v) => v && toggleLogic()}>
          <SelectTrigger className="w-[180px]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="AND">
              <div className="flex items-center gap-2">
                <Badge variant="default">AND</Badge>
                <span className="text-muted-foreground text-xs">All must match</span>
              </div>
            </SelectItem>
            <SelectItem value="OR">
              <div className="flex items-center gap-2">
                <Badge variant="secondary">OR</Badge>
                <span className="text-muted-foreground text-xs">Any must match</span>
              </div>
            </SelectItem>
          </SelectContent>
        </Select>
        <span className="text-sm text-muted-foreground">
          {logic === 'AND' ? 'Match all conditions below' : 'Match any condition below'}
        </span>
      </div>

      {/* Children */}
      <div className="space-y-2">
        {children.map((child, index) => {
          if (isConditionCheck(child)) {
            return (
              <ConditionRow
                key={index}
                value={child}
                onChange={(c) => updateChild(index, c)}
                onRemove={() => removeChild(index)}
              />
            )
          }
          // Nested group
          if (isConditionNot(child)) {
            return (
              <div key={index} className="relative bg-card border rounded-xl p-4 border-l-2 border-l-amber-500 ml-4">
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2">
                    <Badge variant="secondary">NOT</Badge>
                    <span className="text-sm text-muted-foreground">Negate</span>
                  </div>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    className="text-muted-foreground hover:text-destructive"
                    onClick={() => removeChild(index)}
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
                <ConditionBuilder
                  value={child.not}
                  onChange={(newCondition) => updateChild(index, { not: newCondition })}
                />
              </div>
            )
          }
          if (isConditionAll(child) || isConditionAny(child)) {
            return (
              <div key={index} className="relative">
                <ConditionGroup
                  value={child}
                  onChange={(c) => updateChild(index, c)}
                  depth={depth + 1}
                />
                <Button
                  variant="ghost"
                  size="icon-sm"
                  className="absolute top-2 right-2 text-muted-foreground hover:text-destructive"
                  onClick={() => removeChild(index)}
                >
                  <Trash2 className="h-4 w-4" />
                </Button>
              </div>
            )
          }
          return null
        })}
      </div>

      {/* Add Buttons */}
      <div className="flex gap-2 mt-4 pt-4 border-t border-dashed">
        <Button variant="outline" size="sm" onClick={addChild}>
          <Plus className="h-4 w-4 mr-1" />
          Add Condition
        </Button>
        <Button variant="outline" size="sm" onClick={() => addGroup('AND')}>
          <Parentheses className="h-4 w-4 mr-1" />
          AND Group
        </Button>
        <Button variant="outline" size="sm" onClick={() => addGroup('OR')}>
          <Parentheses className="h-4 w-4 mr-1" />
          OR Group
        </Button>
      </div>
    </div>
  )
}

// ============================================================================
// Condition Row (Leaf)
// ============================================================================

function ConditionRow({ value, onChange, onRemove }: ConditionRowProps) {
  const fieldDef = FIELDS.find(f => f.name === value.field) || FIELDS[0]
  const availableOperators = fieldDef.operators

  const handleFieldChange = (fieldName: string) => {
    const newFieldDef = FIELDS.find(f => f.name === fieldName) || FIELDS[0]
    const newOp = newFieldDef.operators.includes(value.op) ? value.op : newFieldDef.operators[0]
    onChange({ ...value, field: fieldName, op: newOp })
  }

  const handleOperatorChange = (op: string) => {
    onChange({ ...value, op: op as ConditionOperator })
  }

  const handleValueChange = (val: string) => {
    let parsedValue: string | number = val
    if (value.op !== 'in' && fieldDef.valueType === 'number' && val !== '') {
      const num = parseFloat(val)
      if (!isNaN(num)) {
        parsedValue = num
      }
    }
    onChange({ ...value, value: parsedValue })
  }

  return (
    <div className="flex items-center gap-2 p-3 bg-muted/30 rounded-lg">
      {/* Field Selector */}
      <Select value={value.field} onValueChange={handleFieldChange}>
        <SelectTrigger className="w-[180px]">
          <SelectValue placeholder="Select field" />
        </SelectTrigger>
        <SelectContent>
          {Object.entries(FIELDS_BY_CATEGORY).map(([category, fields]) => (
            <div key={category}>
              <div className="px-2 py-1 text-xs font-semibold text-muted-foreground uppercase">
                {category}
              </div>
              {fields.map((field) => (
                <SelectItem key={field.name} value={field.name}>
                  {field.label}
                </SelectItem>
              ))}
            </div>
          ))}
        </SelectContent>
      </Select>

      {/* Operator Selector */}
      <Select value={value.op} onValueChange={handleOperatorChange}>
        <SelectTrigger className="w-[140px]">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {availableOperators.map((op) => (
            <SelectItem key={op} value={op}>
              {OPERATORS[op]?.label || op}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      {/* Value Input */}
      {value.op !== 'exists' && (
        <Input
          className="flex-1"
          value={String(value.value)}
          onChange={(e) => handleValueChange(e.target.value)}
          placeholder={value.op === 'in' ? 'value1, value2, value3' : 'Enter value'}
          type={fieldDef.valueType === 'number' ? 'number' : 'text'}
        />
      )}

      {/* Remove Button */}
      <Button
        variant="ghost"
        size="icon-sm"
        className="text-muted-foreground hover:text-destructive"
        onClick={onRemove}
      >
        <Trash2 className="h-4 w-4" />
      </Button>
    </div>
  )
}

// ============================================================================
// Field Reference Component (shows all available fields)
// ============================================================================

interface FieldReferenceProps {
  onFieldClick: (fieldName: string) => void
}

export function ConditionFieldReference({ onFieldClick }: FieldReferenceProps) {
  return (
    <div className="p-4 bg-muted/30 rounded-xl">
      <h4 className="text-sm font-medium text-muted-foreground mb-3">
        Available Fields <span className="font-normal">(click to add)</span>
      </h4>
      <div className="flex flex-wrap gap-1.5">
        {FIELDS.map((field) => (
          <TooltipProvider key={field.name}>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 px-2 text-xs bg-background border"
                  onClick={() => onFieldClick(field.name)}
                >
                  {field.label}
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                <p className="font-medium">{field.name}</p>
                <p className="text-xs text-muted-foreground">{field.description}</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        ))}
      </div>
    </div>
  )
}

export default ConditionBuilder