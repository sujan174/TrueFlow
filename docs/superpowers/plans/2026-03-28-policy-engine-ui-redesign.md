# Policy Engine UI/UX Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the policy condition builder with hierarchical categories and smart guidance while maintaining full backend flexibility.

**Architecture:** UI-only changes. The backend already supports all field paths (`context.ip`, `request.body.tools[*].function.name`, etc.) and operators. The gap is purely in the UI - users must know JSON paths manually. We add a hierarchical category selector with progressive disclosure and a custom path escape hatch.

**Tech Stack:** React, TypeScript, ShadCN UI components, Tailwind CSS

---

## Scope Analysis

### What's Already Working (No Changes Needed)

**Backend:**
- `gateway/src/middleware/fields.rs` - Full field resolution for:
  - `request.method`, `request.path`, `request.body_size`, `request.body.*`, `request.headers.*`, `request.query.*`
  - `response.status`, `response.body.*`, `response.headers.*`
  - `context.ip`, `context.time.hour`, `context.time.weekday`, `context.time.date`
  - `agent.name`, `token.id`, `token.name`, `token.project_id`, `token.purpose`
  - `usage.*` (dynamic counters)
- Wildcard array access: `tools[*].function.name`
- All operators: eq, neq, gt, gte, lt, lte, in, glob, regex, contains, exists, starts_with, ends_with

**Frontend:**
- `dashboard/src/lib/types/policy.ts` - CONDITION_FIELDS already defined with categories
- `dashboard/src/components/policies/condition-builder.tsx` - Basic builder exists

### What Needs to Change

| Area | Scope | Impact |
|------|-------|--------|
| Condition Builder | UI redesign | Main focus |
| Token Restrictions | Add IP/tool quick restrictions | New feature |
| Field Registry | Enhanced metadata | Frontend only |
| Backend | None | No changes |

---

## File Structure

### Files to Modify

```
dashboard/src/
├── lib/
│   ├── types/
│   │   └── policy.ts                    # Enhance CONDITION_FIELDS with examples, validation hints
│   └── field-registry.ts                # NEW: Hierarchical field definitions
├── components/
│   └── policies/
│       ├── condition-builder.tsx        # MAJOR REDESIGN: Hierarchical category selector
│       ├── condition-category-selector.tsx  # NEW: Top-level category picker
│       ├── condition-field-input.tsx    # NEW: Field input with autocomplete
│       ├── condition-value-input.tsx    # NEW: Smart value input with hints
│       └── custom-field-input.tsx       # NEW: Raw JSON path input for advanced users
```

### Files to Create

```
dashboard/src/
├── lib/
│   └── field-registry.ts                # Hierarchical field definitions with metadata
├── components/
│   └── policies/
│       ├── condition-category-selector.tsx  # Category picker component
│       ├── condition-field-input.tsx    # Guided field selector
│       ├── condition-value-input.tsx    # Value input with type-aware hints
│       └── custom-field-input.tsx       # Custom path input for advanced users
```

---

## Task 1: Create Field Registry with Hierarchical Categories

**Files:**
- Create: `dashboard/src/lib/field-registry.ts`
- Test: Manual UI testing

### Design: Field Categories

```typescript
// Hierarchical structure for the category-first approach
const FIELD_CATEGORIES = {
  ip_network: {
    label: "IP / Network",
    icon: Globe,
    description: "Restrict by client IP address",
    fields: [
      { path: "context.ip", label: "Client IP", type: "string", examples: ["192.168.1.1", "10.0.0.0/8"] }
    ]
  },
  model: {
    label: "Model",
    icon: Brain,
    description: "Restrict by model name",
    fields: [
      { path: "request.body.model", label: "Model Name", type: "string", examples: ["gpt-4", "claude-3"] }
    ]
  },
  tools: {
    label: "Tools / Functions",
    icon: Wrench,
    description: "Restrict by MCP tool names",
    fields: [
      { path: "request.body.tools[*].function.name", label: "Tool Name", type: "array" }
    ]
  },
  time: {
    label: "Time",
    icon: Clock,
    description: "Time-based conditions",
    fields: [
      { path: "context.time.hour", label: "Hour of Day", type: "number", range: [0, 23] },
      { path: "context.time.weekday", label: "Day of Week", type: "string", enum: ["mon", "tue", "wed", "thu", "fri", "sat", "sun"] }
    ]
  },
  custom: {
    label: "Custom",
    icon: Code,
    description: "Enter any JSON path for advanced conditions",
    fields: [] // Dynamic - user types their own path
  }
}
```

