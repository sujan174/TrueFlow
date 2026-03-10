"use client";

import { useState, useEffect } from "react";
import useSWR, { mutate } from "swr";
import {
  listTokens,
  createToken,
  revokeToken,
  listCredentials,
  listTeams,
  Token,
  Team,
  Credential,
  CreateTokenRequest,
  swrFetcher,
} from "@/lib/api";
import {
  Plus, RefreshCw, Key, Shield, Trash2, Loader2, AlertTriangle, Blocks, Copy, Check
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { CountUp } from "@/components/ui/count-up";
import { EmptyState } from "@/components/empty-state";
import { PageSkeleton } from "@/components/page-skeleton";
import { useRouter } from "next/navigation";
import { DataTable } from "@/components/data-table";
import { columns } from "./columns";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
  DialogClose,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { toast } from "sonner";
import { cn } from "@/lib/utils";

export default function TokensPage() {
  const router = useRouter();
  const { data: rawTokens = [], mutate: mutateTokens, isLoading: loading } = useSWR<Token[]>("/tokens", swrFetcher);
  const [createOpen, setCreateOpen] = useState(false);
  const [revokeTokenData, setRevokeTokenData] = useState<Token | null>(null);
  const [createdToken, setCreatedToken] = useState<{ id: string; name: string } | null>(null);

  const tokens = [...rawTokens].sort((a, b) => {
    if (a.is_active && !b.is_active) return -1;
    if (!a.is_active && b.is_active) return 1;
    return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
  });

  const handleRevoke = async () => {
    if (!revokeTokenData) return;
    try {
      await mutateTokens(
        async () => {
          await revokeToken(revokeTokenData.id);
          return rawTokens.map(t => t.id === revokeTokenData.id ? { ...t, is_active: false } : t);
        },
        {
          optimisticData: rawTokens.map(t => t.id === revokeTokenData.id ? { ...t, is_active: false } : t),
          rollbackOnError: true,
          revalidate: true
        }
      );
      toast.success("Token revoked successfully");
      setRevokeTokenData(null);
    } catch {
      toast.error("Failed to revoke token");
    }
  };

  const handleCreateToken = async (data: CreateTokenRequest) => {
    // Generate an ID that will be consistent across retries of the same mutate request
    const tempId = "temp-" + crypto.randomUUID();
    const tempToken: Token = {
      id: tempId,
      project_id: "",
      name: data.name,
      credential_id: data.credential_id || "",
      upstream_url: data.upstream_url,
      scopes: [],
      policy_ids: [],
      log_level: data.log_level ?? 1,
      is_active: true,
      created_at: new Date().toISOString(),
      team_id: data.team_id ?? null,
      allowed_models: data.allowed_models ?? null,
      allowed_model_group_ids: null,
      tags: data.tags ?? null,
      mcp_allowed_tools: data.mcp_allowed_tools ?? null,
      mcp_blocked_tools: data.mcp_blocked_tools ?? null,
    };

    let realTokenId = tempId;
    await mutateTokens(
      async () => {
        const result = await createToken(data);
        realTokenId = result.token_id;
        // Trigger revalidation to get the real token
        return [...rawTokens, tempToken];
      },
      {
        optimisticData: [...rawTokens, tempToken],
        rollbackOnError: true,
        revalidate: true
      }
    );
    setCreatedToken({ id: realTokenId, name: data.name });
  };

  const activeCount = tokens.filter((t) => t.is_active).length;
  const revokedCount = tokens.length - activeCount;

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-lg font-semibold tracking-tight text-white">Agents</h1>
          <p className="text-xs text-zinc-500 mt-0.5">Virtual tokens for AI agent authentication.</p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <Button variant="outline" size="sm" onClick={() => mutateTokens()} disabled={loading}>
            <RefreshCw className={cn("h-3.5 w-3.5 mr-1.5", loading && "animate-spin")} />
            Refresh
          </Button>
          <Dialog open={createOpen} onOpenChange={setCreateOpen}>
            <DialogTrigger asChild>
              <Button size="sm">
                <Plus className="mr-1.5 h-3.5 w-3.5" /> Create Token
              </Button>
            </DialogTrigger>
            <DialogContent className="sm:max-w-[480px] bg-zinc-950 border-white/10 p-0 overflow-hidden">
              <div className="p-6 overflow-y-auto max-h-[85vh] scrollbar-none">
                <CreateTokenForm
                  onSuccess={() => setCreateOpen(false)}
                  onCreate={handleCreateToken}
                />
              </div>            </DialogContent>
          </Dialog>
        </div>
      </div>

      {/* KPI Cards */}
      <div className="grid gap-4 md:grid-cols-3 animate-slide-up">
        <div className="bg-black border border-white/10 rounded-lg p-5">
          <div className="flex items-center gap-4">
            <div className="h-10 w-10 rounded-full bg-white/5 border border-white/10 flex items-center justify-center flex-shrink-0">
              <Key className="h-5 w-5 text-white" />
            </div>
            <div>
              <p className="text-2xl font-semibold tabular-nums text-white leading-none tracking-tight">
                <CountUp value={tokens.length} />
              </p>
              <p className="text-[11px] font-medium text-zinc-500 uppercase tracking-widest mt-1">Total Tokens</p>
            </div>
          </div>
        </div>
        <div className={cn("bg-black border border-white/10 rounded-lg p-5", activeCount > 0 && "border-white/20 bg-white/[0.02]")}>
          <div className="flex items-center gap-4">
            <div className="h-10 w-10 rounded-full bg-white/5 border border-white/10 flex items-center justify-center flex-shrink-0">
              <Shield className="h-5 w-5 text-white" />
            </div>
            <div>
              <p className="text-2xl font-semibold tabular-nums text-white leading-none tracking-tight">
                <CountUp value={activeCount} />
              </p>
              <p className="text-[11px] font-medium text-zinc-500 uppercase tracking-widest mt-1">Active</p>
            </div>
          </div>
        </div>
        <div className="bg-black border border-white/10 rounded-lg p-5">
          <div className="flex items-center gap-4">
            <div className="h-10 w-10 rounded-full bg-white/5 border border-white/10 flex items-center justify-center flex-shrink-0">
              <Trash2 className="h-5 w-5 text-zinc-500" />
            </div>
            <div>
              <p className="text-2xl font-semibold tabular-nums text-zinc-400 leading-none tracking-tight">
                <CountUp value={revokedCount} />
              </p>
              <p className="text-[11px] font-medium text-zinc-600 uppercase tracking-widest mt-1">Revoked</p>
            </div>
          </div>
        </div>
      </div>

      {/* Table */}
      {loading ? (
        <PageSkeleton cards={3} rows={5} />
      ) : tokens.length === 0 ? (
        <EmptyState
          icon={Key}
          title="No tokens created"
          description="Create a virtual token to give your agents controlled access to upstream APIs."
          actionLabel="Create Token"
          onAction={() => setCreateOpen(true)}
          className="bg-black border-white/10"
        />
      ) : (
        <div className="animate-slide-up stagger-2 bg-black border border-white/10 rounded-md overflow-hidden">
          <DataTable
            columns={columns}
            data={tokens}
            searchKey="name"
            searchPlaceholder="Filter tokens..."
            onRowClick={(token) => router.push(`/virtual-keys/${token.id}`)}
            meta={{
              onRevoke: (t: Token) => setRevokeTokenData(t),
            }}
          />
        </div>
      )}

      {/* Revoke Confirmation Dialog */}
      <Dialog open={!!revokeTokenData} onOpenChange={(open) => !open && setRevokeTokenData(null)}>
        <DialogContent className="bg-zinc-950 border-rose-500/20 text-white">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-rose-500 font-medium">
              <AlertTriangle className="h-5 w-5" /> Revoke Token
            </DialogTitle>
            <DialogDescription className="pt-1 text-zinc-400 text-[13px]">
              Are you sure you want to revoke the token <span className="font-mono font-medium text-white">{revokeTokenData?.name}</span>?
              This action cannot be undone and any agents using this token will effectively stop working.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setRevokeTokenData(null)}>Cancel</Button>
            <Button variant="destructive" onClick={handleRevoke}>Revoke Token</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Token Created — Show Integration Snippet */}
      {createdToken && (
        <TokenCreatedDialog
          tokenId={createdToken.id}
          tokenName={createdToken.name}
          onClose={() => setCreatedToken(null)}
        />
      )}
    </div>
  );
}

