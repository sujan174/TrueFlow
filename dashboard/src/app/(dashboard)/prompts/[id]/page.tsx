"use client"

import { useEffect, useState, useCallback } from "react"
import { useParams, useRouter } from "next/navigation"
import dynamic from "next/dynamic"
import {
  Plus,
  Play,
  GitBranch,
  Clock,
  ArrowLeft,
  Save,
  Tag,
  ChevronDown,
  ChevronUp,
  MessageSquare,
  Send,
  Loader2,
  Code,
  History,
  TestTube,
} from "lucide-react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { toast } from "sonner"
import {
  getPrompt,
  createPromptVersion,
  deployPromptVersion,
  listCredentials,
  renderPrompt,
  type PromptDetailResponse,
  type PromptVersionRow,
  type CredentialMeta,
} from "@/lib/api"
import {
  LABEL_COLORS,
  DEPLOYMENT_LABELS,
  type DeploymentLabel,
  type Message,
  type Tool,
} from "@/lib/types/prompt"

// Dynamic import for Monaco editor (SSR disabled)
const MonacoEditor = dynamic(() => import("@monaco-editor/react").then(mod => mod.default), {
  ssr: false,
  loading: () => (
    <div className="h-[400px] bg-muted/30 rounded-lg flex items-center justify-center">
      <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
    </div>
  ),
})

// Dynamic import for diff viewer
const ReactDiffViewer = dynamic(() => import("react-diff-viewer-continued").then(mod => mod.default), {
  ssr: false,
  loading: () => (
    <div className="h-[300px] bg-muted/30 rounded-lg flex items-center justify-center">
      <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
    </div>
  ),
})

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

function LabelBadge({ label }: { label: string }) {
  const colors = LABEL_COLORS[label as DeploymentLabel] || {
    bg: "bg-muted",
    text: "text-muted-foreground",
    border: "border-border",
  }
  return (
    <Badge variant="outline" className={`${colors.bg} ${colors.text} ${colors.border}`}>
      {label}
    </Badge>
  )
}

// Editor Tab Component
function EditorTab({
  prompt,
  versions,
  onVersionCreated,
}: {
  prompt: PromptDetailResponse
  versions: PromptVersionRow[]
  onVersionCreated: () => void
}) {
  const [model, setModel] = useState("gpt-4o")
  const [messages, setMessages] = useState<Message[]>([
    { role: "system", content: "You are a helpful assistant." },
  ])
  const [temperature, setTemperature] = useState(1.0)
  const [maxTokens, setMaxTokens] = useState<number | undefined>()
  const [commitMessage, setCommitMessage] = useState("")
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [showAdvanced, setShowAdvanced] = useState(false)

  // Load latest version into editor
  useEffect(() => {
    if (versions.length > 0) {
      const latest = versions[0]
      setModel(latest.model)
      setMessages(latest.messages)
      setTemperature(latest.temperature ?? 1.0)
      setMaxTokens(latest.max_tokens ?? undefined)
    }
  }, [versions])

  const handleSubmit = async () => {
    if (!commitMessage.trim()) {
      toast.error("Please add a commit message")
      return
    }

    setIsSubmitting(true)
    try {
      await createPromptVersion(prompt.prompt.id, {
        model,
        messages,
        temperature,
        max_tokens: maxTokens,
        commit_message: commitMessage,
      })
      toast.success("New version created")
      setCommitMessage("")
      onVersionCreated()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to create version")
    } finally {
      setIsSubmitting(false)
    }
  }

  const messagesJson = JSON.stringify(messages, null, 2)

  const handleEditorChange = (value: string | undefined) => {
    if (!value) return
    try {
      const parsed = JSON.parse(value)
      if (Array.isArray(parsed)) {
        setMessages(parsed)
      }
    } catch {
      // Invalid JSON, keep current state
    }
  }

  return (
    <div className="space-y-4">
      {/* Model Selection */}
      <div className="grid grid-cols-3 gap-4">
        <div>
          <label className="text-sm font-medium">Model</label>
          <Select value={model} onValueChange={(v) => v && setModel(v)}>
            <SelectTrigger className="mt-1">
              <SelectValue placeholder="Select model" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="gpt-4o">GPT-4o</SelectItem>
              <SelectItem value="gpt-4o-mini">GPT-4o Mini</SelectItem>
              <SelectItem value="gpt-4-turbo">GPT-4 Turbo</SelectItem>
              <SelectItem value="claude-sonnet-4-20250514">Claude Sonnet 4</SelectItem>
              <SelectItem value="claude-3-5-sonnet-20241022">Claude 3.5 Sonnet</SelectItem>
              <SelectItem value="claude-3-haiku-20240307">Claude 3 Haiku</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div>
          <label className="text-sm font-medium">Temperature</label>
          <input
            type="number"
            step="0.1"
            min="0"
            max="2"
            value={temperature}
            onChange={(e) => setTemperature(parseFloat(e.target.value))}
            className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
          />
        </div>
        <div>
          <label className="text-sm font-medium">Max Tokens</label>
          <input
            type="number"
            value={maxTokens || ""}
            onChange={(e) => setMaxTokens(e.target.value ? parseInt(e.target.value) : undefined)}
            placeholder="Auto"
            className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
          />
        </div>
      </div>

      {/* Messages Editor */}
      <div>
        <label className="text-sm font-medium">Messages (JSON)</label>
        <p className="text-xs text-muted-foreground mb-2">
          Use {"{{variable}}"} syntax for template variables
        </p>
        <div className="border rounded-lg overflow-hidden">
          <MonacoEditor
            height="400px"
            language="json"
            theme="vs-dark"
            value={messagesJson}
            onChange={handleEditorChange}
            options={{
              minimap: { enabled: false },
              fontSize: 13,
              lineNumbers: "on",
              scrollBeyondLastLine: false,
              automaticLayout: true,
            }}
          />
        </div>
      </div>

      {/* Commit Message */}
      <div>
        <label className="text-sm font-medium">Commit Message *</label>
        <input
          type="text"
          value={commitMessage}
          onChange={(e) => setCommitMessage(e.target.value)}
          placeholder="Describe your changes..."
          className="w-full mt-1 px-3 py-2 text-sm border rounded-lg bg-background"
        />
      </div>

      {/* Submit */}
      <div className="flex justify-end gap-2">
        <Button onClick={handleSubmit} disabled={isSubmitting || !commitMessage.trim()}>
          <Save className="h-4 w-4 mr-2" />
          {isSubmitting ? "Creating..." : "Publish Version"}
        </Button>
      </div>
    </div>
  )
}

