"use client";

import { useState } from "react";
import useSWR, { type KeyedMutator } from "swr";
import {
    createService,
    deleteService,
    listCredentials,
    Service,
    Credential,
    McpServerInfo,
    McpDiscoveryResult,
    registerMcpServer,
    deleteMcpServer as deleteMcpServerApi,
    refreshMcpServer,
    discoverMcpServer,
    reauthMcpServer,
    swrFetcher,
} from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
    DialogTrigger,
} from "@/components/ui/dialog";
import { Plus, Trash2, Plug, Globe, Loader2, RefreshCw, Wrench, Server, ChevronDown, ChevronRight, Search, ShieldCheck, Key, KeyRound } from "lucide-react";
import { toast } from "sonner";
import { formatDistanceToNow } from "date-fns";
import { cn } from "@/lib/utils";

const Select = ({ className, children, ...props }: React.SelectHTMLAttributes<HTMLSelectElement>) => (
    <select
        className={`flex h-9 w-full items-center rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors focus:outline-none focus:ring-1 focus:ring-ring disabled:cursor-not-allowed disabled:opacity-50 ${className || ""}`}
        {...props}
    >
        {children}
    </select>
);

type Tab = "services" | "mcp";

export default function ToolsPage() {
    const [activeTab, setActiveTab] = useState<Tab>("mcp");
    const { data: services = [], mutate: mutateServices, isLoading: servicesLoading } = useSWR<Service[]>("/services", swrFetcher);
    const { data: credentials = [], isLoading: credsLoading } = useSWR<Credential[]>("/credentials", swrFetcher);
    const { data: mcpServers = [], mutate: mutateMcp, isLoading: mcpLoading } = useSWR<McpServerInfo[]>("/mcp/servers", swrFetcher, { refreshInterval: 10000 });

    const loading = servicesLoading || credsLoading || mcpLoading;

    return (
        <div className="space-y-4">
            {/* Tabs */}
            <div className="flex items-center gap-1 border-b border-border">
                <button
                    onClick={() => setActiveTab("mcp")}
                    className={cn(
                        "px-3 py-2 text-sm font-medium transition-colors border-b-2 -mb-px",
                        activeTab === "mcp"
                            ? "text-foreground border-[var(--primary)]"
                            : "text-muted-foreground border-transparent hover:text-foreground"
                    )}
                >
                    <Server className="inline h-3.5 w-3.5 mr-1.5 -mt-0.5" />
                    MCP Servers
                    {mcpServers.length > 0 && (
                        <span className="ml-1.5 text-[10px] font-mono bg-[var(--primary)]/10 text-[var(--primary)] px-1.5 py-0.5 rounded-full">
                            {mcpServers.length}
                        </span>
                    )}
                </button>
                <button
                    onClick={() => setActiveTab("services")}
                    className={cn(
                        "px-3 py-2 text-sm font-medium transition-colors border-b-2 -mb-px",
                        activeTab === "services"
                            ? "text-foreground border-[var(--primary)]"
                            : "text-muted-foreground border-transparent hover:text-foreground"
                    )}
                >
                    <Plug className="inline h-3.5 w-3.5 mr-1.5 -mt-0.5" />
                    Services
                    {services.length > 0 && (
                        <span className="ml-1.5 text-[10px] font-mono text-muted-foreground">{services.length}</span>
                    )}
                </button>
            </div>

            {loading ? (
                <div className="flex items-center justify-center py-32">
                    <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
                </div>
            ) : activeTab === "mcp" ? (
                <McpTab servers={mcpServers} mutateMcp={mutateMcp} />
            ) : (
                <ServicesTab
                    services={services}
                    credentials={credentials}
                    mutateServices={mutateServices}
                />
            )}
        </div>
    );
}

// ── Auth badge helper ──────────────────────────────────────────

