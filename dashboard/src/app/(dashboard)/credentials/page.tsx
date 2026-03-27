"use client"

import { useEffect, useState } from "react"
import { Plus, Key, MoreHorizontal, Trash2, Shield, Check } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { toast } from "sonner"
import {
  listCredentials,
  createCredential,
  deleteCredential,
  type CredentialMeta,
} from "@/lib/api"
import { PROVIDER_PRESETS } from "@/lib/provider-presets"

function formatRelativeTime(dateString: string): string {
  const date = new Date(dateString)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffMins = Math.floor(diffMs / 60000)
  const diffHours = Math.floor(diffMs / 3600000)
  const diffDays = Math.floor(diffMs / 86400000)

  if (diffMins < 1) return "just now"
  if (diffMins < 60) return `${diffMins}m ago`
  if (diffHours < 24) return `${diffHours}h ago`
  if (diffDays < 30) return `${diffDays}d ago`
  return date.toLocaleDateString()
}

const providerLabels: Record<string, string> = {
  openai: "OpenAI",
  anthropic: "Anthropic",
  gemini: "Google Gemini",
  azure: "Azure OpenAI",
  bedrock: "AWS Bedrock",
  cohere: "Cohere",
  mistral: "Mistral",
  groq: "Groq",
  together: "Together AI",
  ollama: "Ollama",
}

// Map UI provider names to backend provider IDs
const providerIdMap: Record<string, string> = {
  "OpenAI": "openai",
  "Anthropic": "anthropic",
  "Google Gemini": "gemini",
  "Azure OpenAI": "azure",
  "AWS Bedrock": "bedrock",
  "Cohere": "cohere",
  "Mistral": "mistral",
  "Groq": "groq",
  "Together AI": "together",
  "OpenRouter": "openrouter",
  "Ollama": "ollama",
}

