"use client"

import { useEffect, useState } from "react"
import { Plus, MoreHorizontal, Trash2, Key, Layers, Copy, Check, Eye, EyeOff, Pencil } from "lucide-react"
import { Button } from "@/components/ui/button"
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
  listApiKeys,
  createApiKey,
  revokeApiKey,
  updateApiKey,
  listModelAccessGroups,
  createModelAccessGroup,
  updateModelAccessGroup,
  deleteModelAccessGroup,
  type ApiKey,
  type CreateApiKeyRequest,
  type CreateApiKeyResponse,
  type UpdateApiKeyRequest,
  type ModelAccessGroup,
  type CreateModelAccessGroupRequest,
} from "@/lib/api"
import { cn } from "@/lib/utils"
import { SettingsSidebar } from "../_components/settings-sidebar"
import { usePermissions } from "@/contexts/permissions-context"

function formatRelativeTime(dateString: string | null): string {
  if (!dateString) return "Never"
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

function RoleBadge({ role }: { role: string }) {
  const styles: Record<string, string> = {
    SuperAdmin: "bg-destructive/10 text-destructive",
    Admin: "bg-primary/10 text-primary",
    Member: "bg-muted text-muted-foreground",
    ReadOnly: "bg-muted text-muted-foreground",
  }

  return (
    <span className={cn("inline-flex px-2 py-0.5 text-xs font-medium rounded-full", styles[role] || "bg-muted text-muted-foreground")}>
      {role}
    </span>
  )
}

// Available scopes organized by category
const SCOPE_CATEGORIES = [
  {
    name: "Core Resources",
    scopes: [
      { id: "tokens:read", label: "Tokens (Read)", description: "List and view tokens" },
      { id: "tokens:write", label: "Tokens (Write)", description: "Create, update, revoke tokens" },
      { id: "credentials:read", label: "Credentials (Read)", description: "List credentials" },
      { id: "credentials:write", label: "Credentials (Write)", description: "Add and delete credentials" },
      { id: "policies:read", label: "Policies (Read)", description: "List and view policies" },
      { id: "policies:write", label: "Policies (Write)", description: "Create, update, delete policies" },
      { id: "prompts:read", label: "Prompts (Read)", description: "List, view, render prompts" },
      { id: "prompts:write", label: "Prompts (Write)", description: "Create, update, deploy prompts" },
    ]
  },
  {
    name: "Analytics & Audit",
    scopes: [
      { id: "analytics:read", label: "Analytics", description: "View usage analytics" },
      { id: "audit:read", label: "Audit Logs", description: "View audit logs" },
      { id: "billing:read", label: "Billing", description: "View billing information" },
    ]
  },
  {
    name: "Integrations",
    scopes: [
      { id: "webhooks:read", label: "Webhooks (Read)", description: "List webhooks" },
      { id: "webhooks:write", label: "Webhooks (Write)", description: "Create and delete webhooks" },
      { id: "mcp:read", label: "MCP (Read)", description: "List MCP servers" },
      { id: "mcp:write", label: "MCP (Write)", description: "Configure MCP servers" },
      { id: "services:read", label: "Services (Read)", description: "List external services" },
      { id: "services:write", label: "Services (Write)", description: "Configure services" },
    ]
  },
  {
    name: "Advanced",
    scopes: [
      { id: "keys:manage", label: "API Keys", description: "Create and revoke API keys" },
      { id: "projects:read", label: "Projects (Read)", description: "List projects" },
      { id: "projects:write", label: "Projects (Write)", description: "Create and delete projects" },
      { id: "config:read", label: "Config (Read)", description: "Export configuration" },
      { id: "config:write", label: "Config (Write)", description: "Import configuration" },
      { id: "experiments:read", label: "Experiments (Read)", description: "View experiments" },
      { id: "experiments:write", label: "Experiments (Write)", description: "Manage experiments" },
      { id: "approvals:read", label: "Approvals (Read)", description: "View approval requests" },
      { id: "approvals:write", label: "Approvals (Write)", description: "Approve/reject requests" },
    ]
  }
]

// Scope presets for quick selection
const SCOPE_PRESETS = [
  { name: "Full Access", scopes: ["*"] },
  { name: "Prompt Manager", scopes: ["prompts:*"] },
  { name: "Token Manager", scopes: ["tokens:*", "credentials:read"] },
  { name: "Analytics Viewer", scopes: ["analytics:read", "audit:read"] },
  { name: "Developer", scopes: ["tokens:*", "credentials:*", "policies:*", "prompts:*"] },
  { name: "Read Only", scopes: ["tokens:read", "credentials:read", "policies:read", "prompts:read", "analytics:read", "audit:read"] },
]

function CreateApiKeyModal({
  open,
  onOpenChange,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onSubmit: (data: CreateApiKeyRequest) => Promise<CreateApiKeyResponse>
}) {
  const [name, setName] = useState("")
  const [role, setRole] = useState<"Admin" | "Member" | "ReadOnly">("Member")
  const [selectedScopes, setSelectedScopes] = useState<string[]>([])
  const [isSubmitting, setIsSubmitting] = useState(false)

  // When role changes, set default scopes
  useEffect(() => {
    if (role === "Admin") {
      setSelectedScopes(["*"])
    } else if (role === "Member") {
      setSelectedScopes(["tokens:*", "credentials:*", "policies:*", "prompts:*", "analytics:read", "audit:read"])
    } else {
      setSelectedScopes(["tokens:read", "credentials:read", "policies:read", "prompts:read", "analytics:read", "audit:read"])
    }
  }, [role])

  const toggleScope = (scope: string) => {
    setSelectedScopes(prev =>
      prev.includes(scope)
        ? prev.filter(s => s !== scope)
        : [...prev, scope]
    )
  }

  const applyPreset = (scopes: string[]) => {
    setSelectedScopes(scopes)
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setIsSubmitting(true)
    try {
      await onSubmit({ name, role, scopes: selectedScopes })
      setName("")
      setRole("Member")
      setSelectedScopes([])
    } catch (error) {
      // Error handled in parent
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Create API Key</DialogTitle>
          <DialogDescription>
            Generate a new API key for programmatic access to the gateway.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="text-sm font-medium">Name</label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
                placeholder="CI/CD Pipeline"
                required
              />
            </div>
            <div>
              <label className="text-sm font-medium">Role</label>
              <select
                value={role}
                onChange={(e) => setRole(e.target.value as "Admin" | "Member" | "ReadOnly")}
                className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
              >
                <option value="Admin">Admin - Full access</option>
                <option value="Member">Member - Read/write</option>
                <option value="ReadOnly">ReadOnly - Read-only</option>
              </select>
            </div>
          </div>

          {/* Scope Presets */}
          <div>
            <label className="text-sm font-medium">Quick Presets</label>
            <div className="flex flex-wrap gap-2 mt-2">
              {SCOPE_PRESETS.map((preset) => (
                <button
                  key={preset.name}
                  type="button"
                  onClick={() => applyPreset(preset.scopes)}
                  className={cn(
                    "px-3 py-1.5 text-xs font-medium rounded-full border transition-colors",
                    JSON.stringify(selectedScopes.sort()) === JSON.stringify(preset.scopes.sort())
                      ? "bg-primary text-primary-foreground border-primary"
                      : "bg-background hover:bg-muted"
                  )}
                >
                  {preset.name}
                </button>
              ))}
            </div>
          </div>

          {/* Scope Categories */}
          <div>
            <label className="text-sm font-medium">Scopes</label>
            <p className="text-xs text-muted-foreground mt-1 mb-2">
              Fine-tune permissions for this key. Wildcards like <code className="text-xs">tokens:*</code> grant all actions on a resource.
            </p>
            <div className="space-y-3">
              {SCOPE_CATEGORIES.map((category) => (
                <div key={category.name} className="border rounded-lg p-3">
                  <h4 className="text-xs font-semibold text-muted-foreground uppercase tracking-wide mb-2">
                    {category.name}
                  </h4>
                  <div className="grid grid-cols-2 gap-2">
                    {category.scopes.map((scope) => (
                      <label
                        key={scope.id}
                        className="flex items-start gap-2 p-2 rounded hover:bg-muted/50 cursor-pointer"
                      >
                        <input
                          type="checkbox"
                          checked={selectedScopes.includes(scope.id) || selectedScopes.includes("*") || selectedScopes.includes(scope.id.split(":")[0] + ":*")}
                          disabled={selectedScopes.includes("*") || selectedScopes.includes(scope.id.split(":")[0] + ":*")}
                          onChange={() => toggleScope(scope.id)}
                          className="mt-0.5"
                        />
                        <div className="flex-1 min-w-0">
                          <div className="text-sm font-medium">{scope.label}</div>
                          <div className="text-xs text-muted-foreground">{scope.description}</div>
                        </div>
                      </label>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={isSubmitting}>
              {isSubmitting ? "Creating..." : "Create Key"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

function EditApiKeyModal({
  open,
  onOpenChange,
  apiKey,
  onSuccess,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  apiKey: ApiKey | null
  onSuccess: () => void
}) {
  const [name, setName] = useState("")
  const [selectedScopes, setSelectedScopes] = useState<string[]>([])
  const [isSubmitting, setIsSubmitting] = useState(false)

  // Initialize form when apiKey changes
  useEffect(() => {
    if (apiKey) {
      setName(apiKey.name)
      setSelectedScopes(apiKey.scopes || [])
    }
  }, [apiKey, open])

  const toggleScope = (scope: string) => {
    setSelectedScopes(prev =>
      prev.includes(scope)
        ? prev.filter(s => s !== scope)
        : [...prev, scope]
    )
  }

  const applyPreset = (scopes: string[]) => {
    setSelectedScopes(scopes)
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!apiKey) return

    setIsSubmitting(true)
    try {
      await updateApiKey(apiKey.id, { name, scopes: selectedScopes })
      toast.success("API key updated successfully")
      onSuccess()
      onOpenChange(false)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to update API key")
    } finally {
      setIsSubmitting(false)
    }
  }

  if (!apiKey) return null

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Edit API Key</DialogTitle>
          <DialogDescription>
            Update the name and scopes for this API key.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="text-sm font-medium">Name</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
              placeholder="CI/CD Pipeline"
              required
            />
          </div>

          {/* Scope Presets */}
          <div>
            <label className="text-sm font-medium">Quick Presets</label>
            <div className="flex flex-wrap gap-2 mt-2">
              {SCOPE_PRESETS.map((preset) => (
                <button
                  key={preset.name}
                  type="button"
                  onClick={() => applyPreset(preset.scopes)}
                  className={cn(
                    "px-3 py-1.5 text-xs font-medium rounded-full border transition-colors",
                    JSON.stringify(selectedScopes.sort()) === JSON.stringify(preset.scopes.sort())
                      ? "bg-primary text-primary-foreground border-primary"
                      : "bg-background hover:bg-muted"
                  )}
                >
                  {preset.name}
                </button>
              ))}
            </div>
          </div>

          {/* Scope Categories */}
          <div>
            <label className="text-sm font-medium">Scopes</label>
            <p className="text-xs text-muted-foreground mt-1 mb-2">
              Fine-tune permissions for this key. Wildcards like <code className="text-xs">tokens:*</code> grant all actions on a resource.
            </p>
            <div className="space-y-3">
              {SCOPE_CATEGORIES.map((category) => (
                <div key={category.name} className="border rounded-lg p-3">
                  <h4 className="text-xs font-semibold text-muted-foreground uppercase tracking-wide mb-2">
                    {category.name}
                  </h4>
                  <div className="grid grid-cols-2 gap-2">
                    {category.scopes.map((scope) => (
                      <label
                        key={scope.id}
                        className="flex items-start gap-2 p-2 rounded hover:bg-muted/50 cursor-pointer"
                      >
                        <input
                          type="checkbox"
                          checked={selectedScopes.includes(scope.id) || selectedScopes.includes("*") || selectedScopes.includes(scope.id.split(":")[0] + ":*")}
                          disabled={selectedScopes.includes("*") || selectedScopes.includes(scope.id.split(":")[0] + ":*")}
                          onChange={() => toggleScope(scope.id)}
                          className="mt-0.5"
                        />
                        <div className="flex-1 min-w-0">
                          <div className="text-sm font-medium">{scope.label}</div>
                          <div className="text-xs text-muted-foreground">{scope.description}</div>
                        </div>
                      </label>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={isSubmitting}>
              {isSubmitting ? "Saving..." : "Save Changes"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

function ShowKeyModal({
  open,
  onOpenChange,
  apiKey,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  apiKey: CreateApiKeyResponse | null
}) {
  const [copied, setCopied] = useState(false)
  const [showKey, setShowKey] = useState(false)

  const handleCopy = async () => {
    if (apiKey?.key) {
      await navigator.clipboard.writeText(apiKey.key)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }
  }

  if (!apiKey) return null

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md" showCloseButton={false}>
        <DialogHeader>
          <DialogTitle className="text-success">API Key Created</DialogTitle>
          <DialogDescription>
            Save this key now. It will never be shown again.
          </DialogDescription>
        </DialogHeader>
        <div className="bg-muted rounded-lg p-3 font-mono text-sm break-all relative">
          <div className="flex items-center gap-2">
            <code className="flex-1 text-xs">
              {showKey ? apiKey.key : "••••••••••••••••••••••••••••••••"}
            </code>
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={() => setShowKey(!showKey)}
              title={showKey ? "Hide key" : "Show key"}
            >
              {showKey ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={handleCopy}
              title="Copy to clipboard"
            >
              {copied ? <Check className="h-4 w-4 text-success" /> : <Copy className="h-4 w-4" />}
            </Button>
          </div>
        </div>
        <p className="text-xs text-muted-foreground">
          <strong>Key ID:</strong> {apiKey.id}
        </p>
        <DialogFooter>
          <Button
            onClick={() => {
              handleCopy()
              onOpenChange(false)
            }}
          >
            Copy & Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function ModelGroupFormModal({
  open,
  onOpenChange,
  group,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  group?: ModelAccessGroup
  onSubmit: (data: CreateModelAccessGroupRequest) => Promise<void>
}) {
  const [name, setName] = useState(group?.name || "")
  const [description, setDescription] = useState(group?.description || "")
  const [models, setModels] = useState(group?.models?.join(", ") || "")
  const [isSubmitting, setIsSubmitting] = useState(false)

  useEffect(() => {
    if (group) {
      setName(group.name)
      setDescription(group?.description || "")
      setModels(group?.models?.join(", ") || "")
    } else {
      setName("")
      setDescription("")
      setModels("")
    }
  }, [group, open])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setIsSubmitting(true)
    try {
      await onSubmit({
        name,
        description: description || undefined,
        models: models
          .split(",")
          .map((m) => m.trim())
          .filter(Boolean),
      })
      onOpenChange(false)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to save group")
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{group ? "Edit Model Group" : "Create Model Group"}</DialogTitle>
          <DialogDescription>
            Define a group of models that can be assigned to tokens.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="text-sm font-medium">Name</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
              placeholder="GPT-4 Family"
              required
            />
          </div>
          <div>
            <label className="text-sm font-medium">Description</label>
            <input
              type="text"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
              placeholder="Optional description"
            />
          </div>
          <div>
            <label className="text-sm font-medium">Models</label>
            <textarea
              value={models}
              onChange={(e) => setModels(e.target.value)}
              className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background min-h-[80px]"
              placeholder="gpt-4*, gpt-4o, claude-* (comma-separated)"
            />
            <p className="text-xs text-muted-foreground mt-1">
              Use glob patterns like gpt-4* or claude-* for wildcards
            </p>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={isSubmitting}>
              {isSubmitting ? "Saving..." : group ? "Update" : "Create"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

export default function AccessControlPage() {
  const { isAdmin } = usePermissions()
  const [activeTab, setActiveTab] = useState<"keys" | "groups">("keys")
  const [apiKeys, setApiKeys] = useState<ApiKey[]>([])
  const [modelGroups, setModelGroups] = useState<ModelAccessGroup[]>([])
  const [loading, setLoading] = useState(true)
  const [createKeyOpen, setCreateKeyOpen] = useState(false)
  const [showKeyOpen, setShowKeyOpen] = useState(false)
  const [editKeyOpen, setEditKeyOpen] = useState(false)
  const [editingKey, setEditingKey] = useState<ApiKey | null>(null)
  const [newKey, setNewKey] = useState<CreateApiKeyResponse | null>(null)
  const [groupModalOpen, setGroupModalOpen] = useState(false)
  const [editingGroup, setEditingGroup] = useState<ModelAccessGroup | undefined>()

  useEffect(() => {
    async function fetchData() {
      try {
        const [keys, groups] = await Promise.all([listApiKeys(), listModelAccessGroups()])
        setApiKeys(keys)
        setModelGroups(groups)
      } catch (err) {
        console.error("Failed to load access data:", err)
      } finally {
        setLoading(false)
      }
    }
    fetchData()
  }, [])

  const fetchApiKeys = async () => {
    try {
      const keys = await listApiKeys()
      setApiKeys(keys)
    } catch (err) {
      console.error("Failed to refresh API keys:", err)
    }
  }

  const handleCreateKey = async (data: CreateApiKeyRequest) => {
    const response = await createApiKey(data)
    const newApiKey: ApiKey = {
      id: response.id,
      org_id: "",
      user_id: null,
      name: data.name,
      key_prefix: response.key.slice(0, 16),
      role: data.role,
      scopes: data.scopes || [],
      is_active: true,
      last_used_at: null,
      expires_at: null,
      created_at: new Date().toISOString(),
    }
    setApiKeys([...apiKeys, newApiKey])
    setNewKey(response)
    setCreateKeyOpen(false)
    setShowKeyOpen(true)
    toast.success("API key created successfully")
    return response
  }

  const handleRevokeKey = async (id: string) => {
    if (!confirm("Are you sure you want to revoke this API key? This cannot be undone.")) return
    try {
      await revokeApiKey(id)
      setApiKeys(apiKeys.map((k) => (k.id === id ? { ...k, is_active: false } : k)))
      toast.success("API key revoked")
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to revoke key")
    }
  }

  const handleCreateGroup = async (data: CreateModelAccessGroupRequest) => {
    const group = await createModelAccessGroup(data)
    setModelGroups([...modelGroups, group])
    toast.success("Model group created successfully")
  }

  const handleUpdateGroup = async (data: CreateModelAccessGroupRequest) => {
    if (!editingGroup) return
    const updated = await updateModelAccessGroup(editingGroup.id, data)
    setModelGroups(modelGroups.map((g) => (g.id === editingGroup.id ? updated : g)))
    setEditingGroup(undefined)
    toast.success("Model group updated successfully")
  }

  const handleDeleteGroup = async (id: string) => {
    if (!confirm("Are you sure you want to delete this model group?")) return
    try {
      await deleteModelAccessGroup(id)
      setModelGroups(modelGroups.filter((g) => g.id !== id))
      toast.success("Model group deleted")
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to delete group")
    }
  }

  return (
    <div className="flex-1 flex min-w-0">
      <SettingsSidebar />

      {/* Main Content */}
      <div className="flex-1 flex flex-col min-w-0 overflow-auto">
        <div className="flex-1 p-6 lg:p-8">
          {/* Header */}
          <header className="flex items-start justify-between mb-8">
            <div>
              <h1 className="text-xl font-semibold tracking-tight">Access Control</h1>
              <p className="text-sm text-muted-foreground mt-1">
                Manage API keys and model access groups
              </p>
            </div>
            {isAdmin && (
              <Button
                onClick={() => {
                  if (activeTab === "keys") {
                    setCreateKeyOpen(true)
                  } else {
                    setEditingGroup(undefined)
                    setGroupModalOpen(true)
                  }
                }}
                className="gap-2"
              >
                <Plus className="h-4 w-4" />
                {activeTab === "keys" ? "Create Key" : "Create Group"}
              </Button>
            )}
          </header>

          {/* Tabs */}
          <div className="flex gap-1 p-1 bg-muted rounded-lg w-fit mb-6">
            <button
              onClick={() => setActiveTab("keys")}
              className={cn(
                "flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-md transition-colors",
                activeTab === "keys"
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground"
              )}
            >
              <Key className="h-4 w-4" />
              API Keys
            </button>
            <button
              onClick={() => setActiveTab("groups")}
              className={cn(
                "flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-md transition-colors",
                activeTab === "groups"
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground hover:text-foreground"
              )}
            >
              <Layers className="h-4 w-4" />
              Model Groups
            </button>
          </div>

          {/* Content */}
          {loading ? (
            <div className="border rounded-lg p-12 text-center">
              <div className="w-8 h-8 border-2 border-muted-foreground border-t-foreground rounded-full animate-spin mx-auto" />
            </div>
          ) : activeTab === "keys" ? (
            <div className="border rounded-lg">
              {apiKeys.length === 0 ? (
                <div className="p-12 text-center">
                  <div className="w-12 h-12 rounded-full bg-muted flex items-center justify-center mx-auto mb-4">
                    <Key className="h-6 w-6 text-muted-foreground" />
                  </div>
                  <p className="text-sm font-medium mb-1">No API keys yet</p>
                  <p className="text-sm text-muted-foreground">
                    Create your first API key for programmatic access
                  </p>
                </div>
              ) : (
                <table className="w-full">
                  <thead>
                    <tr className="border-b bg-muted/30">
                      <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Name</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Key</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Role</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Scopes</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Status</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Last Used</th>
                      <th className="px-4 py-3 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                        <span className="sr-only">Actions</span>
                      </th>
                    </tr>
                  </thead>
                  <tbody className="divide-y">
                    {apiKeys.map((key) => (
                      <tr key={key.id} className="hover:bg-muted/30 transition-colors">
                        <td className="px-4 py-3">
                          <span className="text-sm font-medium">{key.name}</span>
                        </td>
                        <td className="px-4 py-3">
                          <code className="text-xs text-muted-foreground font-mono">
                            {key.key_prefix}...
                          </code>
                        </td>
                        <td className="px-4 py-3">
                          <RoleBadge role={key.role} />
                        </td>
                        <td className="px-4 py-3">
                          <div className="flex flex-wrap gap-1 max-w-[200px]">
                            {key.scopes && key.scopes.length > 0 ? (
                              key.scopes.slice(0, 3).map((scope) => (
                                <span
                                  key={scope}
                                  className="inline-flex px-1.5 py-0.5 text-[10px] font-mono rounded bg-muted"
                                >
                                  {scope}
                                </span>
                              ))
                            ) : (
                              <span className="text-xs text-muted-foreground">—</span>
                            )}
                            {key.scopes && key.scopes.length > 3 && (
                              <span className="text-xs text-muted-foreground">
                                +{key.scopes.length - 3}
                              </span>
                            )}
                          </div>
                        </td>
                        <td className="px-4 py-3">
                          <span className={cn(
                            "inline-flex px-2 py-0.5 text-xs font-medium rounded-full",
                            key.is_active ? "bg-success/10 text-success" : "bg-muted text-muted-foreground"
                          )}>
                            {key.is_active ? "Active" : "Revoked"}
                          </span>
                        </td>
                        <td className="px-4 py-3">
                          <span className="text-xs text-muted-foreground">
                            {formatRelativeTime(key.last_used_at)}
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
                              {key.is_active && isAdmin && (
                                <>
                                  <DropdownMenuItem
                                    onClick={() => {
                                      setEditingKey(key)
                                      setEditKeyOpen(true)
                                    }}
                                  >
                                    <Pencil className="h-4 w-4 mr-2" />
                                    Edit
                                  </DropdownMenuItem>
                                  <DropdownMenuItem
                                    className="text-destructive"
                                    onClick={() => handleRevokeKey(key.id)}
                                  >
                                    <Trash2 className="h-4 w-4 mr-2" />
                                    Revoke
                                  </DropdownMenuItem>
                                </>
                              )}
                            </DropdownMenuContent>
                          </DropdownMenu>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          ) : (
            <div className="border rounded-lg">
              {modelGroups.length === 0 ? (
                <div className="p-12 text-center">
                  <div className="w-12 h-12 rounded-full bg-muted flex items-center justify-center mx-auto mb-4">
                    <Layers className="h-6 w-6 text-muted-foreground" />
                  </div>
                  <p className="text-sm font-medium mb-1">No model access groups yet</p>
                  <p className="text-sm text-muted-foreground">
                    Create groups to define which models tokens can access
                  </p>
                </div>
              ) : (
                <table className="w-full">
                  <thead>
                    <tr className="border-b bg-muted/30">
                      <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Name</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Description</th>
                      <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">Models</th>
                      <th className="px-4 py-3 text-right text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                        <span className="sr-only">Actions</span>
                      </th>
                    </tr>
                  </thead>
                  <tbody className="divide-y">
                    {modelGroups.map((group) => (
                      <tr key={group.id} className="hover:bg-muted/30 transition-colors">
                        <td className="px-4 py-3">
                          <span className="text-sm font-medium">{group.name}</span>
                        </td>
                        <td className="px-4 py-3">
                          <span className="text-sm text-muted-foreground">
                            {group.description || "—"}
                          </span>
                        </td>
                        <td className="px-4 py-3">
                          <div className="flex flex-wrap gap-1 max-w-[300px]">
                            {group.models?.slice(0, 3).map((model, i) => (
                              <span key={i} className="inline-flex px-2 py-0.5 text-xs font-mono rounded bg-muted">
                                {model}
                              </span>
                            ))}
                            {group.models && group.models.length > 3 && (
                              <span className="text-xs text-muted-foreground">
                                +{group.models.length - 3} more
                              </span>
                            )}
                          </div>
                        </td>
                        <td className="px-4 py-3 text-right">
                          <DropdownMenu>
                            <DropdownMenuTrigger>
                              <Button variant="ghost" size="icon-sm">
                                <MoreHorizontal className="h-4 w-4" />
                              </Button>
                            </DropdownMenuTrigger>
                            <DropdownMenuContent align="end">
                              {isAdmin && (
                                <>
                                  <DropdownMenuItem
                                    onClick={() => {
                                      setEditingGroup(group)
                                      setGroupModalOpen(true)
                                    }}
                                  >
                                    Edit
                                  </DropdownMenuItem>
                                  <DropdownMenuItem
                                    className="text-destructive"
                                    onClick={() => handleDeleteGroup(group.id)}
                                  >
                                    <Trash2 className="h-4 w-4 mr-2" />
                                    Delete
                                  </DropdownMenuItem>
                                </>
                              )}
                            </DropdownMenuContent>
                          </DropdownMenu>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Modals */}
      <CreateApiKeyModal
        open={createKeyOpen}
        onOpenChange={setCreateKeyOpen}
        onSubmit={handleCreateKey}
      />
      <ShowKeyModal open={showKeyOpen} onOpenChange={setShowKeyOpen} apiKey={newKey} />
      <EditApiKeyModal
        open={editKeyOpen}
        onOpenChange={(open) => {
          setEditKeyOpen(open)
          if (!open) setEditingKey(null)
        }}
        apiKey={editingKey}
        onSuccess={fetchApiKeys}
      />
      <ModelGroupFormModal
        open={groupModalOpen}
        onOpenChange={(open) => {
          setGroupModalOpen(open)
          if (!open) setEditingGroup(undefined)
        }}
        group={editingGroup}
        onSubmit={editingGroup ? handleUpdateGroup : handleCreateGroup}
      />
    </div>
  )
}