"use client"

import { useState, useMemo } from "react"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Badge } from "@/components/ui/badge"
import { cn } from "@/lib/utils"
import { FIELD_CATEGORIES, getFieldByPath } from "@/lib/field-registry"

// Common patterns for autocomplete
const COMMON_PATTERNS = [
  "request.body.messages[*].content",
  "request.body.messages[*].role",
  "request.body.tools[*].function.name",
  "request.body.tools[*].function.arguments",
  "request.headers.authorization",
  "request.headers.content-type",
  "response.body.choices[*].message.content",
  "response.body.usage.total_tokens",
]

interface CustomFieldInputProps {
  value: string
  onChange: (value: string) => void
}

export function CustomFieldInput({ value, onChange }: CustomFieldInputProps) {
  const [showSuggestions, setShowSuggestions] = useState(false)

  // Collect all known field paths
  const allKnownPaths = useMemo(() => {
    const paths = new Set<string>()
    FIELD_CATEGORIES.forEach(cat => {
      cat.fields.forEach(field => paths.add(field.path))
    })
    COMMON_PATTERNS.forEach(p => paths.add(p))
    return Array.from(paths).sort()
  }, [])

  // Filter suggestions based on input
  const suggestions = useMemo(() => {
    if (!value || value.length < 1) return allKnownPaths.slice(0, 10)
    return allKnownPaths.filter(p =>
      p.toLowerCase().includes(value.toLowerCase())
    ).slice(0, 10)
  }, [value, allKnownPaths])

  const handleSelect = (path: string) => {
    onChange(path)
    setShowSuggestions(false)
  }

  return (
    <div className="relative">
      <Label className="text-xs text-muted-foreground mb-1.5 block">Custom Field Path</Label>
      <Input
        value={value}
        onChange={(e) => {
          onChange(e.target.value)
          setShowSuggestions(true)
        }}
        onFocus={() => setShowSuggestions(true)}
        onBlur={() => setTimeout(() => setShowSuggestions(false), 200)}
        placeholder="request.body.custom.field"
        className="font-mono text-sm"
      />

      {showSuggestions && suggestions.length > 0 && (
        <div className="absolute z-50 w-full mt-1 bg-popover border rounded-lg shadow-lg max-h-60 overflow-auto">
          {suggestions.map((path) => {
            const fieldDef = getFieldByPath(path)
            return (
              <button
                key={path}
                type="button"
                className={cn(
                  "w-full px-3 py-2 text-left text-sm hover:bg-muted flex items-center justify-between",
                  path === value && "bg-muted"
                )}
                onClick={() => handleSelect(path)}
              >
                <span className="font-mono">{path}</span>
                {fieldDef && (
                  <Badge variant="secondary" className="text-[10px]">
                    {fieldDef.label}
                  </Badge>
                )}
              </button>
            )
          })}
        </div>
      )}

      <p className="text-xs text-muted-foreground mt-1">
        Enter any JSON path or select from suggestions. Use [*] for array wildcards.
      </p>
    </div>
  )
}

export default CustomFieldInput