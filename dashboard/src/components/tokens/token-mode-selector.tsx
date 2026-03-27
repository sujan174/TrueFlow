"use client"

import { Shield, KeyRound } from "lucide-react"
import { cn } from "@/lib/utils"

export type TokenMode = "managed" | "passthrough"

interface TokenModeSelectorProps {
  value: TokenMode
  onChange: (mode: TokenMode) => void
}

export function TokenModeSelector({ value, onChange }: TokenModeSelectorProps) {
  return (
    <div className="space-y-2">
      <label className="text-sm font-medium">Token Mode</label>
      <div className="grid grid-cols-2 gap-2">
        <button
          type="button"
          onClick={() => onChange("managed")}
          className={cn(
            "flex flex-col items-start p-3 border rounded-lg transition-all text-left",
            value === "managed"
              ? "border-primary bg-primary/5"
              : "border-border hover:border-primary/50"
          )}
        >
          <div className="flex items-center gap-2 mb-1">
            <Shield className={cn("h-4 w-4", value === "managed" ? "text-primary" : "text-muted-foreground")} />
            <span className={cn("text-sm font-medium", value === "managed" && "text-primary")}>
              Managed Credential
            </span>
          </div>
          <p className="text-xs text-muted-foreground">
            API keys are encrypted and stored securely by AILink
          </p>
        </button>

        <button
          type="button"
          onClick={() => onChange("passthrough")}
          className={cn(
            "flex flex-col items-start p-3 border rounded-lg transition-all text-left",
            value === "passthrough"
              ? "border-primary bg-primary/5"
              : "border-border hover:border-primary/50"
          )}
        >
          <div className="flex items-center gap-2 mb-1">
            <KeyRound className={cn("h-4 w-4", value === "passthrough" ? "text-primary" : "text-muted-foreground")} />
            <span className={cn("text-sm font-medium", value === "passthrough" && "text-primary")}>
              Passthrough (BYOK)
            </span>
          </div>
          <p className="text-xs text-muted-foreground">
            You control your API keys. They pass directly to providers
          </p>
        </button>
      </div>

      {value === "passthrough" && (
        <div className="mt-2 p-3 bg-amber-50 dark:bg-amber-950/20 border border-amber-200 dark:border-amber-800 rounded-lg">
          <p className="text-xs text-amber-800 dark:text-amber-200">
            <strong>Passthrough Mode:</strong> Your API keys are never stored.
            Provide them with each request using the Authorization header.
            Perfect for maximum security and existing key management systems.
          </p>
        </div>
      )}
    </div>
  )
}