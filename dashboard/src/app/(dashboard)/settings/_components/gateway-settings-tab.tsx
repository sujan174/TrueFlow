"use client"

import { useState, useEffect } from "react"
import { toast } from "sonner"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog"
import { Loader2, Save, Trash2, Database, HardDrive, RefreshCw } from "lucide-react"
import {
  getSettings,
  updateSettings,
  getCacheStats,
  flushCache,
  type GatewaySettings,
  type CacheStats,
} from "@/lib/api"
import { cn } from "@/lib/utils"

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B"
  const k = 1024
  const sizes = ["B", "KB", "MB", "GB"]
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i]
}

function StatCard({
  label,
  value,
  sublabel,
  icon: Icon,
}: {
  label: string
  value: string | number
  sublabel?: string
  icon?: React.ElementType
}) {
  return (
    <div className="p-4 border rounded-lg">
      {Icon && (
        <div className="w-8 h-8 rounded-lg bg-muted flex items-center justify-center mb-3">
          <Icon className="h-4 w-4 text-muted-foreground" />
        </div>
      )}
      <div className="text-2xl font-semibold tabular-nums">{value}</div>
      <div className="text-xs text-muted-foreground mt-0.5">{label}</div>
      {sublabel && (
        <div className="text-xs text-muted-foreground/70 mt-0.5">{sublabel}</div>
      )}
    </div>
  )
}