function AuthBadge({ authType }: { authType: string }) {
    switch (authType) {
        case "oauth2":
            return (
                <Badge variant="secondary" className="text-[9px] h-4 gap-0.5 bg-blue-500/10 text-blue-500 border-blue-500/20">
                    <ShieldCheck className="h-2.5 w-2.5" />
                    OAuth 2.0
                </Badge>
            );
        case "bearer":
            return (
                <Badge variant="secondary" className="text-[9px] h-4 gap-0.5 bg-amber-500/10 text-amber-500 border-amber-500/20">
                    <Key className="h-2.5 w-2.5" />
                    API Key
                </Badge>
            );
        default:
            return (
                <Badge variant="secondary" className="text-[9px] h-4 gap-0.5 text-muted-foreground">
                    No Auth
                </Badge>
            );
    }
}

// ── MCP Servers Tab ────────────────────────────────────────────

function McpTab({
    servers,
    mutateMcp,
}: {
    servers: McpServerInfo[];
    mutateMcp: () => void;
}) {
    const [dialogOpen, setDialogOpen] = useState(false);
    const [endpoint, setEndpoint] = useState("");
    const [name, setName] = useState("");
    const [apiKey, setApiKey] = useState("");
    // OAuth fields
    const [clientId, setClientId] = useState("");
    const [clientSecret, setClientSecret] = useState("");
    // Mode
    const [autoDiscover, setAutoDiscover] = useState(true);
    // Discovery state
    const [discovering, setDiscovering] = useState(false);
    const [discoveryResult, setDiscoveryResult] = useState<McpDiscoveryResult | null>(null);
    const [creating, setCreating] = useState(false);
    const [expandedServer, setExpandedServer] = useState<string | null>(null);

    const handleDiscover = async () => {
        if (!endpoint) {
            toast.error("Endpoint URL is required");
            return;
        }
        setDiscovering(true);
        setDiscoveryResult(null);
        try {
            const result = await discoverMcpServer(endpoint);
            setDiscoveryResult(result);
            if (result.requires_auth) {
                toast.info(`Server requires OAuth 2.0 — enter client credentials to continue`);
            } else {
                toast.success(`Discovered ${result.tool_count} tools — ready to register`);
            }
        } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : "Unknown error";
            toast.error(`Discovery failed: ${msg}`);
        } finally {
            setDiscovering(false);
        }
    };

    const handleRegister = async () => {
        if (!endpoint) {
            toast.error("Endpoint URL is required");
            return;
        }
        if (!autoDiscover && !name) {
            toast.error("Name is required for manual registration");
            return;
        }
        setCreating(true);
        try {
            const result = await registerMcpServer({
                endpoint,
                name: name ? name.toLowerCase().replace(/\s+/g, "-") : undefined,
                api_key: apiKey || undefined,
                client_id: clientId || undefined,
                client_secret: clientSecret || undefined,
                auto_discover: autoDiscover,
            });
            toast.success(`MCP server "${result.name}" registered — ${result.tool_count} tools discovered`);
            setDialogOpen(false);
            resetForm();
            mutateMcp();
        } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : "Unknown error";
            toast.error(`Failed to register: ${msg}`);
        } finally {
            setCreating(false);
        }
    };

    const resetForm = () => {
        setEndpoint("");
        setName("");
        setApiKey("");
        setClientId("");
        setClientSecret("");
        setDiscoveryResult(null);
        setAutoDiscover(true);
    };

    const handleDelete = async (id: string, srvName: string) => {
        if (!confirm(`Remove MCP server "${srvName}"? Tools from this server will no longer be available.`)) return;
        try {
            await deleteMcpServerApi(id);
            toast.success(`Removed "${srvName}"`);
            mutateMcp();
        } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : "Unknown error";
            toast.error(`Failed to remove: ${msg}`);
        }
    };

    const handleRefresh = async (id: string, srvName: string) => {
        try {
            const tools = await refreshMcpServer(id);
            toast.success(`Refreshed "${srvName}" — ${tools.length} tools`);
            mutateMcp();
        } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : "Unknown error";
            toast.error(`Failed to refresh: ${msg}`);
        }
    };

    const handleReauth = async (id: string, srvName: string) => {
        try {
            const result = await reauthMcpServer(id);
            if (result.success) {
                toast.success(`Re-authenticated "${srvName}"`);
            } else {
                toast.error(`Re-auth failed: ${result.error || "Unknown error"}`);
            }
            mutateMcp();
        } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : "Unknown error";
            toast.error(`Re-auth failed: ${msg}`);
        }
    };

    return (
        <div className="space-y-4">
            {/* Controls */}
            <div className="flex items-center justify-between">
                <div>
                    <p className="text-xs text-muted-foreground">
                        Register MCP servers to auto-discover tools for your AI agents.
                        Use <code className="text-[10px] bg-muted px-1 py-0.5 rounded">X-MCP-Servers: name</code> header to activate.
                    </p>
                </div>
                <Dialog open={dialogOpen} onOpenChange={(open) => { setDialogOpen(open); if (!open) resetForm(); }}>
                    <DialogTrigger asChild>
                        <Button className="gap-2 ml-4 shrink-0" size="sm">
                            <Plus className="h-3.5 w-3.5" />
                            Add MCP Server
                        </Button>
                    </DialogTrigger>
                    <DialogContent className="sm:max-w-[520px]">
                        <DialogHeader>
                            <DialogTitle>Register MCP Server</DialogTitle>
                        </DialogHeader>
                        <div className="space-y-4 pt-2">
                            {/* Mode Toggle */}
                            <div className="flex items-center gap-2 text-xs">
                                <button
                                    onClick={() => setAutoDiscover(true)}
                                    className={cn(
                                        "px-3 py-1 rounded-md transition-colors border",
                                        autoDiscover
                                            ? "bg-[var(--primary)]/10 text-[var(--primary)] border-[var(--primary)]/20"
                                            : "border-border text-muted-foreground hover:text-foreground"
                                    )}
                                >
                                    <Search className="inline h-3 w-3 mr-1 -mt-0.5" />
                                    Auto-Discover
                                </button>
                                <button
                                    onClick={() => setAutoDiscover(false)}
                                    className={cn(
                                        "px-3 py-1 rounded-md transition-colors border",
                                        !autoDiscover
                                            ? "bg-[var(--primary)]/10 text-[var(--primary)] border-[var(--primary)]/20"
                                            : "border-border text-muted-foreground hover:text-foreground"
                                    )}
                                >
                                    <KeyRound className="inline h-3 w-3 mr-1 -mt-0.5" />
                                    Manual
                                </button>
                            </div>

                            {/* Endpoint URL */}
                            <div className="space-y-2">
                                <Label htmlFor="mcp-endpoint">Endpoint URL</Label>
                                <div className="flex gap-2">
                                    <Input
                                        id="mcp-endpoint"
                                        placeholder="http://localhost:3001/mcp"
                                        value={endpoint}
                                        onChange={(e) => { setEndpoint(e.target.value); setDiscoveryResult(null); }}
                                        className="flex-1"
                                    />
                                    {autoDiscover && (
                                        <Button
                                            variant="outline"
                                            size="sm"
                                            onClick={handleDiscover}
                                            disabled={discovering || !endpoint}
                                            className="shrink-0"
                                        >
                                            {discovering ? (
                                                <Loader2 className="h-3.5 w-3.5 animate-spin" />
                                            ) : (
                                                <>
                                                    <Search className="h-3.5 w-3.5 mr-1" />
                                                    Discover
                                                </>
                                            )}
                                        </Button>
                                    )}
                                </div>
                                <p className="text-[10px] text-muted-foreground">
                                    Streamable HTTP endpoint (JSON-RPC 2.0 over HTTP POST)
                                </p>
                            </div>

                            {/* Discovery Result Preview */}
                            {discoveryResult && (
                                <div className="rounded-md border border-border bg-muted/30 p-3 space-y-2">
                                    <div className="flex items-center justify-between">
                                        <span className="text-xs font-medium">Discovery Result</span>
                                        <AuthBadge authType={discoveryResult.auth_type} />
                                    </div>
                                    {discoveryResult.server_info && (
                                        <p className="text-[11px] text-muted-foreground">
                                            {discoveryResult.server_info.name} v{discoveryResult.server_info.version}
                                        </p>
                                    )}
                                    <p className="text-[11px] text-muted-foreground">
                                        {discoveryResult.tool_count} tools discovered
                                    </p>
                                    {discoveryResult.tool_count > 0 && (
                                        <div className="flex flex-wrap gap-1 mt-1">
                                            {discoveryResult.tools.slice(0, 8).map((t) => (
                                                <span key={t.name} className="text-[9px] font-mono bg-muted px-1.5 py-0.5 rounded">
                                                    {t.name}
                                                </span>
                                            ))}
                                            {discoveryResult.tools.length > 8 && (
                                                <span className="text-[9px] text-muted-foreground">
                                                    +{discoveryResult.tools.length - 8} more
                                                </span>
                                            )}
                                        </div>
                                    )}
                                    {discoveryResult.requires_auth && discoveryResult.token_endpoint && (
                                        <p className="text-[10px] text-amber-500 mt-1">
                                            ⚠ OAuth 2.0 required — Token endpoint: <code className="bg-muted px-1 rounded">{discoveryResult.token_endpoint}</code>
                                        </p>
                                    )}
                                </div>
                            )}

                            {/* Server Name (required for manual, optional for auto) */}
                            <div className="space-y-2">
                                <Label htmlFor="mcp-name">
                                    Server Name {autoDiscover && <span className="text-muted-foreground">(optional — auto-derived)</span>}
                                </Label>
                                <Input
                                    id="mcp-name"
                                    placeholder={autoDiscover ? "Auto-derived from server info" : "e.g. brave-search, filesystem, slack"}
                                    value={name}
                                    onChange={(e) => setName(e.target.value)}
                                />
                                {!autoDiscover && (
                                    <p className="text-[10px] text-muted-foreground">
                                        Alphanumeric + hyphens. Used in namespacing: <code>mcp__name__tool</code>
                                    </p>
                                )}
                            </div>

                            {/* Auth Fields — show based on mode or discovery result */}
                            {!autoDiscover && (
                                <div className="space-y-2">
                                    <Label htmlFor="mcp-key">API Key <span className="text-muted-foreground">(optional)</span></Label>
                                    <Input
                                        id="mcp-key"
                                        type="password"
                                        placeholder="Bearer token for authenticated servers"
                                        value={apiKey}
                                        onChange={(e) => setApiKey(e.target.value)}
                                    />
                                </div>
                            )}

                            {/* OAuth 2.0 Credentials — shown when discovery says OAuth is needed */}
                            {autoDiscover && discoveryResult?.requires_auth && (
                                <div className="space-y-3 rounded-md border border-blue-500/20 bg-blue-500/5 p-3">
                                    <div className="flex items-center gap-2">
                                        <ShieldCheck className="h-3.5 w-3.5 text-blue-500" />
                                        <span className="text-xs font-medium text-blue-500">OAuth 2.0 Credentials</span>
                                    </div>
                                    <div className="space-y-2">
                                        <Label htmlFor="mcp-client-id" className="text-xs">Client ID</Label>
                                        <Input
                                            id="mcp-client-id"
                                            placeholder="Your OAuth client_id"
                                            value={clientId}
                                            onChange={(e) => setClientId(e.target.value)}
                                        />
                                    </div>
                                    <div className="space-y-2">
                                        <Label htmlFor="mcp-client-secret" className="text-xs">Client Secret</Label>
                                        <Input
                                            id="mcp-client-secret"
                                            type="password"
                                            placeholder="Your OAuth client_secret"
                                            value={clientSecret}
                                            onChange={(e) => setClientSecret(e.target.value)}
                                        />
                                    </div>
                                </div>
                            )}

                            <Button onClick={handleRegister} disabled={creating} className="w-full">
                                {creating ? (
                                    <><Loader2 className="h-4 w-4 mr-2 animate-spin" /> Connecting…</>
                                ) : autoDiscover ? (
                                    "Register & Auto-Discover Tools"
                                ) : (
                                    "Register & Discover Tools"
                                )}
                            </Button>
                        </div>
                    </DialogContent>
                </Dialog>
            </div>

            {/* Server List */}
            {servers.length === 0 ? (
                <Card>
                    <CardContent className="py-16 text-center">
                        <Server className="h-10 w-10 mx-auto text-muted-foreground/20 mb-4" />
                        <h3 className="text-base font-medium">No MCP servers registered</h3>
                        <p className="text-xs text-muted-foreground mt-1 max-w-sm mx-auto">
                            Add an MCP server to auto-discover tools. The gateway will inject them into LLM requests and execute tool calls automatically.
                        </p>
                    </CardContent>
                </Card>
            ) : (
                <div className="space-y-2">
                    {servers.map((srv) => {
                        const isExpanded = expandedServer === srv.id;
                        const isConnected = srv.status === "Connected";
                        const isOAuth = srv.auth_type === "oauth2";
                        return (
                            <Card key={srv.id} className="group">
                                <div
                                    className="px-4 py-3 flex items-center gap-3 cursor-pointer hover:bg-card/80 transition-colors"
                                    onClick={() => setExpandedServer(isExpanded ? null : srv.id)}
                                >
                                    {isExpanded ? (
                                        <ChevronDown className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                                    ) : (
                                        <ChevronRight className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                                    )}

                                    <div className={cn(
                                        "h-2 w-2 rounded-full shrink-0",
                                        isConnected ? "bg-emerald-500" : "bg-rose-500"
                                    )} />

                                    <div className="flex-1 min-w-0">
                                        <div className="flex items-center gap-2">
                                            <span className="font-medium text-sm">{srv.name}</span>
                                            <Badge variant="secondary" className="text-[9px] h-4">
                                                {srv.tool_count} tools
                                            </Badge>
                                            <AuthBadge authType={srv.auth_type} />
                                            {srv.server_info && (
                                                <span className="text-[10px] text-muted-foreground/50 font-mono">
                                                    {srv.server_info.name} v{srv.server_info.version}
                                                </span>
                                            )}
                                        </div>
                                        <p className="text-[11px] text-muted-foreground font-mono truncate mt-0.5">
                                            {srv.endpoint}
                                        </p>
                                    </div>

                                    <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity" onClick={(e) => e.stopPropagation()}>
                                        {isOAuth && (
                                            <Button
                                                variant="ghost"
                                                size="icon"
                                                className="h-7 w-7"
                                                title="Re-authenticate OAuth token"
                                                onClick={() => handleReauth(srv.id, srv.name)}
                                            >
                                                <ShieldCheck className="h-3 w-3" />
                                            </Button>
                                        )}
                                        <Button
                                            variant="ghost"
                                            size="icon"
                                            className="h-7 w-7"
                                            title="Refresh tools"
                                            onClick={() => handleRefresh(srv.id, srv.name)}
                                        >
                                            <RefreshCw className="h-3 w-3" />
                                        </Button>
                                        <Button
                                            variant="ghost"
                                            size="icon"
                                            className="h-7 w-7 text-destructive"
                                            title="Remove server"
                                            onClick={() => handleDelete(srv.id, srv.name)}
                                        >
                                            <Trash2 className="h-3 w-3" />
                                        </Button>
                                    </div>
                                </div>

                                {isExpanded && (
                                    <div className="px-4 pb-3 border-t border-border pt-3">
                                        <div className="flex items-center gap-2 mb-2">
                                            <Wrench className="h-3 w-3 text-muted-foreground" />
                                            <span className="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">
                                                Discovered Tools
                                            </span>
                                        </div>
                                        {srv.tools.length === 0 ? (
                                            <p className="text-xs text-muted-foreground">No tools discovered</p>
                                        ) : (
                                            <div className="grid gap-1">
                                                {srv.tools.map((tool) => (
                                                    <div
                                                        key={tool}
                                                        className="flex items-center gap-2 py-1 px-2 rounded bg-muted/30 text-xs"
                                                    >
                                                        <span className="font-mono text-[11px] text-foreground/70">
                                                            mcp__{srv.name}__{tool}
                                                        </span>
                                                    </div>
                                                ))}
                                            </div>
                                        )}
                                        <div className="mt-3 pt-2 border-t border-border/40">
                                            <p className="text-[10px] text-muted-foreground">
                                                Usage: <code className="bg-muted px-1 py-0.5 rounded text-[9px]">
                                                    curl -H &quot;X-MCP-Servers: {srv.name}&quot; …
                                                </code>
                                            </p>
                                            <p className="text-[10px] text-muted-foreground/60 mt-1">
                                                Last refreshed {srv.last_refreshed_secs_ago}s ago
                                            </p>
                                        </div>
                                    </div>
                                )}
                            </Card>
                        );
                    })}
                </div>
            )}
        </div>
    );
}

