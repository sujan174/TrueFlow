"use client"

import { useState } from "react"
import { useRouter } from "next/navigation"
import Link from "next/link"
import { ArrowLeft, Search, Loader2, CheckCircle, XCircle, AlertCircle } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Badge } from "@/components/ui/badge"
import {
  discoverMcpServer,
  registerMcpServer,
  type DiscoveryResult,
} from "@/lib/api"

type Step = "discover" | "configure" | "registering" | "success" | "error"

export default function NewMcpServerPage() {
  const router = useRouter()
  const [step, setStep] = useState<Step>("discover")
  const [endpoint, setEndpoint] = useState("")
  const [name, setName] = useState("")
  const [apiKey, setApiKey] = useState("")
  const [clientId, setClientId] = useState("")
  const [clientSecret, setClientSecret] = useState("")
  const [discovery, setDiscovery] = useState<DiscoveryResult | null>(null)
  const [error, setError] = useState<string | null>(null)

  const handleDiscover = async () => {
    if (!endpoint.trim()) {
      setError("Please enter an endpoint URL")
      return
    }

    setError(null)
    setStep("discover")

    try {
      // Add https:// if no protocol specified
      let url = endpoint.trim()
      if (!url.startsWith("http://") && !url.startsWith("https://")) {
        url = "https://" + url
      }

      const result = await discoverMcpServer(url)
      setDiscovery(result)

      // Auto-fill name from server info
      if (result.server_info?.name) {
        setName(result.server_info.name.toLowerCase().replace(/[^a-z0-9-]/g, "-"))
      }

      setStep("configure")
    } catch (err) {
      setError(err instanceof Error ? err.message : "Discovery failed")
      setStep("error")
    }
  }

  const handleRegister = async () => {
    if (!discovery) return

    setError(null)
    setStep("registering")

    try {
      const request: {
        endpoint: string
        name?: string
        api_key?: string
        client_id?: string
        client_secret?: string
        auto_discover: boolean
      } = {
        endpoint: discovery.endpoint,
        name: name || undefined,
        auto_discover: true,
      }

      // Add auth credentials if OAuth or Bearer
      if (discovery.auth_type === "bearer" && apiKey) {
        request.api_key = apiKey
        request.auto_discover = false // Use manual mode for bearer
      } else if (discovery.auth_type === "oauth2") {
        if (clientId) request.client_id = clientId
        if (clientSecret) request.client_secret = clientSecret
      }

      await registerMcpServer(request)
      setStep("success")

      // Redirect after short delay
      setTimeout(() => {
        router.push("/mcp/servers")
      }, 1500)
    } catch (err) {
      setError(err instanceof Error ? err.message : "Registration failed")
      setStep("error")
    }
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background max-w-2xl">
        {/* Header */}
        <div className="flex items-center gap-4">
          <Link href="/mcp/servers">
            <Button variant="ghost" size="icon-sm">
              <ArrowLeft className="h-4 w-4" />
            </Button>
          </Link>
          <div className="flex flex-col gap-1">
            <h1 className="text-2xl font-bold tracking-tight">Add MCP Server</h1>
            <p className="text-sm text-muted-foreground">
              Register a new MCP server to enable tool integration
            </p>
          </div>
        </div>

        {/* Discovery Step */}
        <div className="bg-card border rounded-xl p-6 space-y-4">
          <div className="flex items-center gap-2">
            <Search className="h-4 w-4 text-muted-foreground" />
            <h2 className="font-semibold">Step 1: Discover Server</h2>
          </div>

          <div className="space-y-2">
            <Label htmlFor="endpoint">Endpoint URL</Label>
            <div className="flex gap-2">
              <Input
                id="endpoint"
                placeholder="https://mcp.example.com or mcp.example.com"
                value={endpoint}
                onChange={(e) => setEndpoint(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleDiscover()}
              />
              <Button onClick={handleDiscover} disabled={step === "discover" && !endpoint.trim()}>
                {step === "discover" ? "Discover" : "Retry"}
              </Button>
            </div>
          </div>
        </div>

        {/* Discovery Result */}
        {discovery && (
          <div className="bg-card border rounded-xl p-6 space-y-4">
            <div className="flex items-center gap-2">
              {discovery.requires_auth ? (
                <AlertCircle className="h-4 w-4 text-yellow-500" />
              ) : (
                <CheckCircle className="h-4 w-4 text-green-500" />
              )}
              <h2 className="font-semibold">Discovery Result</h2>
            </div>

            <div className="grid grid-cols-2 gap-4 text-sm">
              <div>
                <span className="text-muted-foreground">Server:</span>
                <span className="ml-2">{discovery.server_info?.name || "Unknown"}</span>
              </div>
              <div>
                <span className="text-muted-foreground">Version:</span>
                <span className="ml-2">{discovery.server_info?.version || "-"}</span>
              </div>
              <div>
                <span className="text-muted-foreground">Auth Type:</span>
                <Badge variant="outline" className="ml-2">{discovery.auth_type}</Badge>
              </div>
              <div>
                <span className="text-muted-foreground">Tools:</span>
                <span className="ml-2">{discovery.tool_count}</span>
              </div>
            </div>

            {discovery.tool_count > 0 && (
              <div className="space-y-2">
                <span className="text-sm text-muted-foreground">Available Tools:</span>
                <div className="flex flex-wrap gap-2">
                  {discovery.tools.slice(0, 10).map((tool) => (
                    <Badge key={tool.name} variant="secondary" className="text-xs">
                      {tool.name}
                    </Badge>
                  ))}
                  {discovery.tool_count > 10 && (
                    <Badge variant="outline" className="text-xs">
                      +{discovery.tool_count - 10} more
                    </Badge>
                  )}
                </div>
              </div>
            )}
          </div>
        )}

        {/* Configure Step */}
        {discovery && step !== "discover" && (
          <div className="bg-card border rounded-xl p-6 space-y-4">
            <h2 className="font-semibold">Step 2: Configure</h2>

            <div className="space-y-2">
              <Label htmlFor="name">Server Name</Label>
              <Input
                id="name"
                placeholder="my-mcp-server"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
              <p className="text-xs text-muted-foreground">
                Used to identify the server and prefix tools (e.g., mcp__my-server__tool)
              </p>
            </div>

            {discovery.auth_type === "bearer" && (
              <div className="space-y-2">
                <Label htmlFor="apiKey">API Key</Label>
                <Input
                  id="apiKey"
                  type="password"
                  placeholder="Enter the API key"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                />
              </div>
            )}

            {discovery.auth_type === "oauth2" && (
              <>
                <div className="space-y-2">
                  <Label htmlFor="clientId">OAuth Client ID</Label>
                  <Input
                    id="clientId"
                    placeholder="Enter OAuth client ID"
                    value={clientId}
                    onChange={(e) => setClientId(e.target.value)}
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="clientSecret">OAuth Client Secret</Label>
                  <Input
                    id="clientSecret"
                    type="password"
                    placeholder="Enter OAuth client secret"
                    value={clientSecret}
                    onChange={(e) => setClientSecret(e.target.value)}
                  />
                </div>
              </>
            )}

            <div className="flex gap-2 pt-4">
              <Button variant="outline" onClick={() => router.push("/mcp/servers")}>
                Cancel
              </Button>
              <Button onClick={handleRegister} disabled={step === "registering"}>
                {step === "registering" && <Loader2 className="h-4 w-4 mr-2 animate-spin" />}
                Register Server
              </Button>
            </div>
          </div>
        )}

        {/* Success State */}
        {step === "success" && (
          <div className="bg-green-500/10 border border-green-500/20 rounded-xl p-6 flex items-center gap-3">
            <CheckCircle className="h-5 w-5 text-green-500" />
            <span className="text-green-700 dark:text-green-400">
              Server registered successfully! Redirecting...
            </span>
          </div>
        )}

        {/* Error State */}
        {error && (
          <div className="bg-destructive/10 border border-destructive/20 rounded-xl p-6 flex items-center gap-3">
            <XCircle className="h-5 w-5 text-destructive" />
            <span className="text-destructive">{error}</span>
          </div>
        )}
      </div>
    </div>
  )
}