- [ ] **Step 1: Create the field registry file**

Create `dashboard/src/lib/field-registry.ts` with:
- Category definitions (IP/Network, Model, Tools, Time, Request, Response, Token, Agent, Custom)
- Field metadata (type, operators, examples, validation rules)
- Helper functions: `getOperatorsForType()`, `getExamplesForField()`, `validateValue()`

```typescript
// dashboard/src/lib/field-registry.ts

import { Globe, Brain, Wrench, Clock, Code, Send, Shield, User, Activity } from "lucide-react"
import type { ConditionOperator } from "./types/policy"

export interface FieldDefinition {
  path: string
  label: string
  description: string
  type: "string" | "number" | "array" | "boolean"
  operators: ConditionOperator[]
  examples?: string[]
  placeholder?: string
  validationHint?: string
  enum?: string[]  // For fields with fixed values
  range?: [number, number]  // For number fields
}

export interface FieldCategory {
  id: string
  label: string
  icon: React.ComponentType<{ className?: string }>
  description: string
  color: string  // Tailwind color class prefix
  fields: FieldDefinition[]
}

export const FIELD_CATEGORIES: FieldCategory[] = [
  {
    id: "ip_network",
    label: "IP / Network",
    icon: Globe,
    description: "Restrict by client IP address or network",
    color: "blue",
    fields: [
      {
        path: "context.ip",
        label: "Client IP",
        description: "The IP address of the client making the request",
        type: "string",
        operators: ["eq", "neq", "in", "glob"],
        examples: ["192.168.1.1", "10.0.0.1", "172.16.0.0/12"],
        placeholder: "Enter IP address or CIDR",
        validationHint: "Supports single IPs or CIDR notation (e.g., 10.0.0.0/8)"
      }
    ]
  },
  {
    id: "model",
    label: "Model",
    icon: Brain,
    description: "Restrict by AI model name",
    color: "purple",
    fields: [
      {
        path: "request.body.model",
        label: "Model Name",
        description: "The model specified in the request body",
        type: "string",
        operators: ["eq", "neq", "in", "contains", "starts_with", "glob", "regex"],
        examples: ["gpt-4", "claude-3-opus", "gemini-pro"],
        placeholder: "Enter model name",
      }
    ]
  },
  {
    id: "tools",
    label: "Tools / Functions",
    icon: Wrench,
    description: "Restrict by MCP tool or function names",
    color: "amber",
    fields: [
      {
        path: "request.body.tools[*].function.name",
        label: "Tool Name",
        description: "Name of the tool/function being called",
        type: "array",
        operators: ["contains", "in"],
        examples: ["web_search", "read_file", "execute_code"],
        placeholder: "Enter tool name",
        validationHint: "Matches against any tool in the request"
      }
    ]
  },
  {
    id: "time",
    label: "Time",
    icon: Clock,
    description: "Time-based conditions for business hours",
    color: "green",
    fields: [
      {
        path: "context.time.hour",
        label: "Hour of Day",
        description: "Current hour (0-23, UTC)",
        type: "number",
        operators: ["eq", "neq", "gt", "gte", "lt", "lte", "in"],
        range: [0, 23],
        placeholder: "0-23",
        validationHint: "Hours are in UTC (0-23)"
      },
      {
        path: "context.time.weekday",
        label: "Day of Week",
        description: "Current day of the week",
        type: "string",
        operators: ["eq", "neq", "in"],
        enum: ["mon", "tue", "wed", "thu", "fri", "sat", "sun"],
        examples: ["mon", "fri"],
        placeholder: "Select day"
      }
    ]
  },
  {
    id: "request",
    label: "Request",
    icon: Send,
    description: "HTTP request properties",
    color: "slate",
    fields: [
      {
        path: "request.method",
        label: "HTTP Method",
        description: "The HTTP method (GET, POST, etc.)",
        type: "string",
        operators: ["eq", "neq", "in"],
        enum: ["GET", "POST", "PUT", "DELETE", "PATCH"],
        examples: ["POST", "GET"]
      },
      {
        path: "request.path",
        label: "Request Path",
        description: "The URL path of the request",
        type: "string",
        operators: ["eq", "neq", "contains", "starts_with", "ends_with", "glob", "regex", "in"],
        examples: ["/v1/chat/completions", "/v1/embeddings"],
        placeholder: "/v1/chat/completions"
      },
      {
        path: "request.body_size",
        label: "Body Size (bytes)",
        description: "Size of the request body in bytes",
        type: "number",
        operators: ["eq", "neq", "gt", "gte", "lt", "lte"],
        placeholder: "1024"
      }
    ]
  },
  {
    id: "token",
    label: "Token",
    icon: Shield,
    description: "Token properties",
    color: "cyan",
    fields: [
      {
        path: "token.id",
        label: "Token ID",
        description: "The virtual token ID",
        type: "string",
        operators: ["eq", "neq", "in", "contains"],
        placeholder: "tf_v1_..."
      },
      {
        path: "token.name",
        label: "Token Name",
        description: "The display name of the token",
        type: "string",
        operators: ["eq", "neq", "in", "contains", "starts_with"]
      },
      {
        path: "token.purpose",
        label: "Token Purpose",
        description: "Purpose of the token",
        type: "string",
        operators: ["eq", "neq", "in"],
        enum: ["llm", "tool", "both"]
      }
    ]
  },
  {
    id: "agent",
    label: "Agent",
    icon: User,
    description: "Agent identity properties",
    color: "indigo",
    fields: [
      {
        path: "agent.name",
        label: "Agent Name",
        description: "The name of the agent making requests",
        type: "string",
        operators: ["eq", "neq", "in", "contains", "starts_with"],
        examples: ["claude", "gpt-4-agent"],
        placeholder: "Enter agent name"
      }
    ]
  },
  {
    id: "response",
    label: "Response",
    icon: Activity,
    description: "Response properties (post-flight phase only)",
    color: "rose",
    fields: [
      {
        path: "response.status",
        label: "Response Status",
        description: "HTTP response status code",
        type: "number",
        operators: ["eq", "neq", "gt", "gte", "lt", "lte", "in"],
        examples: ["200", "429", "500"],
        placeholder: "200"
      }
    ]
  },
  {
    id: "custom",
    label: "Custom Path",
    icon: Code,
    description: "Enter any JSON path for advanced conditions",
    color: "gray",
    fields: [] // Custom fields are user-defined
  }
]

// Helper functions
export function getCategoryById(id: string): FieldCategory | undefined {
  return FIELD_CATEGORIES.find(c => c.id === id)
}

export function getFieldByPath(path: string): FieldDefinition | undefined {
  for (const category of FIELD_CATEGORIES) {
    const field = category.fields.find(f => f.path === path)
    if (field) return field
  }
  return undefined
}

export function getOperatorsForField(field: FieldDefinition): ConditionOperator[] {
  return field.operators
}

export function getExamplesForField(field: FieldDefinition): string[] {
  return field.examples || []
}

export function validateFieldValue(field: FieldDefinition, value: unknown): { valid: boolean; error?: string } {
  if (field.enum && typeof value === 'string' && !field.enum.includes(value)) {
    return { valid: false, error: `Must be one of: ${field.enum.join(', ')}` }
  }
  if (field.range && typeof value === 'number') {
    const [min, max] = field.range
    if (value < min || value > max) {
      return { valid: false, error: `Must be between ${min} and ${max}` }
    }
  }
  return { valid: true }
}
```

