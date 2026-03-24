"use client"

import { useState, useEffect } from "react"
import { Shield, Loader2 } from "lucide-react"
import { toast } from "sonner"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Badge } from "@/components/ui/badge"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  enableGuardrails,
  listGuardrailPresets,
  type GuardrailPreset,
  type GuardrailScope,
  type TokenRow,
  listTokens,
} from "@/lib/api"

interface GuardrailPresetDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  initialTokenId?: string
  onSuccess?: () => void
}

const PRESET_CATEGORIES = [
  { id: "safety", label: "Safety & Security", color: "bg-red-100 text-red-800" },
  { id: "privacy", label: "PII Protection", color: "bg-blue-100 text-blue-800" },
  { id: "compliance", label: "Compliance", color: "bg-yellow-100 text-yellow-800" },
  { id: "business", label: "Business", color: "bg-purple-100 text-purple-800" },
  { id: "enterprise", label: "Enterprise", color: "bg-orange-100 text-orange-800" },
  { id: "output_safety", label: "Output Guardrails", color: "bg-green-100 text-green-800" },
  { id: "output_privacy", label: "Output PII Protection", color: "bg-teal-100 text-teal-800" },
]

export function GuardrailPresetDialog({
  open,
  onOpenChange,
  initialTokenId,
  onSuccess,
}: GuardrailPresetDialogProps) {
  const [tokens, setTokens] = useState<TokenRow[]>([])
  const [presets, setPresets] = useState<GuardrailPreset[]>([])
  const [loading, setLoading] = useState(false)
  const [loadingPresets, setLoadingPresets] = useState(false)
  const [selectedToken, setSelectedToken] = useState<string>(initialTokenId || "")
  const [selectedPresets, setSelectedPresets] = useState<string[]>([])
  const [scopeModels, setScopeModels] = useState<string>("")
  const [scopePaths, setScopePaths] = useState<string>("")
  const [error, setError] = useState<string | null>(null)

  // Load tokens and presets on dialog open
  useEffect(() => {
    if (open) {
      loadTokens()
      loadPresets()
    }
  }, [open])

  const loadTokens = async () => {
    setLoading(true)
    try {
      const data = await listTokens(100)
      setTokens(data.filter((t) => t.is_active))
    } catch (err) {
      console.error("Failed to load tokens:", err)
    } finally {
      setLoading(false)
    }
  }

  const loadPresets = async () => {
    setLoadingPresets(true)
    try {
      const response = await listGuardrailPresets()
      setPresets(response.presets)
    } catch (err) {
      console.error("Failed to load presets:", err)
      // Use hardcoded presets as fallback
      setPresets(getFallbackPresets())
    } finally {
      setLoadingPresets(false)
    }
  }

  const getFallbackPresets = (): GuardrailPreset[] => [
    { name: "prompt_injection", description: "Block jailbreaks, harmful content, code injection", category: "safety", actions: ["content_filter"] },
    { name: "code_injection", description: "Block SQL injection, shell commands, XSS", category: "safety", actions: ["content_filter"] },
    { name: "pii_redaction", description: "Redact SSN, email, credit card, phone, etc.", category: "privacy", actions: ["redact"] },
    { name: "pii_block", description: "Block requests containing PII", category: "privacy", actions: ["redact"] },
    { name: "hipaa", description: "Healthcare PII: SSN, phone, DOB, email, MRN", category: "compliance", actions: ["redact"] },
    { name: "toxicity", description: "Block profanity, bias, hate speech", category: "safety", actions: ["content_filter"] },
    { name: "profanity_filter", description: "Block profanity/slurs only", category: "safety", actions: ["content_filter"] },
    { name: "sensitive_topics", description: "Block political/religious/medical/legal advice", category: "compliance", actions: ["content_filter"] },
    { name: "competitor_block", description: "Block competitor mentions", category: "business", actions: ["content_filter"] },
    { name: "ip_protection", description: "Block trade secret/confidentiality leaks", category: "enterprise", actions: ["content_filter"] },
    { name: "strict_enterprise", description: "All-in-one: injection + toxicity + PII + IP", category: "enterprise", actions: ["content_filter", "redact"] },
    { name: "output_content_filter", description: "Filter harmful content in responses", category: "output_safety", actions: ["content_filter"] },
    { name: "output_pii_redaction", description: "Redact PII in responses", category: "output_privacy", actions: ["redact"] },
  ]

  const handlePresetToggle = (presetName: string) => {
    setSelectedPresets((prev) =>
      prev.includes(presetName)
        ? prev.filter((p) => p !== presetName)
        : [...prev, presetName]
    )
  }

  const handleSubmit = async () => {
    if (!selectedToken) {
      setError("Please select a token")
      return
    }
    if (selectedPresets.length === 0) {
      setError("Please select at least one preset")
      return
    }

    setError(null)
    setLoading(true)

    try {
      // Parse scope models/paths, filtering out empty strings
      const models = scopeModels
        ? scopeModels.split(",").map((m) => m.trim()).filter((m) => m.length > 0)
        : undefined
      const paths = scopePaths
        ? scopePaths.split(",").map((p) => p.trim()).filter((p) => p.length > 0)
        : undefined

      const scope: GuardrailScope | undefined =
        (models && models.length > 0) || (paths && paths.length > 0)
          ? { models, paths }
          : undefined

      await enableGuardrails({
        token_id: selectedToken,
        presets: selectedPresets,
        source: "dashboard",
        scope,
      })

      toast.success(`Enabled ${selectedPresets.length} guardrail${selectedPresets.length > 1 ? "s" : ""}`)
      onSuccess?.()
      onOpenChange(false)
      // Reset form
      setSelectedPresets([])
      setScopeModels("")
      setScopePaths("")
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to enable guardrails")
    } finally {
      setLoading(false)
    }
  }

  const presetsByCategory = PRESET_CATEGORIES.reduce((acc, cat) => {
    acc[cat.id] = presets.filter((p) => p.category === cat.id)
    return acc
  }, {} as Record<string, GuardrailPreset[]>)

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Shield className="h-5 w-5" />
            Enable Guardrails
          </DialogTitle>
          <DialogDescription>
            Select preset guardrails to apply to a token. Guardrails will be applied as policy rules.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-6 py-4">
          {/* Token Selection */}
          <div className="space-y-2">
            <Label htmlFor="token">Token</Label>
            <Select value={selectedToken} onValueChange={(value) => value && setSelectedToken(value)}>
              <SelectTrigger id="token">
                <SelectValue placeholder="Select a token" />
              </SelectTrigger>
              <SelectContent>
                {tokens.map((token) => (
                  <SelectItem key={token.id} value={token.id}>
                    {token.name} ({token.id.slice(0, 12)}...)
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {/* Preset Selection by Category */}
          <div className="space-y-4">
            <Label>Guardrail Presets</Label>
            {loadingPresets ? (
              <div className="flex items-center justify-center py-8">
                <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
              </div>
            ) : (
              PRESET_CATEGORIES.map((category) => {
                const categoryPresets = presetsByCategory[category.id] || []
                if (categoryPresets.length === 0) return null

                return (
                  <div key={category.id} className="space-y-2">
                    <Badge variant="outline" className={category.color}>
                      {category.label}
                    </Badge>
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-2 pl-2">
                      {categoryPresets.map((preset) => (
                        <div
                          key={preset.name}
                          className="flex items-start space-x-2 p-2 rounded-lg border hover:bg-muted/50 cursor-pointer"
                          onClick={() => handlePresetToggle(preset.name)}
                        >
                          <Checkbox
                            id={preset.name}
                            checked={selectedPresets.includes(preset.name)}
                            onCheckedChange={() => handlePresetToggle(preset.name)}
                          />
                          <div className="flex-1">
                            <label
                              htmlFor={preset.name}
                              className="text-sm font-medium cursor-pointer"
                            >
                              {preset.name.replace(/_/g, " ")}
                            </label>
                            <p className="text-xs text-muted-foreground">
                              {preset.description}
                            </p>
                            {preset.warning && (
                              <p className="text-xs text-amber-600 mt-1">
                                {preset.warning}
                              </p>
                            )}
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                )
              })
            )}
          </div>

          {/* Scope Options */}
          <div className="space-y-4 border-t pt-4">
            <div>
              <Label className="text-base">Scope (Optional)</Label>
              <p className="text-sm text-muted-foreground">
                Limit guardrails to specific models or paths. Leave empty to apply to all requests.
              </p>
            </div>

            <div className="grid gap-4 md:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="scope-models">Models</Label>
                <Input
                  id="scope-models"
                  placeholder="gpt-4, claude-*"
                  value={scopeModels}
                  onChange={(e) => setScopeModels(e.target.value)}
                />
                <p className="text-xs text-muted-foreground">
                  Comma-separated model names or prefixes
                </p>
              </div>

              <div className="space-y-2">
                <Label htmlFor="scope-paths">Paths</Label>
                <Input
                  id="scope-paths"
                  placeholder="/v1/chat/*, /v1/completions"
                  value={scopePaths}
                  onChange={(e) => setScopePaths(e.target.value)}
                />
                <p className="text-xs text-muted-foreground">
                  Comma-separated paths (glob patterns supported)
                </p>
              </div>
            </div>
          </div>

          {/* Error */}
          {error && (
            <div className="text-sm text-destructive bg-destructive/10 p-3 rounded-lg">
              {error}
            </div>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleSubmit} disabled={loading || !selectedToken || selectedPresets.length === 0}>
            {loading && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
            Enable {selectedPresets.length > 0 ? `(${selectedPresets.length})` : ""}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}