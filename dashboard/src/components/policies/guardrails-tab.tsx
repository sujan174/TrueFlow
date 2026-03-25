"use client"

import { useState, useCallback } from "react"
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
import { Shield, AlertTriangle, Code, MessageSquareWarning, Scale, Target, Puzzle, Mail, Building, Lock, Plus, Trash2, Power } from "lucide-react"
import type { ActionContentFilter, Rule, Condition } from "@/lib/types/policy"
import { GUARDRAIL_CATEGORIES } from "@/lib/types/policy"

// ============================================================================
// Types
// ============================================================================

interface GuardrailsTabProps {
  value: ActionContentFilter | null
  onChange: (action: ActionContentFilter | null) => void
}

interface GuardrailPreset {
  id: string
  name: string
  description: string
  config: Partial<ActionContentFilter>
}

// ============================================================================
// Icons mapping
// ============================================================================

const CATEGORY_ICONS: Record<string, React.ReactNode> = {
  jailbreak: <AlertTriangle className="h-4 w-4" />,
  harmful: <AlertTriangle className="h-4 w-4" />,
  code_injection: <Code className="h-4 w-4" />,
  profanity: <MessageSquareWarning className="h-4 w-4" />,
  bias: <Scale className="h-4 w-4" />,
  sensitive_topics: <Target className="h-4 w-4" />,
  gibberish: <Puzzle className="h-4 w-4" />,
  contact_info: <Mail className="h-4 w-4" />,
  competitor: <Building className="h-4 w-4" />,
  ip_leakage: <Lock className="h-4 w-4" />,
}

// ============================================================================
// Presets
// ============================================================================

const GUARDRAIL_PRESETS: GuardrailPreset[] = [
  {
    id: 'strict',
    name: 'Strict Safety',
    description: 'Blocks all harmful content categories',
    config: {
      block_jailbreak: true,
      block_harmful: true,
      block_code_injection: true,
      block_profanity: true,
      block_bias: true,
      block_competitor_mention: true,
      block_sensitive_topics: true,
      block_gibberish: true,
      block_contact_info: true,
      block_ip_leakage: true,
      risk_threshold: 0.5,
    },
  },
  {
    id: 'balanced',
    name: 'Balanced',
    description: 'Common protections for most use cases',
    config: {
      block_jailbreak: true,
      block_harmful: true,
      block_code_injection: true,
      block_profanity: false,
      block_bias: false,
      block_competitor_mention: false,
      block_sensitive_topics: false,
      block_gibberish: false,
      block_contact_info: false,
      block_ip_leakage: true,
      risk_threshold: 0.6,
    },
  },
  {
    id: 'minimal',
    name: 'Minimal',
    description: 'Jailbreak protection only',
    config: {
      block_jailbreak: true,
      block_harmful: true,
      block_code_injection: false,
      block_profanity: false,
      block_bias: false,
      block_competitor_mention: false,
      block_sensitive_topics: false,
      block_gibberish: false,
      block_contact_info: false,
      block_ip_leakage: false,
      risk_threshold: 0.7,
    },
  },
  {
    id: 'enterprise',
    name: 'Enterprise',
    description: 'PII + compliance + security',
    config: {
      block_jailbreak: true,
      block_harmful: true,
      block_code_injection: true,
      block_profanity: false,
      block_bias: false,
      block_competitor_mention: true,
      block_sensitive_topics: true,
      block_gibberish: false,
      block_contact_info: true,
      block_ip_leakage: true,
      risk_threshold: 0.5,
    },
  },
]

// ============================================================================
// Main Component
// ============================================================================

