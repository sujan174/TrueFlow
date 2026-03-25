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
import { Search, Brain, ArrowRightLeft, AlertCircle, Key, Settings, User, MapPin, Phone, Building2, Activity, Lock, Power } from "lucide-react"
import type { ActionRedact, Condition, RedactDirection, RedactOnMatch } from "@/lib/types/policy"
import { PII_REGEX_PATTERNS, PII_NLP_ENTITIES } from "@/lib/types/policy"

// ============================================================================
// Types
// ============================================================================

interface PIITabProps {
  value: ActionRedact | null
  onChange: (action: ActionRedact | null) => void
}

interface PresidioConfig {
  endpoint: string
  language: string
  score_threshold: number
  enabled: boolean
}

// ============================================================================
// Main Component
// ============================================================================

export function PIITab({ value, onChange }: PIITabProps) {
  const isEnabled = value !== null

  // Direction state
  const [direction, setDirection] = useState<RedactDirection>(
    value?.direction || 'both'
  )

  // Regex patterns state
  const [selectedPatterns, setSelectedPatterns] = useState<string[]>(
    value?.patterns || []
  )

  // NLP entities state
  const [selectedEntities, setSelectedEntities] = useState<string[]>([])

  // On match action
  const [onMatch, setOnMatch] = useState<RedactOnMatch | 'tokenize'>(
    value?.on_match || 'redact'
  )

  // Presidio config
  const [presidioConfig, setPresidioConfig] = useState<PresidioConfig>({
    endpoint: 'http://presidio:5002',
    language: 'en',
    score_threshold: 0.7,
    enabled: selectedEntities.length > 0,
  })

  // Default config
  const config: ActionRedact = value || {
    action: 'redact',
    direction: 'both',
    patterns: [],
    on_match: 'redact',
  }

  const toggleEnabled = (enabled: boolean) => {
    if (enabled) {
      // Create default action
      onChange({
        action: 'redact',
        direction: 'both',
        patterns: ['ssn', 'credit_card', 'email'],
        on_match: 'redact',
      })
    } else {
      onChange(null)
    }
  }

  const updateAction = useCallback((updates: Partial<ActionRedact>) => {
    onChange({ ...config, ...updates })
  }, [config, onChange])

  const handleDirectionChange = (newDirection: RedactDirection) => {
    setDirection(newDirection)
    updateAction({ direction: newDirection })
  }

  const togglePattern = (patternId: string) => {
    const newPatterns = selectedPatterns.includes(patternId)
      ? selectedPatterns.filter(p => p !== patternId)
      : [...selectedPatterns, patternId]
    setSelectedPatterns(newPatterns)
    updateAction({ patterns: newPatterns.length > 0 ? newPatterns : undefined })
  }

  const toggleEntity = (entityId: string) => {
    const newEntities = selectedEntities.includes(entityId)
      ? selectedEntities.filter(e => e !== entityId)
      : [...selectedEntities, entityId]
    setSelectedEntities(newEntities)
  }

  const handlePresidioConfigChange = (field: keyof PresidioConfig, val: string | number | boolean) => {
    setPresidioConfig({ ...presidioConfig, [field]: val })
  }

  // Direction icons
  const DirectionIcon = ArrowRightLeft

  return (
    <div className="space-y-6">
      {/* Enable Toggle */}
      <div className="flex items-center justify-between p-4 bg-card border rounded-xl">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-muted">
            <Lock className="h-5 w-5" />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium">PII Redaction</span>
              {isEnabled && <Badge variant="default">Enabled</Badge>}
            </div>
            <p className="text-xs text-muted-foreground">
              Detect and redact personally identifiable information
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
              {isEnabled ? 'Disable PII redaction' : 'Enable PII redaction'}
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>
      </div>

      {/* Configuration (only shown when enabled) */}
      {isEnabled && (
        <>
          {/* Direction Selection */}
      <div>
        <Label className="text-sm font-medium mb-3 block">Apply To</Label>
        <div className="flex gap-2">
          {(['request', 'response', 'both'] as RedactDirection[]).map((dir) => {
            const isSelected = direction === dir
            const labels = {
              request: 'Request Only',
              response: 'Response Only',
              both: 'Both Directions',
            }
            return (
              <Button
                key={dir}
                type="button"
                variant={isSelected ? 'default' : 'outline'}
                onClick={() => handleDirectionChange(dir)}
              >
                {labels[dir]}
              </Button>
            )
          })}
        </div>
        <p className="text-xs text-muted-foreground mt-2">
          Choose which request direction(s) to apply PII detection to.
        </p>
      </div>

      {/* Detection Methods */}
      <div>
        <Label className="text-sm font-medium mb-3 block">Detection Method</Label>
        <div className="grid grid-cols-2 gap-4">
          {/* Regex Pattern Matching */}
          <div className="p-4 bg-card border rounded-xl">
            <div className="flex items-center gap-2 mb-3">
              <div className="p-2 rounded-lg bg-muted">
                <Search className="h-4 w-4 text-muted-foreground" />
              </div>
              <div>
                <h4 className="font-medium text-sm">Pattern Matching</h4>
                <p className="text-xs text-muted-foreground">Fast regex-based detection</p>
              </div>
            </div>
            <div className="space-y-2">
              {PII_REGEX_PATTERNS.map((pattern) => {
                const isSelected = selectedPatterns.includes(pattern.id)
                return (
                  <TooltipProvider key={pattern.id}>
                    <Tooltip>
                      <TooltipTrigger>
                        <label className="flex items-center gap-2 text-sm cursor-pointer p-2 rounded-lg hover:bg-muted/50 transition-colors">
                          <Checkbox
                            checked={isSelected}
                            onCheckedChange={() => togglePattern(pattern.id)}
                          />
                          <span className={isSelected ? 'text-foreground' : 'text-muted-foreground'}>
                            {pattern.label}
                          </span>
                        </label>
                      </TooltipTrigger>
                      <TooltipContent>
                        <p>{pattern.description}</p>
                      </TooltipContent>
                    </Tooltip>
                  </TooltipProvider>
                )
              })}
            </div>
          </div>

          {/* NLP Detection */}
          <div className="p-4 bg-card border rounded-xl">
            <div className="flex items-center gap-2 mb-3">
              <div className="p-2 rounded-lg bg-muted">
                <Brain className="h-4 w-4 text-muted-foreground" />
              </div>
              <div>
                <h4 className="font-medium text-sm">NLP Detection (Presidio)</h4>
                <p className="text-xs text-muted-foreground">AI-powered for unstructured text</p>
              </div>
            </div>
            <div className="space-y-2">
              {PII_NLP_ENTITIES.map((entity) => {
                const isSelected = selectedEntities.includes(entity.id)
                return (
                  <TooltipProvider key={entity.id}>
                    <Tooltip>
                      <TooltipTrigger>
                        <label className="flex items-center gap-2 text-sm cursor-pointer p-2 rounded-lg hover:bg-muted/50 transition-colors">
                          <Checkbox
                            checked={isSelected}
                            onCheckedChange={() => toggleEntity(entity.id)}
                          />
                          <span className={isSelected ? 'text-foreground' : 'text-muted-foreground'}>
                            {entity.label}
                          </span>
                        </label>
                      </TooltipTrigger>
                      <TooltipContent>
                        <p>{entity.description}</p>
                      </TooltipContent>
                    </Tooltip>
                  </TooltipProvider>
                )
              })}
            </div>
          </div>
        </div>
      </div>

      {/* On Match Action */}
      <div>
        <Label className="text-sm font-medium mb-3 block">When PII Detected</Label>
        <div className="flex gap-2">
          {(['redact', 'block', 'tokenize'] as const).map((action) => {
            const isSelected = onMatch === action
            const labels = {
              redact: 'Redact [REDACTED_SSN]',
              block: 'Block Request',
              tokenize: 'Tokenize (reversible)',
            }
            const descriptions = {
              redact: 'Replace PII with placeholder text',
              block: 'Reject the entire request',
              tokenize: 'Replace with reversible token for later restoration',
            }
            return (
              <TooltipProvider key={action}>
                <Tooltip>
                  <TooltipTrigger>
                    <Button
                      type="button"
                      variant={isSelected ? 'default' : 'outline'}
                      onClick={() => {
                        setOnMatch(action as RedactOnMatch | 'tokenize')
                        updateAction({ on_match: action === 'tokenize' ? 'redact' : action as RedactOnMatch })
                      }}
                    >
                      {labels[action]}
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>
                    <p>{descriptions[action]}</p>
                  </TooltipContent>
                </Tooltip>
              </TooltipProvider>
            )
          })}
        </div>
      </div>

      {/* Presidio Settings */}
      {selectedEntities.length > 0 && (
        <div className="p-4 bg-card border rounded-xl">
          <div className="flex items-center gap-2 mb-4">
            <Settings className="h-4 w-4 text-muted-foreground" />
            <h4 className="text-sm font-medium">Presidio Settings</h4>
          </div>
          <div className="grid grid-cols-3 gap-4">
            <div>
              <Label className="text-xs text-muted-foreground">Endpoint</Label>
              <Input
                value={presidioConfig.endpoint}
                onChange={(e) => handlePresidioConfigChange('endpoint', e.target.value)}
                placeholder="http://presidio:5002"
                className="mt-1"
              />
            </div>
            <div>
              <Label className="text-xs text-muted-foreground">Language</Label>
              <Select
                value={presidioConfig.language}
                onValueChange={(v) => v && handlePresidioConfigChange('language', v)}
              >
                <SelectTrigger className="mt-1">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="en">English (en)</SelectItem>
                  <SelectItem value="es">Spanish (es)</SelectItem>
                  <SelectItem value="de">German (de)</SelectItem>
                  <SelectItem value="fr">French (fr)</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div>
              <Label className="text-xs text-muted-foreground">Score Threshold</Label>
              <Input
                type="number"
                step="0.1"
                min="0"
                max="1"
                value={presidioConfig.score_threshold}
                onChange={(e) => handlePresidioConfigChange('score_threshold', parseFloat(e.target.value) || 0.7)}
                className="mt-1"
              />
            </div>
          </div>
          <p className="text-xs text-muted-foreground mt-3">
            Presidio is an open-source PII detection service. Configure the endpoint and detection sensitivity.
          </p>
        </div>
      )}

      {/* Fields to Redact (Optional) */}
      <div>
        <Label className="text-sm font-medium">Specific Fields to Redact</Label>
        <p className="text-xs text-muted-foreground mb-2">
          Limit redaction to specific JSON paths in request/response body
        </p>
        <Input
          placeholder="messages.*.content, prompt"
          className="font-mono text-sm"
          onChange={(e) => {
            const fields = e.target.value.split(',').map(f => f.trim()).filter(Boolean)
            updateAction({ fields: fields.length > 0 ? fields : undefined })
          }}
        />
      </div>
        </>
      )}
    </div>
  )
}

export default PIITab