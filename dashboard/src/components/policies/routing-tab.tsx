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
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip"
import { DollarSign, Zap, RefreshCw, BarChart3, Dice5, Plus, Trash2, Zap as Bolt } from "lucide-react"
import type { ActionDynamicRoute, Condition } from "@/lib/types/policy"
import { ROUTING_STRATEGIES } from "@/lib/types/policy"

// ============================================================================
// Types
// ============================================================================

interface RoutingTabProps {
  value: ActionDynamicRoute | null
  onChange: (action: ActionDynamicRoute | null) => void
}

interface ModelPoolEntry {
  model: string
  upstream_url: string
  credential_id?: string
  weight: number
}

interface CircuitBreakerConfig {
  enabled: boolean
  failure_threshold: number
  failure_rate_threshold: number
  min_sample_size: number
  recovery_cooldown_secs: number
  half_open_max_requests: number
}

// ============================================================================
// Icons
// ============================================================================

const STRATEGY_ICONS: Record<string, React.ReactNode> = {
  lowest_cost: <DollarSign className="h-5 w-5" />,
  lowest_latency: <Zap className="h-5 w-5" />,
  round_robin: <RefreshCw className="h-5 w-5" />,
  least_busy: <BarChart3 className="h-5 w-5" />,
  weighted_random: <Dice5 className="h-5 w-5" />,
}

// ============================================================================
// Default Values
// ============================================================================

const DEFAULT_CIRCUIT_BREAKER: CircuitBreakerConfig = {
  enabled: true,
  failure_threshold: 3,
  failure_rate_threshold: 30,
  min_sample_size: 20,
  recovery_cooldown_secs: 60,
  half_open_max_requests: 1,
}

// ============================================================================
// Main Component
// ============================================================================