// ── Create Token Form ─────────────────────────────

function CreateTokenForm({ onSuccess, onCreate }: { onSuccess: () => void; onCreate: (data: CreateTokenRequest) => Promise<void> }) {
  const [loading, setLoading] = useState(false);
  const [credentials, setCredentials] = useState<Credential[]>([]);
  const [fetchingCreds, setFetchingCreds] = useState(true);
  const [mode, setMode] = useState<"managed" | "passthrough">("managed");
  const [upstreamMode, setUpstreamMode] = useState<"single" | "multi">("single");

  const [formData, setFormData] = useState<CreateTokenRequest>({
    name: "",
    credential_id: "",
    upstream_url: "https://api.openai.com/v1", // Default good DX
    log_level: 1, // 1 = Redacted payload
  });

  // Multi-upstream entries
  const [upstreams, setUpstreams] = useState<Array<{ url: string; weight: string; priority: string }>>([
    { url: "https://api.openai.com/v1", weight: "1", priority: "1" },
  ]);

  // Extended fields
  const [teams, setTeams] = useState<Team[]>([]);
  const [allowedModelsInput, setAllowedModelsInput] = useState("");
  const [tagsInput, setTagsInput] = useState(""); // JSON string for tags kv
  const [mcpAllowedInput, setMcpAllowedInput] = useState("");
  const [mcpBlockedInput, setMcpBlockedInput] = useState("");

  useEffect(() => {
    listCredentials()
      .then(setCredentials)
      .catch(() => toast.error("Failed to load credentials"))
      .finally(() => setFetchingCreds(false));
    listTeams()
      .then(setTeams)
      .catch(() => { }); // teams are optional, soft fail
  }, []);

  const addUpstream = () =>
    setUpstreams((prev) => [...prev, { url: "", weight: "1", priority: String(prev.length + 1) }]);

  const removeUpstream = (i: number) =>
    setUpstreams((prev) => prev.filter((_, idx) => idx !== i));

  const updateUpstream = (i: number, field: "url" | "weight" | "priority", value: string) =>
    setUpstreams((prev) => prev.map((u, idx) => idx === i ? { ...u, [field]: value } : u));

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      setLoading(true);
      const payload: CreateTokenRequest = { ...formData };
      if (mode === "passthrough") {
        delete (payload as unknown as Record<string, unknown>).credential_id;
      }
      if (upstreamMode === "multi") {
        // Validate all URLs are filled
        if (upstreams.some((u) => !u.url.trim())) {
          toast.error("All upstream URLs must be filled in");
          return;
        }
        payload.upstreams = upstreams.map((u) => ({
          url: u.url.trim(),
          weight: u.weight ? parseFloat(u.weight) : undefined,
          priority: u.priority ? parseInt(u.priority) : undefined,
        }));
        // Use first upstream as the primary URL for backward compat
        payload.upstream_url = upstreams[0].url.trim();
      }
      // Parse allowed_models from comma-separated input
      if (allowedModelsInput.trim()) {
        payload.allowed_models = allowedModelsInput.split(",").map(m => m.trim()).filter(Boolean);
      }
      // Parse tags from JSON
      if (tagsInput.trim()) {
        try {
          payload.tags = JSON.parse(tagsInput);
        } catch {
          toast.error("Tags must be valid JSON, e.g. {\"team\": \"eng\"}");
          return;
        }
      }
      // Parse MCP allowed tools
      if (mcpAllowedInput.trim()) {
        payload.mcp_allowed_tools = mcpAllowedInput.split(",").map(t => t.trim()).filter(Boolean);
      }
      // Parse MCP blocked tools
      if (mcpBlockedInput.trim()) {
        payload.mcp_blocked_tools = mcpBlockedInput.split(",").map(t => t.trim()).filter(Boolean);
      }
      await onCreate(payload);
      toast.success("Token created successfully");
      onSuccess();
      toast.error(e instanceof Error ? e.message : "Failed to create token");
    } finally {
      setLoading(false);
    }
  };

  return (
    <form onSubmit={handleSubmit}>
      <DialogHeader>
        <DialogTitle className="text-white font-medium">Create Token</DialogTitle>
        <DialogDescription className="text-zinc-500 text-[13px]">
          Issue a new virtual token for agent authentication.
        </DialogDescription>
      </DialogHeader>
      <div className="grid gap-4 py-4">
        {/* Credential Mode Selector */}
        <div className="space-y-1.5">
          <Label className="text-zinc-400 text-xs uppercase tracking-widest">Credential Mode</Label>
          <div className="grid grid-cols-2 gap-2">
            <button
              type="button"
              onClick={() => setMode("managed")}
              className={cn(
                "rounded-md border px-3 py-2 text-xs font-medium transition-all",
                mode === "managed"
                  ? "border-white/30 bg-white/5 text-white"
                  : "border-white/10 bg-black text-zinc-500 hover:text-white hover:bg-white/[0.02]"
              )}
            >
              <Shield className="h-3.5 w-3.5 mx-auto mb-1" />
              Managed Credentials
            </button>
            <button
              type="button"
              onClick={() => setMode("passthrough")}
              className={cn(
                "rounded-md border px-3 py-2 text-xs font-medium transition-all",
                mode === "passthrough"
                  ? "border-white/30 bg-white/5 text-white"
                  : "border-white/10 bg-black text-zinc-500 hover:text-white hover:bg-white/[0.02]"
              )}
            >
              <Key className="h-3.5 w-3.5 mx-auto mb-1" />
              Passthrough / BYOK
            </button>
          </div>
          <p className="text-[10px] text-zinc-500">
            {mode === "managed"
              ? "TrueFlow injects credentials from the vault. Agents never see real API keys."
              : "Agents provide their own API key via X-Real-Authorization header. TrueFlow provides observability, analytics, and policies."}
          </p>
        </div>

        <div className="space-y-1.5">
          <Label htmlFor="name" className="text-zinc-400 text-xs uppercase tracking-widest">
            Token Name
          </Label>
          <Input
            id="name"
            value={formData.name}
            onChange={(e) => setFormData({ ...formData, name: e.target.value })}
            placeholder="e.g. billing-agent-v1"
            className="bg-black border-white/10 text-white placeholder:text-zinc-600 focus-visible:ring-white/20"
            required
          />
        </div>

        {/* Upstream Mode Selector */}
        <div className="space-y-1.5">
          <Label className="text-zinc-400 text-xs uppercase tracking-widest">Upstream Configuration</Label>
          <div className="grid grid-cols-2 gap-2">
            <button
              type="button"
              onClick={() => setUpstreamMode("single")}
              className={cn(
                "rounded-md border px-3 py-2 text-xs font-medium transition-all",
                upstreamMode === "single"
                  ? "border-white/30 bg-white/5 text-white"
                  : "border-white/10 bg-black text-zinc-500 hover:text-white hover:bg-white/[0.02]"
              )}
            >
              Single Upstream
            </button>
            <button
              type="button"
              onClick={() => setUpstreamMode("multi")}
              className={cn(
                "rounded-md border px-3 py-2 text-xs font-medium transition-all",
                upstreamMode === "multi"
                  ? "border-white/30 bg-white/5 text-white"
                  : "border-white/10 bg-black text-zinc-500 hover:text-white hover:bg-white/[0.02]"
              )}
            >
              <Blocks className="h-3.5 w-3.5 mx-auto mb-1" />
              Load Balancer
            </button>
          </div>
        </div>

        {upstreamMode === "single" ? (
          <div className="space-y-1.5">
            <Label htmlFor="upstream" className="text-zinc-400 text-xs uppercase tracking-widest">
              Upstream API URL
            </Label>
            <Input
              id="upstream"
              value={formData.upstream_url}
              onChange={(e) => setFormData({ ...formData, upstream_url: e.target.value })}
              placeholder="https://api.openai.com/v1"
              className="bg-black border-white/10 text-white font-mono placeholder:text-zinc-600 focus-visible:ring-white/20"
              required
            />
          </div>
        ) : (
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label className="text-zinc-400 text-xs uppercase tracking-widest">Upstream Endpoints</Label>
              <button
                type="button"
                onClick={addUpstream}
                className="text-[10px] text-white hover:text-zinc-300 flex items-center gap-1 transition-colors uppercase tracking-wider"
              >
                <Plus className="h-3 w-3" /> Add endpoint
              </button>
            </div>
            <p className="text-[10px] text-zinc-500">
              Requests are distributed by weight. Higher priority endpoints are tried first on failover.
            </p>
            <div className="space-y-2 max-h-[220px] overflow-y-auto pr-1">
              {upstreams.map((u, i) => (
                <div key={i} className="rounded-md border border-white/10 bg-white/[0.02] p-3 space-y-2">
                  <div className="flex items-center justify-between">
                    <span className="text-[10px] font-semibold text-zinc-500 uppercase tracking-widest">
                      Endpoint {i + 1}
                    </span>
                    {upstreams.length > 1 && (
                      <button
                        type="button"
                        onClick={() => removeUpstream(i)}
                        className="text-zinc-500 hover:text-rose-400 transition-colors"
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </button>
                    )}
                  </div>
                  <Input
                    value={u.url}
                    onChange={(e) => updateUpstream(i, "url", e.target.value)}
                    placeholder="https://api.openai.com/v1"
                    className="h-8 text-[11px] font-mono bg-black border-white/10 text-white placeholder:text-zinc-600 focus-visible:ring-white/20"
                  />
                  <div className="grid grid-cols-2 gap-2">
                    <div className="space-y-1">
                      <Label className="text-[10px] text-zinc-500 uppercase tracking-widest">Weight</Label>
                      <Input
                        type="number"
                        min="0"
                        step="0.1"
                        value={u.weight}
                        onChange={(e) => updateUpstream(i, "weight", e.target.value)}
                        placeholder="1"
                        className="h-7 text-xs bg-black border-white/10 text-white placeholder:text-zinc-600 focus-visible:ring-white/20"
                      />
                    </div>
                    <div className="space-y-1">
                      <Label className="text-[10px] text-zinc-500 uppercase tracking-widest">Priority</Label>
                      <Input
                        type="number"
                        min="1"
                        value={u.priority}
                        onChange={(e) => updateUpstream(i, "priority", e.target.value)}
                        placeholder="1"
                        className="h-7 text-xs bg-black border-white/10 text-white placeholder:text-zinc-600 focus-visible:ring-white/20"
                      />
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}

        {mode === "managed" && (
          <div className="space-y-1.5">
            <Label htmlFor="cred_id" className="text-zinc-400 text-xs uppercase tracking-widest">
              Backing Credential
            </Label>
            {fetchingCreds ? (
              <div className="h-10 w-full animate-pulse bg-white/5 border border-white/10 rounded-md" />
            ) : (
              <Select
                value={formData.credential_id}
                onValueChange={(val) => setFormData({ ...formData, credential_id: val })}
              >
                <SelectTrigger className="bg-black border-white/10 text-white focus:ring-white/20">
                  <SelectValue placeholder="Select a credential..." />
                </SelectTrigger>
                <SelectContent className="bg-zinc-950 border-white/10 text-white">
                  {credentials.filter(c => c.is_active).map((cred) => (
                    <SelectItem key={cred.id} value={cred.id} className="focus:bg-white/5 focus:text-white">
                      {cred.name} ({cred.provider})
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
            {credentials.length === 0 && !fetchingCreds && (
              <p className="text-[10px] text-zinc-500 mt-1">
                No active credentials found. Create one first.
              </p>
            )}
          </div>
        )}

        {/* Advanced Options */}
        <div className="space-y-3 pt-4 border-t border-white/[0.06]">
          <Label className="text-[10px] font-semibold text-zinc-500 uppercase tracking-widest">Advanced Options</Label>

          {/* Team Assignment */}
          {teams.length > 0 && (
            <div className="space-y-1.5">
              <Label htmlFor="team_id" className="text-zinc-400 text-xs uppercase tracking-widest">Team (optional)</Label>
              <Select
                value={formData.team_id || ""}
                onValueChange={(val) => setFormData({ ...formData, team_id: val || undefined })}
              >
                <SelectTrigger className="bg-black border-white/10 text-white focus:ring-white/20">
                  <SelectValue placeholder="No team assigned" />
                </SelectTrigger>
                <SelectContent className="bg-zinc-950 border-white/10 text-white">
                  <SelectItem value="" className="focus:bg-white/5 focus:text-white">No team</SelectItem>
                  {teams.map((t) => (
                    <SelectItem key={t.id} value={t.id} className="focus:bg-white/5 focus:text-white">{t.name}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          )}

          {/* Fallback URL — single upstream only */}
          {upstreamMode === "single" && (
            <div className="space-y-1.5">
              <Label htmlFor="fallback_url" className="text-zinc-400 text-xs uppercase tracking-widest">Fallback URL (optional)</Label>
              <Input
                id="fallback_url"
                value={formData.fallback_url || ""}
                onChange={(e) => setFormData({ ...formData, fallback_url: e.target.value || undefined })}
                placeholder="https://api.backup.com/v1"
                className="bg-black border-white/10 text-white font-mono placeholder:text-zinc-600 focus-visible:ring-white/20"
              />
              <p className="text-[10px] text-zinc-500">Used automatically on upstream failure.</p>
            </div>
          )}

          {/* Allowed Models */}
          <div className="space-y-1.5">
            <Label htmlFor="allowed_models" className="text-zinc-400 text-xs uppercase tracking-widest">Allowed Models (optional)</Label>
            <Input
              id="allowed_models"
              value={allowedModelsInput}
              onChange={(e) => setAllowedModelsInput(e.target.value)}
              placeholder="gpt-4o, claude-3-*, gemini-1.5-pro"
              className="bg-black border-white/10 text-white font-mono placeholder:text-zinc-600 focus-visible:ring-white/20"
            />
            <p className="text-[10px] text-zinc-500">Comma-separated model names or glob patterns. Leave blank to allow all.</p>
          </div>

          {/* Tags */}
          <div className="space-y-1.5">
            <Label htmlFor="tags" className="text-zinc-400 text-xs uppercase tracking-widest">Tags (optional)</Label>
            <Input
              id="tags"
              value={tagsInput}
              onChange={(e) => setTagsInput(e.target.value)}
              placeholder='{"team": "eng", "env": "prod"}'
              className="bg-black border-white/10 text-white font-mono placeholder:text-zinc-600 focus-visible:ring-white/20"
            />
            <p className="text-[10px] text-zinc-500">JSON object for cost attribution and filtering.</p>
          </div>

          {/* MCP Tool Access Control */}
          <div className="space-y-1.5">
            <Label htmlFor="mcp_allowed_tools" className="text-zinc-400 text-xs uppercase tracking-widest">MCP Allowed Tools (optional)</Label>
            <Input
              id="mcp_allowed_tools"
              value={mcpAllowedInput}
              onChange={(e) => setMcpAllowedInput(e.target.value)}
              placeholder="mcp__slack__*, mcp__brave__search"
              className="bg-black border-white/10 text-white font-mono placeholder:text-zinc-600 focus-visible:ring-white/20"
            />
            <p className="text-[10px] text-zinc-500">Comma-separated. Only these MCP tools will be injected. Glob patterns supported. Leave blank to allow all.</p>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="mcp_blocked_tools" className="text-zinc-400 text-xs uppercase tracking-widest">MCP Blocked Tools (optional)</Label>
            <Input
              id="mcp_blocked_tools"
              value={mcpBlockedInput}
              onChange={(e) => setMcpBlockedInput(e.target.value)}
              placeholder="mcp__*__delete_*, mcp__slack__admin_*"
              className="bg-black border-white/10 text-white font-mono placeholder:text-zinc-600 focus-visible:ring-white/20"
            />
            <p className="text-[10px] text-zinc-500">Comma-separated. These MCP tools will be blocked even if in the allowed list. Takes priority.</p>
          </div>
        </div>

        {/* Privacy & Logging */}
        <div className="space-y-1.5 pt-4 border-t border-white/[0.06]">
          <Label className="text-zinc-400 text-xs uppercase tracking-widest flex items-center gap-2">
            Privacy & Logging
          </Label>
          <Select
            value={String(formData.log_level)}
            onValueChange={(val) => setFormData({ ...formData, log_level: parseInt(val) })}
          >
            <SelectTrigger className="bg-black border-white/10 text-white focus:ring-white/20">
              <SelectValue />
            </SelectTrigger>
            <SelectContent className="bg-zinc-950 border-white/10 text-white">
              <SelectItem value="0" className="focus:bg-white/5 focus:text-white">Metadata Only (No payloads saved)</SelectItem>
              <SelectItem value="1" className="focus:bg-white/5 focus:text-white">Redacted Payload (Scrub PII keys/secrets)</SelectItem>
              <SelectItem value="2" className="focus:bg-white/5 focus:text-white">Full Payload (Everything saved - Best for debugging)</SelectItem>
            </SelectContent>
          </Select>
          <p className="text-[10px] text-zinc-500">
            Controls what request and response data is stored in the Gateway&apos;s audit logs.
          </p>
        </div>
      </div>
      <DialogFooter className="pt-2 border-t border-white/[0.06]">
        <DialogClose asChild>
          <Button variant="ghost" type="button">Cancel</Button>
        </DialogClose>
        <Button type="submit" disabled={loading || (mode === "managed" && !formData.credential_id)}>
          {loading && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
          {loading ? "Creating..." : "Create Token"}
        </Button>
      </DialogFooter>
    </form>
  );
}

// ── Token Created Success Dialog ───────────────────────────────

function TokenCreatedDialog({
  tokenId,
  tokenName,
  onClose,
}: {
  tokenId: string;
  tokenName: string;
  onClose: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const gatewayUrl =
    typeof window !== "undefined"
      ? (process.env.NEXT_PUBLIC_GATEWAY_URL || window.location.origin.replace(":3000", ":8443"))
      : "http://localhost:8443";

  const curlSnippet = `curl ${gatewayUrl}/v1/chat/completions \\
  -H "Authorization: Bearer ${tokenId}" \\
  -H "Content-Type: application/json" \\
  -d '{
    "model": "gpt-4o",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'`;

  const handleCopy = () => {
    navigator.clipboard.writeText(curlSnippet);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-[560px] bg-zinc-950 border-white/10 text-white p-0 overflow-hidden">
        <div className="p-6 space-y-5">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-white font-medium">
              <div className="h-5 w-5 rounded-full bg-emerald-500/15 flex items-center justify-center">
                <Check className="h-3 w-3 text-emerald-400" />
              </div>
              Token Created
            </DialogTitle>
            <DialogDescription className="text-zinc-400 text-[13px]">
              <span className="font-mono text-white">{tokenName}</span> is ready. Point your OpenAI SDK at TrueFlow by changing the{" "}
              <code className="text-zinc-300 bg-white/5 px-1 py-0.5 rounded text-[11px]">base_url</code> and key.
            </DialogDescription>
          </DialogHeader>

          {/* Token ID */}
          <div className="space-y-1.5">
            <p className="text-[10px] font-semibold text-zinc-500 uppercase tracking-widest">Your Virtual Token</p>
            <div className="flex items-center gap-2 rounded-md border border-white/10 bg-black px-3 py-2">
              <code className="flex-1 font-mono text-[12px] text-emerald-400 truncate">{tokenId}</code>
            </div>
            <p className="text-[10px] text-zinc-600">This token is shown once. Save it now — it cannot be retrieved again from the dashboard.</p>
          </div>

          {/* Base URL */}
          <div className="space-y-1.5">
            <p className="text-[10px] font-semibold text-zinc-500 uppercase tracking-widest">Gateway Base URL</p>
            <div className="flex items-center gap-2 rounded-md border border-white/10 bg-black px-3 py-2">
              <code className="flex-1 font-mono text-[12px] text-zinc-300">{gatewayUrl}/v1</code>
            </div>
          </div>

          {/* curl snippet */}
          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <p className="text-[10px] font-semibold text-zinc-500 uppercase tracking-widest">Quick Start</p>
              <button
                onClick={handleCopy}
                className="flex items-center gap-1 text-[10px] text-zinc-500 hover:text-white transition-colors"
              >
                {copied ? <Check className="h-3 w-3 text-emerald-400" /> : <Copy className="h-3 w-3" />}
                {copied ? "Copied!" : "Copy"}
              </button>
            </div>
            <pre className="rounded-md border border-white/10 bg-black px-4 py-3 font-mono text-[11px] text-zinc-300 overflow-x-auto whitespace-pre scrollbar-none leading-relaxed">
              {curlSnippet}
            </pre>
          </div>

          {/* SDK hint */}
          <div className="rounded-md border border-white/[0.06] bg-white/[0.02] px-4 py-3 text-[12px] text-zinc-400 space-y-1">
            <p className="font-semibold text-zinc-300">OpenAI SDK drop-in</p>
            <pre className="font-mono text-[11px] text-zinc-500 whitespace-pre-wrap">{`client = OpenAI(
  base_url="${gatewayUrl}/v1",
  api_key="${tokenId}",
)`}</pre>
          </div>
        </div>

        <div className="border-t border-white/[0.06] px-6 py-4 flex justify-end">
          <Button onClick={onClose}>Done</Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
