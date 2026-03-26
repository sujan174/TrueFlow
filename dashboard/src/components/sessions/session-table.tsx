"use client"

import { Eye, AlertCircle } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import type { SessionRow } from "@/lib/types/session"
import { formatRelativeTime, formatCost } from "@/lib/types/audit"

interface SessionTableProps {
  sessions: SessionRow[]
  onSelect: (session: SessionRow) => void
}

export function SessionTable({ sessions, onSelect }: SessionTableProps) {
  return (
    <table className="w-full">
      <thead className="bg-muted/50 border-b">
        <tr className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
          <th className="px-4 py-3 text-left">Session ID</th>
          <th className="px-4 py-3 text-left">Requests</th>
          <th className="px-4 py-3 text-left">Tokens</th>
          <th className="px-4 py-3 text-left">Cost</th>
          <th className="px-4 py-3 text-left">Latency</th>
          <th className="px-4 py-3 text-left">Models</th>
          <th className="px-4 py-3 text-left">First Request</th>
          <th className="px-4 py-3 text-left">Last Request</th>
          <th className="px-4 py-3 text-right">Actions</th>
        </tr>
      </thead>
      <tbody>
        {sessions.map((session, index) => {
          const hasSessionId = !!session.session_id
          return (
            <tr
              key={session.session_id || `unknown-${index}`}
              className={`border-b last:border-0 transition-colors ${
                hasSessionId
                  ? "hover:bg-muted/30 cursor-pointer"
                  : "opacity-60"
              }`}
              onClick={() => hasSessionId && onSelect(session)}
            >
              <td className="px-4 py-3">
                <code className="text-xs font-mono text-muted-foreground">
                  {session.session_id ? (
                    session.session_id.length > 20
                      ? `${session.session_id.slice(0, 20)}...`
                      : session.session_id
                  ) : (
                    <span className="text-muted-foreground/50 italic flex items-center gap-1">
                      <AlertCircle className="h-3 w-3" />
                      No ID
                    </span>
                  )}
                </code>
              </td>
              <td className="px-4 py-3">
                <span className="text-sm font-medium">{session.total_requests.toLocaleString()}</span>
              </td>
              <td className="px-4 py-3">
                <span className="text-sm text-muted-foreground">
                  {(session.total_prompt_tokens + session.total_completion_tokens).toLocaleString()}
                </span>
              </td>
              <td className="px-4 py-3">
                <span className="text-sm font-medium">
                  {formatCost(session.total_cost_usd)}
                </span>
              </td>
              <td className="px-4 py-3">
                <span className="text-sm text-muted-foreground">
                  {session.total_latency_ms > 0 && session.total_requests > 0
                    ? `${(session.total_latency_ms / session.total_requests).toFixed(0)}ms avg`
                    : "—"}
                </span>
              </td>
              <td className="px-4 py-3">
                <div className="flex flex-wrap gap-1 max-w-[200px]">
                  {session.models_used && session.models_used.length > 0 ? (
                    session.models_used.slice(0, 2).map((model, idx) => (
                      <Badge key={`${model}-${idx}`} variant="outline" className="text-[10px]">
                        {model.length > 15 ? `${model.slice(0, 15)}...` : model}
                      </Badge>
                    ))
                  ) : (
                    <span className="text-sm text-muted-foreground">—</span>
                  )}
                  {session.models_used && session.models_used.length > 2 && (
                    <Badge variant="outline" className="text-[10px]">
                      +{session.models_used.length - 2}
                    </Badge>
                  )}
                </div>
              </td>
              <td className="px-4 py-3">
                <span className="text-sm text-muted-foreground">
                  {formatRelativeTime(session.first_request_at)}
                </span>
              </td>
              <td className="px-4 py-3">
                <span className="text-sm text-muted-foreground">
                  {formatRelativeTime(session.last_request_at)}
                </span>
              </td>
              <td className="px-4 py-3 text-right">
                {hasSessionId ? (
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    onClick={(e) => {
                      e.stopPropagation()
                      onSelect(session)
                    }}
                  >
                    <Eye className="h-4 w-4" />
                  </Button>
                ) : (
                  <TooltipProvider>
                    <Tooltip>
                      <TooltipTrigger render={
                        <span className="inline-block">
                          <Button variant="ghost" size="icon-sm" disabled>
                            <Eye className="h-4 w-4" />
                          </Button>
                        </span>
                      } />
                      <TooltipContent>
                        <p>Sessions without ID cannot be viewed</p>
                      </TooltipContent>
                    </Tooltip>
                  </TooltipProvider>
                )}
              </td>
            </tr>
          )
        })}
      </tbody>
    </table>
  )
}