export function RoutingTab({ value, onChange }: RoutingTabProps) {
  const [strategy, setStrategy] = useState<ActionDynamicRoute['strategy']>(
    value?.strategy || 'lowest_cost'
  )
  const [modelPool, setModelPool] = useState<ModelPoolEntry[]>(
    value?.pool?.map(p => ({
      model: p.model,
      upstream_url: p.upstream_url,
      weight: 100,
    })) || []
  )
  const [fallback, setFallback] = useState<string>(value?.fallback?.model || '')
  const [fallbackUrl, setFallbackUrl] = useState<string>(value?.fallback?.upstream_url || '')
  const [circuitBreaker, setCircuitBreaker] = useState<CircuitBreakerConfig>(DEFAULT_CIRCUIT_BREAKER)

  const updateAction = useCallback((updates: Partial<ActionDynamicRoute>) => {
    if (value) {
      onChange({ ...value, ...updates })
    } else {
      onChange({
        action: 'dynamic_route',
        strategy,
        pool: modelPool.map(p => ({
          model: p.model,
          upstream_url: p.upstream_url,
        })),
        ...updates,
      })
    }
  }, [value, strategy, modelPool, onChange])

  const handleStrategyChange = (newStrategy: string) => {
    const strat = newStrategy as ActionDynamicRoute['strategy']
    setStrategy(strat)
    updateAction({ strategy: strat })
  }

  const addModel = () => {
    const newEntry: ModelPoolEntry = {
      model: '',
      upstream_url: '',
      weight: 100,
    }
    setModelPool([...modelPool, newEntry])
  }

  const removeModel = (index: number) => {
    const newPool = modelPool.filter((_, i) => i !== index)
    setModelPool(newPool)
    updateAction({
      pool: newPool.map(p => ({
        model: p.model,
        upstream_url: p.upstream_url,
      })),
    })
  }

  const updateModel = (index: number, field: keyof ModelPoolEntry, val: string | number) => {
    const newPool = [...modelPool]
    newPool[index] = { ...newPool[index], [field]: val }
    setModelPool(newPool)
    updateAction({
      pool: newPool.map(p => ({
        model: p.model,
        upstream_url: p.upstream_url,
      })),
    })
  }

  const handleCircuitBreakerChange = (field: keyof CircuitBreakerConfig, val: boolean | number) => {
    const newConfig = { ...circuitBreaker, [field]: val }
    setCircuitBreaker(newConfig)
  }

  return (
    <div className="space-y-6">
      {/* Strategy Selection */}
      <div>
        <Label className="text-sm font-medium mb-3 block">Routing Strategy</Label>
        <div className="grid grid-cols-5 gap-2">
          {ROUTING_STRATEGIES.map((s) => {
            const isSelected = strategy === s.id
            return (
              <button
                key={s.id}
                type="button"
                onClick={() => handleStrategyChange(s.id)}
                className={`flex flex-col items-center gap-2 p-4 rounded-xl border text-center transition-all ${
                  isSelected
                    ? 'border-primary bg-primary/5 ring-2 ring-primary/20'
                    : 'border-border bg-card hover:bg-muted/50'
                }`}
              >
                <div className={`p-2 rounded-lg ${isSelected ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground'}`}>
                  {STRATEGY_ICONS[s.id]}
                </div>
                <span className={`text-sm font-medium ${isSelected ? 'text-foreground' : 'text-muted-foreground'}`}>
                  {s.label}
                </span>
                <p className="text-xs text-muted-foreground line-clamp-1">{s.description}</p>
              </button>
            )
          })}
        </div>
      </div>

      {/* Model Pool */}
      <div>
        <div className="flex items-center justify-between mb-3">
          <Label className="text-sm font-medium">Model Pool</Label>
          <Button variant="outline" size="sm" onClick={addModel}>
            <Plus className="h-4 w-4 mr-1" />
            Add Model
          </Button>
        </div>
        {modelPool.length === 0 ? (
          <div className="p-8 border-2 border-dashed rounded-xl text-center text-muted-foreground">
            <p>No models in pool. Add models to route to.</p>
            <Button variant="outline" size="sm" className="mt-3" onClick={addModel}>
              <Plus className="h-4 w-4 mr-1" />
              Add First Model
            </Button>
          </div>
        ) : (
          <div className="bg-card border rounded-xl overflow-hidden">
            <table className="w-full text-sm">
              <thead className="bg-muted/50 border-b">
                <tr className="text-xs font-semibold tracking-wider text-muted-foreground uppercase">
                  <th className="px-4 py-3 text-left">Model</th>
                  <th className="px-4 py-3 text-left">Upstream URL</th>
                  {strategy === 'weighted_random' && (
                    <th className="px-4 py-3 text-center w-24">Weight</th>
                  )}
                  <th className="px-4 py-3 text-center w-16"></th>
                </tr>
              </thead>
              <tbody>
                {modelPool.map((entry, index) => (
                  <tr key={index} className="border-b last:border-0">
                    <td className="p-2">
                      <Input
                        value={entry.model}
                        onChange={(e) => updateModel(index, 'model', e.target.value)}
                        placeholder="gpt-4o"
                        className="border-0 bg-transparent h-8"
                      />
                    </td>
                    <td className="p-2">
                      <Input
                        value={entry.upstream_url}
                        onChange={(e) => updateModel(index, 'upstream_url', e.target.value)}
                        placeholder="https://api.openai.com/v1"
                        className="border-0 bg-transparent h-8"
                      />
                    </td>
                    {strategy === 'weighted_random' && (
                      <td className="p-2 text-center">
                        <Input
                          type="number"
                          value={entry.weight}
                          onChange={(e) => updateModel(index, 'weight', parseInt(e.target.value) || 0)}
                          className="w-16 text-center border-0 bg-transparent h-8"
                        />
                      </td>
                    )}
                    <td className="p-2 text-center">
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        className="text-muted-foreground hover:text-destructive"
                        onClick={() => removeModel(index)}
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Circuit Breaker */}
      <div className="p-4 bg-card border rounded-xl">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <Bolt className="h-4 w-4 text-muted-foreground" />
            <h4 className="text-sm font-medium">Circuit Breaker</h4>
          </div>
          <label className="flex items-center gap-2 text-sm">
            <Checkbox
              checked={circuitBreaker.enabled}
              onCheckedChange={(checked) => handleCircuitBreakerChange('enabled', checked === true)}
            />
            Enabled
          </label>
        </div>
        <div className="grid grid-cols-3 gap-4">
          <div>
            <Label className="text-xs text-muted-foreground">Failure Threshold</Label>
            <Input
              type="number"
              value={circuitBreaker.failure_threshold}
              onChange={(e) => handleCircuitBreakerChange('failure_threshold', parseInt(e.target.value) || 3)}
              disabled={!circuitBreaker.enabled}
              className="mt-1"
            />
          </div>
          <div>
            <Label className="text-xs text-muted-foreground">Recovery (secs)</Label>
            <Input
              type="number"
              value={circuitBreaker.recovery_cooldown_secs}
              onChange={(e) => handleCircuitBreakerChange('recovery_cooldown_secs', parseInt(e.target.value) || 60)}
              disabled={!circuitBreaker.enabled}
              className="mt-1"
            />
          </div>
          <div>
            <Label className="text-xs text-muted-foreground">Failure Rate %</Label>
            <Input
              type="number"
              value={circuitBreaker.failure_rate_threshold}
              onChange={(e) => handleCircuitBreakerChange('failure_rate_threshold', parseInt(e.target.value) || 30)}
              disabled={!circuitBreaker.enabled}
              className="mt-1"
            />
          </div>
        </div>
        <p className="text-xs text-muted-foreground mt-3">
          Circuit breaker protects against cascading failures by temporarily stopping requests to unhealthy upstreams.
        </p>
      </div>

      {/* Fallback */}
      <div>
        <Label className="text-sm font-medium">Fallback Model</Label>
        <p className="text-xs text-muted-foreground mb-2">Used when all models in pool fail</p>
        <div className="grid grid-cols-2 gap-3">
          <Input
            value={fallback}
            onChange={(e) => {
              setFallback(e.target.value)
              if (e.target.value && fallbackUrl) {
                updateAction({
                  fallback: {
                    model: e.target.value,
                    upstream_url: fallbackUrl,
                  },
                })
              }
            }}
            placeholder="gpt-3.5-turbo"
          />
          <Input
            value={fallbackUrl}
            onChange={(e) => {
              setFallbackUrl(e.target.value)
              if (fallback && e.target.value) {
                updateAction({
                  fallback: {
                    model: fallback,
                    upstream_url: e.target.value,
                  },
                })
              }
            }}
            placeholder="https://api.openai.com/v1"
          />
        </div>
      </div>
    </div>
  )
}

export default RoutingTab