// Version History Tab
function VersionsTab({
  versions,
  onDeploy,
}: {
  versions: PromptVersionRow[]
  onDeploy: () => void
}) {
  const [diffVersions, setDiffVersions] = useState<[number, number] | null>(null)
  const [deployModal, setDeployModal] = useState<{ version: number; label: string } | null>(null)

  const handleDeploy = async () => {
    if (!deployModal) return
    try {
      await deployPromptVersion(versions[0].prompt_id, {
        version: deployModal.version,
        label: deployModal.label,
      })
      toast.success(`Version ${deployModal.version} deployed to ${deployModal.label}`)
      setDeployModal(null)
      onDeploy()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to deploy")
    }
  }

  return (
    <div className="space-y-4">
      {/* Version List */}
      <div className="border rounded-lg divide-y">
        {versions.map((v, idx) => (
          <div key={v.id} className="p-4 hover:bg-muted/30 transition-colors">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className="flex items-center gap-2">
                  <GitBranch className="h-4 w-4 text-muted-foreground" />
                  <span className="font-medium">v{v.version}</span>
                </div>
                <code className="text-xs bg-muted px-2 py-0.5 rounded">{v.model}</code>
                <div className="flex gap-1">
                  {v.labels.map((label) => (
                    <LabelBadge key={label} label={label} />
                  ))}
                </div>
              </div>
              <div className="flex items-center gap-3">
                <span className="text-xs text-muted-foreground flex items-center gap-1">
                  <Clock className="h-3 w-3" />
                  {formatRelativeTime(v.created_at)}
                </span>
                <div className="flex gap-1">
                  {idx < versions.length - 1 && (
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => setDiffVersions([v.version, versions[idx + 1].version])}
                    >
                      Compare
                    </Button>
                  )}
                  <Select
                    value=""
                    onValueChange={(label) => label && setDeployModal({ version: v.version, label })}
                  >
                    <SelectTrigger className="w-[140px] h-8">
                      <SelectValue placeholder="Deploy to..." />
                    </SelectTrigger>
                    <SelectContent>
                      {DEPLOYMENT_LABELS.map((label) => (
                        <SelectItem key={label} value={label}>
                          Deploy to {label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              </div>
            </div>
            {v.commit_message && (
              <p className="text-sm text-muted-foreground mt-2">{v.commit_message}</p>
            )}
          </div>
        ))}
      </div>

      {/* Diff Dialog */}
      <Dialog open={!!diffVersions} onOpenChange={() => setDiffVersions(null)}>
        <DialogContent className="max-w-4xl max-h-[80vh] overflow-hidden">
          <DialogHeader>
            <DialogTitle>
              Comparing v{diffVersions?.[0]} vs v{diffVersions?.[1]}
            </DialogTitle>
          </DialogHeader>
          <div className="overflow-auto">
            {diffVersions && (
              <ReactDiffViewer
                oldValue={JSON.stringify(
                  versions.find((v) => v.version === diffVersions[0])?.messages,
                  null,
                  2
                )}
                newValue={JSON.stringify(
                  versions.find((v) => v.version === diffVersions[1])?.messages,
                  null,
                  2
                )}
                splitView
                showDiffOnly={false}
                styles={{
                  contentText: { fontSize: 12, fontFamily: "monospace" },
                }}
              />
            )}
          </div>
        </DialogContent>
      </Dialog>

      {/* Deploy Confirmation */}
      <Dialog open={!!deployModal} onOpenChange={() => setDeployModal(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Deploy Version</DialogTitle>
            <DialogDescription>
              Deploy version {deployModal?.version} to{" "}
              <Badge variant="secondary">{deployModal?.label}</Badge>?
              <br />
              This will move the label from any other version.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeployModal(null)}>
              Cancel
            </Button>
            <Button onClick={handleDeploy}>Deploy</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}

// Playground Tab
function PlaygroundTab({
  prompt,
  versions,
}: {
  prompt: PromptDetailResponse
  versions: PromptVersionRow[]
}) {
  const [selectedVersion, setSelectedVersion] = useState<number | "latest">("latest")
  const [selectedLabel, setSelectedLabel] = useState<string>("")
  const [variables, setVariables] = useState<Record<string, string>>({})
  const [renderedPrompt, setRenderedPrompt] = useState<Message[] | null>(null)
  const [isRendering, setIsRendering] = useState(false)
  const [credentials, setCredentials] = useState<CredentialMeta[]>([])
  const [selectedCredential, setSelectedCredential] = useState<string>("")

  // Extract variables from messages
  const extractVariables = (messages: Message[]): string[] => {
    const vars = new Set<string>()
    const regex = /\{\{(\w+)\}\}/g
    messages.forEach((msg) => {
      const content = typeof msg.content === "string" ? msg.content : JSON.stringify(msg.content)
      let match
      while ((match = regex.exec(content)) !== null) {
        vars.add(match[1])
      }
    })
    return Array.from(vars)
  }

  const currentVersion = selectedVersion === "latest"
    ? versions[0]
    : versions.find((v) => v.version === selectedVersion)

  const detectedVars = currentVersion ? extractVariables(currentVersion.messages) : []

  useEffect(() => {
    listCredentials().then(setCredentials).catch(console.error)
  }, [])

  const handleRender = async () => {
    setIsRendering(true)
    try {
      const response = await renderPrompt(prompt.prompt.slug, {
        variables,
        label: selectedLabel || undefined,
        version: selectedVersion === "latest" ? undefined : selectedVersion,
      })
      setRenderedPrompt(response.messages)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to render prompt")
    } finally {
      setIsRendering(false)
    }
  }

  return (
    <div className="grid grid-cols-2 gap-6">
      {/* Left: Configuration */}
      <div className="space-y-4">
        <div>
          <label className="text-sm font-medium">Version</label>
          <Select
            value={selectedVersion.toString()}
            onValueChange={(v) => v && setSelectedVersion(v === "latest" ? "latest" : parseInt(v))}
          >
            <SelectTrigger className="mt-1">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="latest">Latest</SelectItem>
              {versions.map((v) => (
                <SelectItem key={v.id} value={v.version.toString()}>
                  v{v.version}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div>
          <label className="text-sm font-medium">Label (optional)</label>
          <Select value={selectedLabel} onValueChange={(v) => v !== null && setSelectedLabel(v)}>
            <SelectTrigger className="mt-1">
              <SelectValue placeholder="Any" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="">Any</SelectItem>
              {DEPLOYMENT_LABELS.map((label) => (
                <SelectItem key={label} value={label}>
                  {label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div>
          <label className="text-sm font-medium">Credential (for live testing)</label>
          <Select value={selectedCredential} onValueChange={(v) => v !== null && setSelectedCredential(v)}>
            <SelectTrigger className="mt-1">
              <SelectValue placeholder="Select a credential" />
            </SelectTrigger>
            <SelectContent>
              {credentials.map((cred) => (
                <SelectItem key={cred.id} value={cred.id}>
                  {cred.name} ({cred.provider})
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        {/* Variables */}
        {detectedVars.length > 0 && (
          <div>
            <label className="text-sm font-medium">Variables</label>
            <div className="mt-2 space-y-2">
              {detectedVars.map((v) => (
                <div key={v} className="flex items-center gap-2">
                  <code className="text-xs bg-muted px-2 py-1 rounded w-24">{`{{${v}}}`}</code>
                  <input
                    type="text"
                    value={variables[v] || ""}
                    onChange={(e) => setVariables({ ...variables, [v]: e.target.value })}
                    className="flex-1 px-2 py-1 text-sm border rounded bg-background"
                    placeholder={`Value for ${v}`}
                  />
                </div>
              ))}
            </div>
          </div>
        )}

        <Button onClick={handleRender} disabled={isRendering} className="w-full">
          <Play className="h-4 w-4 mr-2" />
          {isRendering ? "Rendering..." : "Render Preview"}
        </Button>
      </div>

      {/* Right: Preview */}
      <div>
        <label className="text-sm font-medium">Rendered Output</label>
        <div className="mt-1 border rounded-lg bg-muted/30 h-[400px] overflow-auto">
          {renderedPrompt ? (
            <div className="p-4 space-y-3">
              {renderedPrompt.map((msg, idx) => (
                <div key={idx} className="flex gap-2">
                  <Badge variant="outline" className="shrink-0">
                    {msg.role}
                  </Badge>
                  <div className="text-sm whitespace-pre-wrap">{msg.content}</div>
                </div>
              ))}
            </div>
          ) : (
            <div className="h-full flex items-center justify-center text-muted-foreground">
              <div className="text-center">
                <MessageSquare className="h-8 w-8 mx-auto mb-2 opacity-50" />
                <p>Click "Render Preview" to see output</p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

export default function PromptDetailPage() {
  const params = useParams()
  const router = useRouter()
  const id = params.id as string

  const [prompt, setPrompt] = useState<PromptDetailResponse | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [activeTab, setActiveTab] = useState("editor")

  const fetchPrompt = useCallback(async () => {
    try {
      const data = await getPrompt(id)
      setPrompt(data)
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load prompt")
    } finally {
      setLoading(false)
    }
  }, [id])

  useEffect(() => {
    fetchPrompt()
  }, [fetchPrompt])

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    )
  }

  if (error || !prompt) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-center">
          <p className="text-destructive">{error || "Prompt not found"}</p>
          <Button variant="outline" className="mt-4" onClick={() => router.push("/prompts")}>
            <ArrowLeft className="h-4 w-4 mr-2" />
            Back to Prompts
          </Button>
        </div>
      </div>
    )
  }

  const { prompt: p, versions } = prompt
  const latestLabels = versions[0]?.labels || []

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-4">
            <Button variant="ghost" size="icon" onClick={() => router.push("/prompts")}>
              <ArrowLeft className="h-4 w-4" />
            </Button>
            <div>
              <div className="flex items-center gap-3">
                <h1 className="text-2xl font-bold tracking-tight">{p.name}</h1>
                {latestLabels.map((label) => (
                  <LabelBadge key={label} label={label} />
                ))}
              </div>
              <p className="text-sm text-muted-foreground flex items-center gap-2 mt-1">
                <code className="bg-muted px-2 py-0.5 rounded">{p.slug}</code>
                <span>•</span>
                <GitBranch className="h-3 w-3" />
                {versions.length} versions
              </p>
            </div>
          </div>
        </div>

        {p.description && (
          <p className="text-muted-foreground -mt-2">{p.description}</p>
        )}

        {/* Tabs */}
        <Tabs value={activeTab} onValueChange={setActiveTab} className="flex-1">
          <TabsList>
            <TabsTrigger value="editor" className="gap-2">
              <Code className="h-4 w-4" />
              Editor
            </TabsTrigger>
            <TabsTrigger value="versions" className="gap-2">
              <History className="h-4 w-4" />
              Versions
            </TabsTrigger>
            <TabsTrigger value="playground" className="gap-2">
              <TestTube className="h-4 w-4" />
              Playground
            </TabsTrigger>
          </TabsList>

          <TabsContent value="editor" className="mt-4">
            <EditorTab
              prompt={prompt}
              versions={versions}
              onVersionCreated={fetchPrompt}
            />
          </TabsContent>

          <TabsContent value="versions" className="mt-4">
            <VersionsTab versions={versions} onDeploy={fetchPrompt} />
          </TabsContent>

          <TabsContent value="playground" className="mt-4">
            <PlaygroundTab prompt={prompt} versions={versions} />
          </TabsContent>
        </Tabs>
      </div>
    </div>
  )
}