export function GatewaySettingsTab() {
  const [settings, setSettings] = useState<GatewaySettings>({})
  const [cacheStats, setCacheStats] = useState<CacheStats | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [isSaving, setIsSaving] = useState(false)
  const [isFlushing, setIsFlushing] = useState(false)

  useEffect(() => {
    loadData()
  }, [])

  async function loadData() {
    try {
      const [settingsData, cacheData] = await Promise.all([getSettings(), getCacheStats()])
      setSettings(settingsData)
      setCacheStats(cacheData)
    } catch (error) {
      toast.error("Failed to load settings")
      console.error(error)
    } finally {
      setIsLoading(false)
    }
  }

  async function handleSave() {
    setIsSaving(true)
    try {
      await updateSettings(settings)
      toast.success("Settings saved successfully")
    } catch (error) {
      toast.error("Failed to save settings")
      console.error(error)
    } finally {
      setIsSaving(false)
    }
  }

  async function handleFlushCache() {
    setIsFlushing(true)
    try {
      const result = await flushCache()
      toast.success(`Cache flushed: ${result.keys_deleted} keys deleted`)
      const cacheData = await getCacheStats()
      setCacheStats(cacheData)
    } catch (error) {
      toast.error("Failed to flush cache")
      console.error(error)
    } finally {
      setIsFlushing(false)
    }
  }

  function updateSetting<K extends keyof GatewaySettings>(key: K, value: GatewaySettings[K]) {
    setSettings((prev) => ({ ...prev, [key]: value }))
  }

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-16">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    )
  }

  return (
    <div className="space-y-8">
      {/* Rate Limiting Section */}
      <section>
        <h3 className="text-sm font-medium mb-4">Rate Limiting</h3>
        <div className="grid gap-6 sm:grid-cols-2">
          <div className="space-y-2">
            <Label htmlFor="default_rate_limit" className="text-xs">Default Rate Limit</Label>
            <Input
              id="default_rate_limit"
              type="number"
              value={settings.default_rate_limit ?? ""}
              onChange={(e) => updateSetting("default_rate_limit", e.target.value ? Number(e.target.value) : undefined)}
              placeholder="e.g., 100"
              className="h-9"
            />
            <p className="text-xs text-muted-foreground">Maximum requests per window</p>
          </div>
          <div className="space-y-2">
            <Label htmlFor="default_rate_limit_window" className="text-xs">Rate Limit Window (seconds)</Label>
            <Input
              id="default_rate_limit_window"
              type="number"
              value={settings.default_rate_limit_window ?? ""}
              onChange={(e) => updateSetting("default_rate_limit_window", e.target.value ? Number(e.target.value) : undefined)}
              placeholder="e.g., 60"
              className="h-9"
            />
            <p className="text-xs text-muted-foreground">Time window in seconds</p>
          </div>
        </div>
      </section>

      {/* Timeouts & Limits Section */}
      <section>
        <h3 className="text-sm font-medium mb-4">Timeouts & Limits</h3>
        <div className="grid gap-6 sm:grid-cols-2">
          <div className="space-y-2">
            <Label htmlFor="hitl_timeout_minutes" className="text-xs">HITL Timeout (minutes)</Label>
            <Input
              id="hitl_timeout_minutes"
              type="number"
              value={settings.hitl_timeout_minutes ?? ""}
              onChange={(e) => updateSetting("hitl_timeout_minutes", e.target.value ? Number(e.target.value) : undefined)}
              placeholder="e.g., 30"
              className="h-9"
            />
            <p className="text-xs text-muted-foreground">Human-in-the-loop approval timeout</p>
          </div>
          <div className="space-y-2">
            <Label htmlFor="max_request_body_bytes" className="text-xs">Max Request Body (bytes)</Label>
            <Input
              id="max_request_body_bytes"
              type="number"
              value={settings.max_request_body_bytes ?? ""}
              onChange={(e) => updateSetting("max_request_body_bytes", e.target.value ? Number(e.target.value) : undefined)}
              placeholder="e.g., 26214400"
              className="h-9"
            />
            <p className="text-xs text-muted-foreground">Maximum request body size (default: 25MB)</p>
          </div>
        </div>
      </section>

      {/* Audit Section */}
      <section>
        <h3 className="text-sm font-medium mb-4">Audit</h3>
        <div className="max-w-xs space-y-2">
          <Label htmlFor="audit_retention_days" className="text-xs">Audit Retention (days)</Label>
          <Input
            id="audit_retention_days"
            type="number"
            value={settings.audit_retention_days ?? ""}
            onChange={(e) => updateSetting("audit_retention_days", e.target.value ? Number(e.target.value) : undefined)}
            placeholder="e.g., 90"
            className="h-9"
          />
          <p className="text-xs text-muted-foreground">How long to retain audit logs</p>
        </div>
      </section>

      {/* Feature Flags Section */}
      <section>
        <h3 className="text-sm font-medium mb-4">Features</h3>
        <div className="space-y-4">
          <div className="flex items-center justify-between py-2 border-b">
            <div>
              <Label htmlFor="enable_response_cache" className="text-sm font-medium">Response Cache</Label>
              <p className="text-xs text-muted-foreground mt-0.5">Cache LLM responses for identical requests</p>
            </div>
            <Switch
              id="enable_response_cache"
              checked={settings.enable_response_cache ?? false}
              onCheckedChange={(checked) => updateSetting("enable_response_cache", checked)}
            />
          </div>
          <div className="flex items-center justify-between py-2 border-b">
            <div>
              <Label htmlFor="enable_guardrails" className="text-sm font-medium">Guardrails</Label>
              <p className="text-xs text-muted-foreground mt-0.5">Enable guardrail checks for all requests</p>
            </div>
            <Switch
              id="enable_guardrails"
              checked={settings.enable_guardrails ?? false}
              onCheckedChange={(checked) => updateSetting("enable_guardrails", checked)}
            />
          </div>
        </div>
      </section>

      {/* Integrations Section */}
      <section>
        <h3 className="text-sm font-medium mb-4">Integrations</h3>
        <div className="max-w-lg space-y-2">
          <Label htmlFor="slack_webhook_url" className="text-xs">Slack Webhook URL</Label>
          <Input
            id="slack_webhook_url"
            type="url"
            value={settings.slack_webhook_url ?? ""}
            onChange={(e) => updateSetting("slack_webhook_url", e.target.value || undefined)}
            placeholder="https://hooks.slack.com/services/..."
            className="h-9"
          />
          <p className="text-xs text-muted-foreground">Receive alerts in Slack</p>
        </div>
      </section>

      {/* Save Button */}
      <div className="flex justify-end pt-4 border-t">
        <Button onClick={handleSave} disabled={isSaving} className="gap-2">
          {isSaving ? (
            <>
              <Loader2 className="h-4 w-4 animate-spin" />
              Saving...
            </>
          ) : (
            <>
              <Save className="h-4 w-4" />
              Save Changes
            </>
          )}
        </Button>
      </div>

      {/* Cache Stats Section */}
      {cacheStats && (
        <section className="pt-8 border-t">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h3 className="text-sm font-medium">Response Cache</h3>
              <p className="text-xs text-muted-foreground mt-0.5">LLM response cache statistics</p>
            </div>
            <AlertDialog>
              <AlertDialogTrigger>
                <Button variant="outline" size="sm" className="gap-2 text-destructive hover:text-destructive">
                  <Trash2 className="h-4 w-4" />
                  Flush Cache
                </Button>
              </AlertDialogTrigger>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>Flush Response Cache?</AlertDialogTitle>
                  <AlertDialogDescription>
                    This will delete all cached LLM responses. Spend tracking and rate limit data will be preserved.
                    This action cannot be undone.
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>Cancel</AlertDialogCancel>
                  <AlertDialogAction
                    onClick={handleFlushCache}
                    disabled={isFlushing}
                    className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                  >
                    {isFlushing ? "Flushing..." : "Flush Cache"}
                  </AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
          </div>

          <div className="grid gap-4 sm:grid-cols-3">
            <StatCard
              label="Cached Responses"
              value={cacheStats.cache_key_count.toLocaleString()}
              icon={Database}
            />
            <StatCard
              label="Estimated Size"
              value={formatBytes(cacheStats.estimated_size_bytes)}
              icon={HardDrive}
            />
            <StatCard
              label="Default TTL"
              value={`${Math.floor(cacheStats.default_ttl_secs / 60)}m`}
              sublabel={`${cacheStats.default_ttl_secs}s`}
            />
          </div>

          {/* Namespace Counts */}
          <div className="mt-6 pt-4 border-t">
            <p className="text-xs font-medium text-muted-foreground mb-3">Redis Keys by Namespace</p>
            <div className="flex flex-wrap gap-2">
              <span className="inline-flex items-center gap-2 px-3 py-1.5 text-xs rounded-lg bg-muted">
                <span className="font-medium">llm_cache</span>
                <span className="text-muted-foreground">{cacheStats.namespace_counts.llm_cache}</span>
              </span>
              <span className="inline-flex items-center gap-2 px-3 py-1.5 text-xs rounded-lg bg-muted">
                <span className="font-medium">spend_tracking</span>
                <span className="text-muted-foreground">{cacheStats.namespace_counts.spend_tracking}</span>
              </span>
              <span className="inline-flex items-center gap-2 px-3 py-1.5 text-xs rounded-lg bg-muted">
                <span className="font-medium">rate_limits</span>
                <span className="text-muted-foreground">{cacheStats.namespace_counts.rate_limits}</span>
              </span>
            </div>
          </div>
        </section>
      )}
    </div>
  )
}