// ── Services Tab ──────────────────────────────────────────────

function ServicesTab({
    services,
    credentials,
    mutateServices,
}: {
    services: Service[];
    credentials: Credential[];
    mutateServices: KeyedMutator<Service[]>;
}) {
    const [dialogOpen, setDialogOpen] = useState(false);
    const [name, setName] = useState("");
    const [description, setDescription] = useState("");
    const [baseUrl, setBaseUrl] = useState("");
    const [serviceType, setServiceType] = useState("generic");
    const [credentialId, setCredentialId] = useState("");
    const [creating, setCreating] = useState(false);

    const handleCreate = async () => {
        if (!name || !baseUrl) {
            toast.error("Name and Base URL are required");
            return;
        }
        setCreating(true);
        const newService: Service = {
            id: "temp-" + Date.now(),
            project_id: "",
            name: name.toLowerCase().replace(/\s+/g, "-"),
            description,
            base_url: baseUrl,
            service_type: serviceType,
            credential_id: credentialId || null,
            is_active: true,
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
        };
        try {
            await mutateServices(
                async () => {
                    const created = await createService({
                        name: newService.name,
                        description,
                        base_url: baseUrl,
                        service_type: serviceType,
                        credential_id: credentialId || undefined,
                    });
                    return [...services, created];
                },
                { optimisticData: [...services, newService], rollbackOnError: true, revalidate: true }
            );
            toast.success(`Service "${name}" registered`);
            setDialogOpen(false);
            setName(""); setDescription(""); setBaseUrl(""); setServiceType("generic"); setCredentialId("");
        } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : "Unknown error";
            toast.error(`Failed to create service: ${msg}`);
        } finally {
            setCreating(false);
        }
    };

    const handleDelete = async (id: string, svcName: string) => {
        if (!confirm(`Delete service "${svcName}"?`)) return;
        try {
            await mutateServices(
                async () => {
                    await deleteService(id);
                    return services.filter(s => s.id !== id);
                },
                { optimisticData: services.filter(s => s.id !== id), rollbackOnError: true, revalidate: true }
            );
            toast.success(`Deleted "${svcName}"`);
        } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : "Unknown error";
            toast.error(`Failed to delete: ${msg}`);
        }
    };

    const getCredentialName = (credId: string | null) => {
        if (!credId) return "None";
        const cred = credentials.find((c) => c.id === credId);
        return cred ? cred.name : credId.slice(0, 8) + "…";
    };

    return (
        <div className="space-y-4">
            <div className="flex items-center justify-between">
                <p className="text-xs text-muted-foreground">
                    Register external APIs for secure credential-injected proxying.
                </p>
                <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
                    <DialogTrigger asChild>
                        <Button className="gap-2" size="sm"><Plus className="h-3.5 w-3.5" /> Add Service</Button>
                    </DialogTrigger>
                    <DialogContent className="sm:max-w-[480px]">
                        <DialogHeader><DialogTitle>Register Service</DialogTitle></DialogHeader>
                        <div className="space-y-4 pt-2">
                            <div className="space-y-2">
                                <Label>Service Name</Label>
                                <Input placeholder="e.g. stripe, slack" value={name} onChange={e => setName(e.target.value)} />
                            </div>
                            <div className="space-y-2">
                                <Label>Description</Label>
                                <Input placeholder="Optional" value={description} onChange={e => setDescription(e.target.value)} />
                            </div>
                            <div className="space-y-2">
                                <Label>Base URL</Label>
                                <Input placeholder="https://api.stripe.com" value={baseUrl} onChange={e => setBaseUrl(e.target.value)} />
                            </div>
                            <div className="space-y-2">
                                <Label>Type</Label>
                                <Select value={serviceType} onChange={e => setServiceType(e.target.value)}>
                                    <option value="generic">Generic API</option>
                                    <option value="llm">LLM Provider</option>
                                </Select>
                            </div>
                            <div className="space-y-2">
                                <Label>Credential</Label>
                                <Select value={credentialId} onChange={e => setCredentialId(e.target.value)}>
                                    <option value="">Select…</option>
                                    {credentials.map(c => (
                                        <option key={c.id} value={c.id}>{c.name} ({c.provider})</option>
                                    ))}
                                </Select>
                            </div>
                            <Button onClick={handleCreate} disabled={creating} className="w-full">
                                {creating ? <><Loader2 className="h-4 w-4 mr-2 animate-spin" /> Registering…</> : "Register Service"}
                            </Button>
                        </div>
                    </DialogContent>
                </Dialog>
            </div>

            {services.length === 0 ? (
                <Card>
                    <CardContent className="py-16 text-center">
                        <Globe className="h-10 w-10 mx-auto text-muted-foreground/20 mb-4" />
                        <h3 className="text-base font-medium">No services registered</h3>
                        <p className="text-xs text-muted-foreground mt-1">Click &quot;Add Service&quot; to register an external API.</p>
                    </CardContent>
                </Card>
            ) : (
                <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
                    {services.map((svc) => (
                        <Card key={svc.id} className="group relative">
                            <CardHeader className="pb-2">
                                <div className="flex items-center justify-between">
                                    <CardTitle className="text-sm flex items-center gap-2">
                                        <Plug className="h-3.5 w-3.5 text-blue-400" />
                                        {svc.name}
                                    </CardTitle>
                                    <div className="flex items-center gap-1">
                                        <Badge variant={svc.service_type === "llm" ? "default" : "secondary"} className="text-[9px] h-4">{svc.service_type}</Badge>
                                        <Button variant="ghost" size="icon" className="h-6 w-6 opacity-0 group-hover:opacity-100 transition-opacity text-destructive" onClick={() => handleDelete(svc.id, svc.name)}>
                                            <Trash2 className="h-3 w-3" />
                                        </Button>
                                    </div>
                                </div>
                            </CardHeader>
                            <CardContent className="space-y-1.5 text-xs">
                                <div className="flex justify-between">
                                    <span className="text-muted-foreground">URL</span>
                                    <span className="font-mono text-[10px] truncate max-w-[180px]">{svc.base_url}</span>
                                </div>
                                <div className="flex justify-between">
                                    <span className="text-muted-foreground">Credential</span>
                                    <span className="font-medium text-[11px]">{getCredentialName(svc.credential_id)}</span>
                                </div>
                                <div className="flex justify-between">
                                    <span className="text-muted-foreground">Created</span>
                                    <span className="text-[10px] text-muted-foreground">{formatDistanceToNow(new Date(svc.created_at), { addSuffix: true })}</span>
                                </div>
                            </CardContent>
                        </Card>
                    ))}
                </div>
            )}
        </div>
    );
}