export function GuardrailsTab({ value, onChange }: GuardrailsTabProps) {
  const isEnabled = value !== null

  const [topicAllowlist, setTopicAllowlist] = useState(value?.topic_allowlist?.join('\n') || '')
  const [topicDenylist, setTopicDenylist] = useState(value?.topic_denylist?.join('\n') || '')
  const [customPatterns, setCustomPatterns] = useState<string[]>(value?.custom_patterns || [])
  const [newPattern, setNewPattern] = useState('')

  // Default config if none provided
  const config: ActionContentFilter = value || {
    action: 'content_filter',
    block_jailbreak: false,
    block_harmful: false,
    block_code_injection: false,
    block_profanity: false,
    block_bias: false,
    block_competitor_mention: false,
    block_sensitive_topics: false,
    block_gibberish: false,
    block_contact_info: false,
    block_ip_leakage: false,
    risk_threshold: 0.5,
  }

  const toggleEnabled = (enabled: boolean) => {
    if (enabled) {
      // Create default action
      onChange({
        action: 'content_filter',
        block_jailbreak: true,
        block_harmful: true,
        block_code_injection: true,
        risk_threshold: 0.5,
      })
    } else {
      onChange(null)
    }
  }

  const updateConfig = useCallback((updates: Partial<ActionContentFilter>) => {
    onChange({ ...config, ...updates })
  }, [config, onChange])

  const applyPreset = (presetId: string | null) => {
    if (!presetId) return
    const preset = GUARDRAIL_PRESETS.find(p => p.id === presetId)
    if (preset) {
      onChange({
        action: 'content_filter',
        ...preset.config,
      } as ActionContentFilter)
    }
  }

  const toggleCategory = (categoryId: string, enabled: boolean) => {
    const key = `block_${categoryId}` as keyof ActionContentFilter
    updateConfig({ [key]: enabled })
  }

  const handleTopicAllowlistChange = (text: string) => {
    setTopicAllowlist(text)
    const topics = text.split('\n').map(t => t.trim()).filter(Boolean)
    updateConfig({ topic_allowlist: topics.length > 0 ? topics : undefined })
  }

  const handleTopicDenylistChange = (text: string) => {
    setTopicDenylist(text)
    const topics = text.split('\n').map(t => t.trim()).filter(Boolean)
    updateConfig({ topic_denylist: topics.length > 0 ? topics : undefined })
  }

  const addCustomPattern = () => {
    if (newPattern.trim()) {
      const patterns = [...customPatterns, newPattern.trim()]
      setCustomPatterns(patterns)
      setNewPattern('')
      updateConfig({ custom_patterns: patterns })
    }
  }

  const removeCustomPattern = (index: number) => {
    const patterns = customPatterns.filter((_, i) => i !== index)
    setCustomPatterns(patterns)
    updateConfig({ custom_patterns: patterns.length > 0 ? patterns : undefined })
  }

  const isCategoryEnabled = (categoryId: string): boolean => {
    const key = `block_${categoryId}` as keyof ActionContentFilter
    return Boolean(config[key])
  }

  return (
    <div className="space-y-6">
      {/* Enable Toggle */}
      <div className="flex items-center justify-between p-4 bg-card border rounded-xl">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-muted">
            <Shield className="h-5 w-5" />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium">Content Guardrails</span>
              {isEnabled && <Badge variant="default">Enabled</Badge>}
            </div>
            <p className="text-xs text-muted-foreground">
              Filter harmful content, jailbreaks, and more
            </p>
          </div>
        </div>
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger>
              <Button
                type="button"
                variant={isEnabled ? "default" : "outline"}
                size="sm"
                onClick={() => toggleEnabled(!isEnabled)}
              >
                <Power className="h-4 w-4 mr-1" />
                {isEnabled ? 'Disable' : 'Enable'}
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {isEnabled ? 'Disable content guardrails' : 'Enable content guardrails'}
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>
      </div>

      {/* Configuration (only shown when enabled) */}
      {isEnabled && (
        <>
          {/* Preset Selector */}
      <div className="flex items-center gap-3">
        <Label className="text-sm font-medium">Quick Preset</Label>
        <Select onValueChange={applyPreset}>
          <SelectTrigger className="w-[280px]">
            <SelectValue placeholder="-- Select a preset --" />
          </SelectTrigger>
          <SelectContent>
            {GUARDRAIL_PRESETS.map((preset) => (
              <SelectItem key={preset.id} value={preset.id}>
                <div className="flex items-center gap-2">
                  <Shield className="h-4 w-4 text-muted-foreground" />
                  <span>{preset.name}</span>
                  <span className="text-muted-foreground text-xs">- {preset.description}</span>
                </div>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {/* Category Toggles */}
      <div>
        <h4 className="text-sm font-medium mb-3">Built-in Categories</h4>
        <div className="grid grid-cols-2 gap-3">
          {GUARDRAIL_CATEGORIES.map((category) => {
            const enabled = isCategoryEnabled(category.id)
            return (
              <div
                key={category.id}
                className={`flex items-center justify-between p-4 rounded-xl border transition-colors ${
                  enabled
                    ? 'bg-primary/5 border-primary/30 ring-1 ring-primary/20'
                    : 'bg-card hover:bg-muted/50'
                }`}
              >
                <div className="flex items-center gap-3">
                  <div className={`p-2 rounded-lg ${enabled ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground'}`}>
                    {CATEGORY_ICONS[category.id]}
                  </div>
                  <div>
                    <span className={`text-sm font-medium ${enabled ? 'text-foreground' : 'text-muted-foreground'}`}>
                      {category.label}
                    </span>
                    <p className="text-xs text-muted-foreground">{category.description}</p>
                  </div>
                </div>
                <TooltipProvider>
                  <Tooltip>
                    <TooltipTrigger>
                      <div>
                        <Checkbox
                          checked={enabled}
                          onCheckedChange={(checked) => toggleCategory(category.id, checked === true)}
                        />
                      </div>
                    </TooltipTrigger>
                    <TooltipContent>
                      {enabled ? 'Click to disable' : 'Click to enable'}
                    </TooltipContent>
                  </Tooltip>
                </TooltipProvider>
              </div>
            )
          })}
        </div>
      </div>

      {/* Topic Lists */}
      <div className="grid grid-cols-2 gap-4">
        <div>
          <Label className="text-sm font-medium">Topic Allowlist</Label>
          <p className="text-xs text-muted-foreground mb-2">Allowed topics (one per line)</p>
          <textarea
            className="w-full h-24 p-3 text-sm border rounded-xl bg-background resize-none focus:outline-none focus:ring-2 focus:ring-ring"
            value={topicAllowlist}
            onChange={(e) => handleTopicAllowlistChange(e.target.value)}
            placeholder="cooking&#10;recipes&#10;food"
          />
        </div>
        <div>
          <Label className="text-sm font-medium">Topic Denylist</Label>
          <p className="text-xs text-muted-foreground mb-2">Blocked topics (one per line)</p>
          <textarea
            className="w-full h-24 p-3 text-sm border rounded-xl bg-background resize-none focus:outline-none focus:ring-2 focus:ring-ring"
            value={topicDenylist}
            onChange={(e) => handleTopicDenylistChange(e.target.value)}
            placeholder="weapons&#10;drugs"
          />
        </div>
      </div>

      {/* Custom Patterns */}
      <div className="p-4 bg-card border rounded-xl">
        <div className="flex items-center gap-2 mb-3">
          <Plus className="h-4 w-4 text-muted-foreground" />
          <h4 className="text-sm font-medium">Custom Patterns (Regex)</h4>
        </div>
        <div className="flex gap-2 mb-3">
          <Input
            placeholder="(?i)competitor_name"
            value={newPattern}
            onChange={(e) => setNewPattern(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && addCustomPattern()}
            className="font-mono text-sm"
          />
          <Button variant="outline" onClick={addCustomPattern}>
            <Plus className="h-4 w-4" />
          </Button>
        </div>
        {customPatterns.length > 0 && (
          <div className="flex flex-wrap gap-1.5">
            {customPatterns.map((pattern, index) => (
              <Badge key={index} variant="secondary" className="font-mono text-xs gap-1">
                <code>{pattern}</code>
                <button
                  className="ml-1 text-muted-foreground hover:text-foreground"
                  onClick={() => removeCustomPattern(index)}
                >
                  <Trash2 className="h-3 w-3" />
                </button>
              </Badge>
            ))}
          </div>
        )}
        <p className="text-xs text-muted-foreground mt-3">
          Add custom regex patterns to detect specific content
        </p>
      </div>

      {/* Risk Threshold */}
      <div className="flex items-center gap-4">
        <Label className="text-sm font-medium">Risk Threshold</Label>
        <Input
          type="number"
          step="0.1"
          min="0"
          max="1"
          value={config.risk_threshold || 0.5}
          onChange={(e) => updateConfig({ risk_threshold: parseFloat(e.target.value) || 0.5 })}
          className="w-24"
        />
        <span className="text-xs text-muted-foreground">
          (0.0 = least strict, 1.0 = most strict)
        </span>
      </div>
        </>
      )}
    </div>
  )
}

export default GuardrailsTab