- [ ] **Step 2: Commit the field registry**

```bash
git add dashboard/src/lib/field-registry.ts
git commit -m "feat(policy): add hierarchical field registry for condition builder

- Define 8 field categories with icons and descriptions
- Add field metadata: type, operators, examples, validation hints
- Include helper functions for field lookup and validation"
```

---

## Task 2: Build Category Selector Component

**Files:**
- Create: `dashboard/src/components/policies/condition-category-selector.tsx`
- Test: Manual UI testing

- [ ] **Step 1: Create the category selector component**

Create `dashboard/src/components/policies/condition-category-selector.tsx`:

```typescript
"use client"

import { useState } from "react"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { cn } from "@/lib/utils"
import { FIELD_CATEGORIES, type FieldCategory } from "@/lib/field-registry"

interface CategorySelectorProps {
  selectedCategoryId: string | null
  onSelect: (categoryId: string) => void
}

export function CategorySelector({ selectedCategoryId, onSelect }: CategorySelectorProps) {
  return (
    <div className="grid grid-cols-3 gap-3">
      {FIELD_CATEGORIES.map((category) => {
        const isSelected = selectedCategoryId === category.id
        const Icon = category.icon

        return (
          <button
            key={category.id}
            type="button"
            onClick={() => onSelect(category.id)}
            className={cn(
              "flex flex-col items-center gap-2 p-4 rounded-xl border text-center transition-all",
              isSelected
                ? `border-${category.color}-500 bg-${category.color}-50 dark:bg-${category.color}-950/30 ring-2 ring-${category.color}-500/20`
                : "border-border bg-card hover:bg-muted/50"
            )}
          >
            <div className={cn(
              "p-2 rounded-lg",
              isSelected
                ? `bg-${category.color}-100 dark:bg-${category.color}-900/50 text-${category.color}-600 dark:text-${category.color}-400`
                : "bg-muted text-muted-foreground"
            )}>
              <Icon className="h-5 w-5" />
            </div>
            <span className={cn(
              "text-sm font-medium",
              isSelected ? "text-foreground" : "text-muted-foreground"
            )}>
              {category.label}
            </span>
          </button>
        )
      })}
    </div>
  )
}

export default CategorySelector
```

