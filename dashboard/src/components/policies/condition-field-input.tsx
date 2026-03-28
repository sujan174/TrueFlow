"use client"

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Badge } from "@/components/ui/badge"
import { Lightbulb } from "lucide-react"
import { type FieldCategory, type FieldDefinition, getFieldByPath, validateFieldValue } from "@/lib/field-registry"
import type { ConditionOperator } from "@/lib/types/policy"
import { OPERATOR_INFO } from "@/lib/types/policy"

interface ConditionFieldInputProps {
  category: FieldCategory
  value: { field: string; op: ConditionOperator; value: unknown }
  onChange: (value: { field: string; op: ConditionOperator; value: unknown }) => void
}

export function ConditionFieldInput({ category, value, onChange }: ConditionFieldInputProps) {
  const isCustom = category.id === "custom"
  const fieldDef = getFieldByPath(value.field)

  // Get available operators based on field type
  const operators = fieldDef?.operators || ["eq", "neq", "in", "contains"]

  const handleFieldChange = (fieldPath: string | null) => {
    if (!fieldPath) return
    const newFieldDef = getFieldByPath(fieldPath)
    const newOp = newFieldDef?.operators[0] || value.op
    onChange({ ...value, field: fieldPath, op: newOp as ConditionOperator, value: "" })
  }

  const handleOperatorChange = (op: ConditionOperator) => {
    onChange({ ...value, op })
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

  // Validate the current value
  const validation = fieldDef ? validateFieldValue(fieldDef, value.value) : { valid: true }

  return (
    <div className="space-y-4">
      {/* Field Selector (or Custom Input) */}
      <div>
        <Label className="text-xs text-muted-foreground mb-1.5 block">Field</Label>
        {isCustom ? (
          <Input
            value={value.field}
            onChange={(e) => handleFieldChange(e.target.value)}
            placeholder="request.body.custom.field"
            className="font-mono text-sm"
          />
        ) : (
          <Select value={value.field} onValueChange={handleFieldChange}>
            <SelectTrigger>
              <SelectValue placeholder="Select field" />
            </SelectTrigger>
            <SelectContent>
              {category.fields.map((field) => (
                <SelectItem key={field.path} value={field.path}>
                  <div className="flex items-center gap-2">
                    <span>{field.label}</span>
                    <span className="text-xs text-muted-foreground font-mono">{field.path}</span>
                  </div>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}
        {fieldDef && (
          <p className="text-xs text-muted-foreground mt-1">{fieldDef.description}</p>
        )}
      </div>

      {/* Operator Selector */}
      <div>
        <Label className="text-xs text-muted-foreground mb-1.5 block">Operator</Label>
        <Select value={value.op} onValueChange={(v) => handleOperatorChange(v as ConditionOperator)}>
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {operators.map((op) => (
              <SelectItem key={op} value={op}>
                <div className="flex items-center gap-2">
                  <span className="font-medium">{OPERATOR_INFO[op]?.label || op}</span>
                  <span className="text-xs text-muted-foreground">{OPERATOR_INFO[op]?.description}</span>
                </div>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {/* Value Input */}
      {value.op !== "exists" && (
        <div>
          <Label className="text-xs text-muted-foreground mb-1.5 block">Value</Label>

          {/* Enum selector for fields with fixed values */}
          {fieldDef?.enum ? (
            <Select value={String(value.value)} onValueChange={(v) => onChange({ ...value, value: v })}>
              <SelectTrigger>
                <SelectValue placeholder={`Select ${fieldDef.label.toLowerCase()}`} />
              </SelectTrigger>
              <SelectContent>
                {fieldDef.enum.map((option) => (
                  <SelectItem key={option} value={option}>
                    {option}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          ) : (
            <Input
              value={String(value.value || "")}
              onChange={(e) => handleValueChange(e.target.value)}
              placeholder={fieldDef?.placeholder || "Enter value"}
              type={fieldDef?.type === "number" ? "number" : "text"}
              className={!validation.valid ? "border-destructive" : ""}
            />
          )}

          {/* Validation error */}
          {!validation.valid && (
            <p className="text-xs text-destructive mt-1">{validation.error}</p>
          )}

          {/* Examples hint */}
          {fieldDef?.examples && fieldDef.examples.length > 0 && (
            <div className="flex items-center gap-1.5 mt-2">
              <Lightbulb className="h-3 w-3 text-muted-foreground" />
              <span className="text-xs text-muted-foreground">Examples:</span>
              <div className="flex gap-1">
                {fieldDef.examples.slice(0, 3).map((example) => (
                  <Badge key={example} variant="secondary" className="text-[10px] font-mono">
                    {example}
                  </Badge>
                ))}
              </div>
            </div>
          )}

          {/* Validation hint */}
          {fieldDef?.validationHint && (
            <p className="text-xs text-muted-foreground mt-1">{fieldDef.validationHint}</p>
          )}
        </div>
      )}
    </div>
  )
}

export default ConditionFieldInput