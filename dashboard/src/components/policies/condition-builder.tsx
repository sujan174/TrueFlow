"use client"

import { useState, useCallback, useMemo } from "react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip"
import { Plus, Trash2, Parentheses } from "lucide-react"
import type {
  Condition,
  ConditionCheck,
  ConditionAll,
  ConditionAny,
  ConditionNot,
  ConditionAlways,
  ConditionOperator,
} from "@/lib/types/policy"
import {
  CONDITION_FIELDS as FIELDS,
  OPERATOR_INFO as OPERATORS,
} from "@/lib/types/policy"
import { FIELD_CATEGORIES, getCategoryById, getFieldByPath } from "@/lib/field-registry"
import { CategorySelector } from "./condition-category-selector"
import { ConditionFieldInput } from "./condition-field-input"
import { CustomFieldInput } from "./custom-field-input"

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

function isConditionAlways(c: Condition): c is ConditionAlways {
  return 'always' in c
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

/**
 * Derive the category ID from a field path.
 * This enables backward compatibility - existing conditions will show
 * with their correct category selected.
 */
function deriveCategoryFromField(fieldPath: string): string {
  // First, try to find the field in the new registry
  const fieldDef = getFieldByPath(fieldPath)
  if (fieldDef) {
    // Find which category contains this field
    for (const category of FIELD_CATEGORIES) {
      if (category.fields.some(f => f.path === fieldPath)) {
        return category.id
      }
    }
  }

  // Fallback: infer category from path prefix
  if (fieldPath.startsWith('context.ip') || fieldPath.startsWith('context.network')) {
    return 'ip_network'
  }
  if (fieldPath.startsWith('context.time')) {
    return 'time'
  }
  if (fieldPath.startsWith('request.body.model')) {
    return 'model'
  }
  if (fieldPath.includes('tools') || fieldPath.includes('function')) {
    return 'tools'
  }
  if (fieldPath.startsWith('request.')) {
    return 'request'
  }
  if (fieldPath.startsWith('token.')) {
    return 'token'
  }
  if (fieldPath.startsWith('agent.')) {
    return 'agent'
  }
  if (fieldPath.startsWith('response.')) {
    return 'response'
  }
  if (fieldPath.startsWith('context.')) {
    return 'request'
  }

  // Unknown path - use custom category
  return 'custom'
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
        onChange={handleRootChange}
        depth={0}
      />
    )
  }

  // If it's an "always" condition, show a simple toggle
  if (isConditionAlways(value)) {
    return (
      <div className="bg-card border rounded-xl p-4">
        <div className="flex items-center gap-3">
          <Badge variant="secondary">Always</Badge>
          <span className="text-sm text-muted-foreground">
            This condition always {value.always ? 'matches' : 'does not match'}
          </span>
          <Button
            variant="outline"
            size="sm"
            onClick={() => onChange({ always: !value.always })}
          >
            Toggle
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onChange(createEmptyCondition())}
          >
            Convert to Check
          </Button>
        </div>
      </div>
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
                <TooltipTrigger>
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
        <SelectLogic value={logic} onChange={toggleLogic} />
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
// Logic Selector (extracted for reuse)
// ============================================================================

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"

function SelectLogic({ value, onChange }: { value: 'AND' | 'OR', onChange: () => void }) {
  return (
    <Select value={value} onValueChange={(v) => v && onChange()}>
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
  )
}

// ============================================================================
// Condition Row (Leaf) - Redesigned with Hierarchical Categories
// ============================================================================

function ConditionRow({ value, onChange, onRemove }: ConditionRowProps) {
  // Derive the initial category from the current field path
  const initialCategoryId = useMemo(() => deriveCategoryFromField(value.field), [value.field])

  // Track selected category (can be changed by user)
  const [selectedCategoryId, setSelectedCategoryId] = useState<string>(initialCategoryId)

  // Get the category object
  const selectedCategory = getCategoryById(selectedCategoryId) || FIELD_CATEGORIES[0]

  // Handle category change
  const handleCategoryChange = (categoryId: string) => {
    setSelectedCategoryId(categoryId)

    // If switching to a non-custom category with fields, reset to the first field
    const category = getCategoryById(categoryId)
    if (category && category.fields.length > 0) {
      const firstField = category.fields[0]
      onChange({
        field: firstField.path,
        op: firstField.operators[0] as ConditionOperator,
        value: ''
      })
    } else if (categoryId === 'custom') {
      // For custom, just clear the field path
      onChange({
        field: '',
        op: 'eq',
        value: ''
      })
    }
  }

  // Handle field/operator/value changes from ConditionFieldInput
  const handleFieldInputChange = (newValue: { field: string; op: ConditionOperator; value: unknown }) => {
    onChange(newValue as ConditionCheck)
  }

  // Handle custom field path change
  const handleCustomFieldChange = (fieldPath: string) => {
    const fieldDef = getFieldByPath(fieldPath)
    const newOp = fieldDef?.operators[0] || value.op
    onChange({
      ...value,
      field: fieldPath,
      op: newOp as ConditionOperator
    })
  }

  return (
    <div className="p-4 bg-muted/30 rounded-lg space-y-4">
      {/* Category Selector */}
      <div>
        <CategorySelector
          selectedCategoryId={selectedCategoryId}
          onSelect={handleCategoryChange}
        />
      </div>

      {/* Field Input (varies by category) */}
      {selectedCategoryId === 'custom' ? (
        <div className="space-y-4">
          <CustomFieldInput
            value={value.field}
            onChange={handleCustomFieldChange}
          />
          {/* Operator and Value for custom fields */}
          <OperatorValueInput
            value={value}
            onChange={onChange}
          />
        </div>
      ) : (
        <ConditionFieldInput
          category={selectedCategory}
          value={{ field: value.field, op: value.op, value: value.value }}
          onChange={handleFieldInputChange}
        />
      )}

      {/* Remove Button */}
      <div className="flex justify-end">
        <Button
          variant="ghost"
          size="sm"
          className="text-muted-foreground hover:text-destructive"
          onClick={onRemove}
        >
          <Trash2 className="h-4 w-4 mr-1" />
          Remove Condition
        </Button>
      </div>
    </div>
  )
}

// ============================================================================
// Operator & Value Input (for custom fields)
// ============================================================================

import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"

function OperatorValueInput({ value, onChange }: { value: ConditionCheck, onChange: (c: ConditionCheck) => void }) {
  // Get operators based on field definition or default set
  const fieldDef = getFieldByPath(value.field)
  const operators = fieldDef?.operators || ['eq', 'neq', 'in', 'contains', 'gt', 'gte', 'lt', 'lte', 'exists']

  const handleOperatorChange = (op: string | null) => {
    if (!op) return
    onChange({ ...value, op: op as ConditionOperator })
  }

  const handleValueChange = (val: string) => {
    let parsedValue: unknown = val

    // Type coercion based on field type
    if (fieldDef?.type === "number" && val !== "") {
      const num = parseFloat(val)
      if (!isNaN(num)) parsedValue = num
    }

    onChange({ ...value, value: parsedValue })
  }

  return (
    <div className="grid grid-cols-2 gap-4">
      {/* Operator Selector */}
      <div>
        <Label className="text-xs text-muted-foreground mb-1.5 block">Operator</Label>
        <Select value={value.op} onValueChange={handleOperatorChange}>
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {operators.map((op) => (
              <SelectItem key={op} value={op}>
                <div className="flex items-center gap-2">
                  <span className="font-medium">{OPERATORS[op]?.label || op}</span>
                  <span className="text-xs text-muted-foreground">{OPERATORS[op]?.description}</span>
                </div>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {/* Value Input */}
      {value.op !== 'exists' && (
        <div>
          <Label className="text-xs text-muted-foreground mb-1.5 block">Value</Label>
          <Input
            value={String(value.value || "")}
            onChange={(e) => handleValueChange(e.target.value)}
            placeholder={fieldDef?.placeholder || "Enter value"}
            type={fieldDef?.type === "number" ? "number" : "text"}
          />
        </div>
      )}
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
              <TooltipTrigger>
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