- [ ] **Step 2: Commit the category selector**

```bash
git add dashboard/src/components/policies/condition-category-selector.tsx
git commit -m "feat(policy): add category selector component for condition builder

- Grid-based category selector with 8 categories
- Visual feedback for selected category
- Icons and labels for discoverability"
```

---

## Task 3: Build Smart Field Input Component

**Files:**
- Create: `dashboard/src/components/policies/condition-field-input.tsx`
- Test: Manual UI testing

- [ ] **Step 1: Create the field input component**

Create `dashboard/src/components/policies/condition-field-input.tsx`:

```typescript
"use client"

import { useState } from "react"
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
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip"
import { Info, Lightbulb } from "lucide-react"
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
  const currentOpInfo = OPERATOR_INFO[value.op]

  const handleFieldChange = (fieldPath: string) => {
    const newFieldDef = getFieldByPath(fieldPath)
    const newOp = newFieldDef?.operators[0] || value.op
    onChange({ ...value, field: fieldPath, op: newOp, value: "" })
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
```

- [ ] **Step 2: Commit the field input component**

```bash
git add dashboard/src/components/policies/condition-field-input.tsx
git commit -m "feat(policy): add smart field input component

- Type-aware value input (number, string, enum)
- Examples and validation hints
- Support for custom JSON path input"
```

---

## Task 4: Redesign Condition Builder with Hierarchical Approach

**Files:**
- Modify: `dashboard/src/components/policies/condition-builder.tsx`
- Test: Manual UI testing in policy form

- [ ] **Step 1: Update condition-builder.tsx to use hierarchical approach**

Replace the existing `ConditionRow` component with the new category-first approach:

```typescript
// Key changes to condition-builder.tsx:

// 1. Add category selection state
const [selectedCategory, setSelectedCategory] = useState<string | null>(null)

// 2. In ConditionRow, show category selector first, then field input
// 3. Preserve the AND/OR grouping logic (no changes to ConditionGroup)
// 4. Add visual hierarchy with category badges
```