// Credential Creation Modal
function CreateCredentialModal({
  open,
  onOpenChange,
  onSuccess,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onSuccess: () => void
}) {
  const [name, setName] = useState("")
  const [selectedProvider, setSelectedProvider] = useState("OpenAI")
  const [apiKey, setApiKey] = useState("")
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [created, setCreated] = useState(false)

  const currentPreset = PROVIDER_PRESETS.find(p => p.name === selectedProvider) || PROVIDER_PRESETS[0]

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setIsSubmitting(true)
    try {
      await createCredential({
        name,
        provider: providerIdMap[selectedProvider] || selectedProvider.toLowerCase(),
        secret: apiKey,
      })
      setCreated(true)
      toast.success("Credential created successfully")
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to create credential")
    } finally {
      setIsSubmitting(false)
    }
  }

  const handleClose = () => {
    if (created) {
      onSuccess()
    }
    setName("")
    setSelectedProvider("OpenAI")
    setApiKey("")
    setCreated(false)
    onOpenChange(false)
  }

  if (created) {
    return (
      <Dialog open={open} onOpenChange={handleClose}>
        <DialogContent className="sm:max-w-md" showCloseButton={false}>
          <DialogHeader>
            <DialogTitle className="text-green-600 flex items-center gap-2">
              <Check className="h-5 w-5" />
              Credential Created
            </DialogTitle>
            <DialogDescription>
              Your API key has been encrypted and stored securely. You can now use it when creating tokens.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button onClick={handleClose}>Done</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    )
  }

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Add Credential</DialogTitle>
          <DialogDescription>
            Store an upstream provider API key securely.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-3">
            <div>
              <label className="text-sm font-medium">Name</label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
                placeholder="My OpenAI Key"
                required
              />
            </div>

            <div>
              <label className="text-sm font-medium">Provider</label>
              <select
                value={selectedProvider}
                onChange={(e) => setSelectedProvider(e.target.value)}
                className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
              >
                {PROVIDER_PRESETS.filter(p => p.name !== "Custom").map((preset) => (
                  <option key={preset.name} value={preset.name}>
                    {preset.name}
                  </option>
                ))}
              </select>
              <p className="text-xs text-muted-foreground mt-1">{currentPreset.description}</p>
            </div>

            {/* Supported Models */}
            <div>
              <label className="text-sm font-medium text-muted-foreground">Supported Models</label>
              <div className="flex flex-wrap gap-1.5 mt-1">
                {currentPreset.allowed_models.map((pattern) => (
                  <Badge key={pattern} variant="outline" className="text-xs font-mono">
                    {pattern}
                  </Badge>
                ))}
              </div>
            </div>

            <div>
              <label className="text-sm font-medium">API Key</label>
              <input
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background font-mono"
                placeholder="sk-..."
                required
              />
              <p className="text-xs text-muted-foreground mt-1">
                Encrypted with AES-256-GCM and never shown after creation.
              </p>
            </div>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={handleClose}>
              Cancel
            </Button>
            <Button type="submit" disabled={isSubmitting}>
              {isSubmitting ? "Creating..." : "Create Credential"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default function CredentialsPage() {
  const [credentials, setCredentials] = useState<CredentialMeta[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [createModalOpen, setCreateModalOpen] = useState(false)

  const fetchCredentials = async () => {
    try {
      const data = await listCredentials()
      setCredentials(data)
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load credentials")
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    fetchCredentials()
  }, [])

  const handleDelete = async (id: string, name: string) => {
    if (!confirm(`Are you sure you want to delete "${name}"? This cannot be undone.`)) return

    try {
      await deleteCredential(id)
      setCredentials(credentials.filter((c) => c.id !== id))
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete credential")
    }
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex flex-col gap-1">
            <h1 className="text-2xl lg:text-3xl font-bold tracking-tight">
              Credentials
            </h1>
            <p className="text-sm text-muted-foreground">
              Securely store upstream provider API keys
            </p>
          </div>
          <Button className="gap-2" onClick={() => setCreateModalOpen(true)}>
            <Plus className="h-4 w-4" />
            Add Credential
          </Button>
        </div>

        {/* Info Banner */}
        <div className="bg-muted/50 border rounded-lg p-4 flex items-start gap-3">
          <Shield className="h-5 w-5 text-muted-foreground shrink-0 mt-0.5" />
          <div className="text-sm text-muted-foreground">
            <p className="font-medium text-foreground">Encrypted Storage</p>
            <p className="mt-1">
              API keys are encrypted with AES-256-GCM and never shown after creation.
              Delete and re-create to rotate keys.
            </p>
          </div>
        </div>

        {/* Table */}
        <div className="bg-card border rounded-xl shadow-sm overflow-hidden">
          {loading ? (
            <div className="p-8 text-center text-muted-foreground">
              Loading credentials...
            </div>
          ) : error ? (
            <div className="p-8 text-center text-destructive">{error}</div>
          ) : credentials.length === 0 ? (
            <div className="p-8 text-center">
              <Key className="h-12 w-12 mx-auto text-muted-foreground/50 mb-4" />
              <p className="text-muted-foreground">No credentials yet</p>
              <p className="text-sm text-muted-foreground/70 mt-1">
                Add your first upstream API key to get started
              </p>
            </div>
          ) : (
            <table className="w-full">
              <thead className="bg-muted/50 border-b">
                <tr className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                  <th className="px-4 py-3 text-left">Name</th>
                  <th className="px-4 py-3 text-left">Provider</th>
                  <th className="px-4 py-3 text-left">Status</th>
                  <th className="px-4 py-3 text-left">Created</th>
                  <th className="px-4 py-3 text-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {credentials.map((credential) => (
                  <tr
                    key={credential.id}
                    className="border-b last:border-0 hover:bg-muted/30 transition-colors"
                  >
                    <td className="px-4 py-3">
                      <span className="text-sm font-medium">{credential.name}</span>
                    </td>
                    <td className="px-4 py-3">
                      <span className="text-sm text-muted-foreground">
                        {providerLabels[credential.provider] || credential.provider}
                      </span>
                    </td>
                    <td className="px-4 py-3">
                      <Badge
                        variant={credential.is_active ? "default" : "destructive"}
                        className="text-[10px]"
                      >
                        {credential.is_active ? "Active" : "Inactive"}
                      </Badge>
                    </td>
                    <td className="px-4 py-3">
                      <span className="text-sm text-muted-foreground">
                        {formatRelativeTime(credential.created_at)}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-right">
                      <DropdownMenu>
                        <DropdownMenuTrigger>
                          <Button variant="ghost" size="icon-sm">
                            <MoreHorizontal className="h-4 w-4" />
                          </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem
                            className="text-destructive"
                            onClick={() => handleDelete(credential.id, credential.name)}
                          >
                            <Trash2 className="h-4 w-4 mr-2" />
                            Delete
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>

      {/* Create Credential Modal */}
      <CreateCredentialModal
        open={createModalOpen}
        onOpenChange={setCreateModalOpen}
        onSuccess={fetchCredentials}
      />
    </div>
  )
}