The main changes:
1. Replace the flat field dropdown with category selector → field selector flow
2. Add "Custom" category for advanced users to enter raw JSON paths
3. Show contextual help (examples, validation hints) for each field
4. Maintain backward compatibility with existing condition structures

- [ ] **Step 2: Test the new condition builder**

1. Create a new policy
2. Select "IP / Network" category → verify context.ip appears
3. Select "Custom" category → verify free-form path input works
4. Test AND/OR grouping still works
5. Test condition evaluation in the backend

- [ ] **Step 3: Commit the redesigned condition builder**

```bash
git add dashboard/src/components/policies/condition-builder.tsx
git commit -m "feat(policy): redesign condition builder with hierarchical categories

- Category-first selection (IP, Model, Tools, Time, etc.)
- Smart field input with examples and validation
- Custom path escape hatch for advanced users
- Maintains backward compatibility with existing conditions"
```

---

## Task 5: Add Token-Level Quick Restrictions

**Files:**
- Modify: `dashboard/src/app/(dashboard)/tokens/[id]/page.tsx`
- Modify: `dashboard/src/lib/types/token.ts`
- Test: Token creation/editing flow

**Note:** This is a separate feature from the condition builder. It adds quick IP and tool restrictions directly on tokens.

- [ ] **Step 1: Analyze existing token restrictions**

Current token restrictions:
- `mcp_allowed_tools` - array of allowed tool names
- `mcp_blocked_tools` - array of blocked tool names
- No IP restrictions at token level

- [ ] **Step 2: Add IP restrictions to token types**

Update `dashboard/src/lib/types/token.ts`:

```typescript
// Add to TokenRow interface
allowed_ips?: string[] | null       // CIDR notation: ["192.168.0.0/16", "10.0.0.1"]
blocked_ips?: string[] | null       // Block specific IPs

// Add to CreateTokenRequest
allowed_ips?: string[]
blocked_ips?: string[]
```

- [ ] **Step 3: Add UI for IP restrictions in token page**

In `dashboard/src/app/(dashboard)/tokens/[id]/page.tsx`:
- Add "IP Restrictions" section
- Input for allowed IPs (CIDR notation)
- Input for blocked IPs
- Helper text: "Enter IP addresses or CIDR ranges (e.g., 192.168.0.0/16)"

- [ ] **Step 4: Test token IP restrictions**

1. Create a token with IP restrictions
2. Make a request from allowed IP
3. Make a request from blocked IP
4. Verify policy engine enforces the restrictions

- [ ] **Step 5: Commit token IP restrictions**

```bash
git add dashboard/src/lib/types/token.ts dashboard/src/app/\(dashboard\)/tokens/\[id\]/page.tsx
git commit -m "feat(tokens): add IP restrictions to token configuration

- Add allowed_ips and blocked_ips fields
- UI for configuring IP restrictions at token level
- Supports CIDR notation for network ranges"
```

---

## Task 6: Add Field Autocomplete for Custom Paths

**Files:**
- Modify: `dashboard/src/components/policies/custom-field-input.tsx`
- Test: Autocomplete functionality

- [ ] **Step 1: Create autocomplete suggestions for custom paths**

When user selects "Custom" category and starts typing:
- Show suggestions from known field paths
- Include common patterns like `request.body.messages[*].content`

- [ ] **Step 2: Commit autocomplete**

```bash
git add dashboard/src/components/policies/custom-field-input.tsx
git commit -m "feat(policy): add autocomplete for custom field paths

- Suggest known field paths as user types
- Include common patterns for array access"
```

---

## Self-Review Checklist

After completing all tasks, verify:

- [ ] **Spec coverage**: All categories (IP, Model, Tools, Time, Request, Response, Token, Agent, Custom) implemented
- [ ] **No placeholders**: All code blocks contain actual implementation code
- [ ] **Type consistency**: FieldDefinition types match across registry and components
- [ ] **Backend compatibility**: Generated conditions work with existing backend evaluator
- [ ] **Accessibility**: All selectors have proper labels and keyboard navigation

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-03-28-policy-engine-ui-redesign.md`. Two execution options:**

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**