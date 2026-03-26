"use client"

import { useState } from "react"
import { toast } from "sonner"
import { Button } from "@/components/ui/button"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Loader2, Download, Upload, FileCode, CheckCircle, AlertCircle } from "lucide-react"
import { exportConfig, importConfig, type ImportResult } from "@/lib/api"
import { cn } from "@/lib/utils"

export function ConfigTab() {
  const [isExporting, setIsExporting] = useState(false)
  const [isImporting, setIsImporting] = useState(false)
  const [importContent, setImportContent] = useState("")
  const [importFormat, setImportFormat] = useState<"yaml" | "json">("yaml")
  const [showResultDialog, setShowResultDialog] = useState(false)
  const [importResult, setImportResult] = useState<ImportResult | null>(null)

  async function handleExport(format: "yaml" | "json") {
    setIsExporting(true)
    try {
      const content = await exportConfig(format)
      const filename = `trueflow_config.${format}`
      const mimeType = format === "json" ? "application/json" : "application/yaml"

      const blob = new Blob([content], { type: mimeType })
      const url = URL.createObjectURL(blob)
      const a = document.createElement("a")
      a.href = url
      a.download = filename
      document.body.appendChild(a)
      a.click()
      document.body.removeChild(a)
      URL.revokeObjectURL(url)

      toast.success(`Config exported as ${format.toUpperCase()}`)
    } catch (error) {
      toast.error("Failed to export config")
      console.error(error)
    } finally {
      setIsExporting(false)
    }
  }

  async function handleImport() {
    if (!importContent.trim()) {
      toast.error("Please paste config content to import")
      return
    }

    setIsImporting(true)
    try {
      const result = await importConfig(importContent, importFormat)
      setImportResult(result)
      setShowResultDialog(true)
      setImportContent("")
      toast.success("Config imported successfully")
    } catch (error) {
      toast.error("Failed to import config. Please check the format.")
      console.error(error)
    } finally {
      setIsImporting(false)
    }
  }

  function handleFileUpload(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0]
    if (!file) return

    const reader = new FileReader()
    reader.onload = (e) => {
      const content = e.target?.result as string
      setImportContent(content)
      const filename = file.name.toLowerCase()
      if (filename.endsWith(".json")) {
        setImportFormat("json")
      } else {
        setImportFormat("yaml")
      }
    }
    reader.readAsText(file)
    event.target.value = ""
  }

  return (
    <div className="space-y-8">
      {/* Export Section */}
      <section>
        <div className="mb-4">
          <h3 className="text-sm font-medium">Export Configuration</h3>
          <p className="text-xs text-muted-foreground mt-0.5">
            Download your policies and tokens as YAML or JSON for backup or GitOps workflows
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => handleExport("yaml")}
            disabled={isExporting}
            className="gap-2"
          >
            {isExporting ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Download className="h-4 w-4" />
            )}
            Export YAML
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => handleExport("json")}
            disabled={isExporting}
            className="gap-2"
          >
            {isExporting ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Download className="h-4 w-4" />
            )}
            Export JSON
          </Button>
        </div>
        <p className="text-xs text-muted-foreground mt-3">
          Exports include policies and token configurations. Credentials are not exported for security reasons.
        </p>
      </section>

      {/* Import Section */}
      <section className="pt-8 border-t">
        <div className="mb-4">
          <h3 className="text-sm font-medium">Import Configuration</h3>
          <p className="text-xs text-muted-foreground mt-0.5">
            Import policies and tokens from a YAML or JSON file. Existing items with matching names will be updated.
          </p>
        </div>

        <div className="space-y-4">
          {/* File Upload */}
          <div>
            <Label className="text-xs">Upload File</Label>
            <div className="mt-1.5">
              <input
                type="file"
                accept=".yaml,.yml,.json"
                onChange={handleFileUpload}
                className="text-sm file:mr-3 file:py-1.5 file:px-3 file:rounded-md file:border file:border-input file:bg-background file:text-xs file:font-medium file:cursor-pointer hover:file:bg-accent"
              />
            </div>
          </div>

          {/* Format Selection */}
          <div>
            <Label className="text-xs">Import Format</Label>
            <div className="flex gap-1 p-1 bg-muted rounded-lg w-fit mt-1.5">
              <button
                onClick={() => setImportFormat("yaml")}
                className={cn(
                  "px-3 py-1.5 text-xs font-medium rounded-md transition-colors",
                  importFormat === "yaml"
                    ? "bg-background text-foreground shadow-sm"
                    : "text-muted-foreground hover:text-foreground"
                )}
              >
                YAML
              </button>
              <button
                onClick={() => setImportFormat("json")}
                className={cn(
                  "px-3 py-1.5 text-xs font-medium rounded-md transition-colors",
                  importFormat === "json"
                    ? "bg-background text-foreground shadow-sm"
                    : "text-muted-foreground hover:text-foreground"
                )}
              >
                JSON
              </button>
            </div>
          </div>

          {/* Paste Content */}
          <div>
            <Label className="text-xs">Or Paste Config Content</Label>
            <Textarea
              value={importContent}
              onChange={(e) => setImportContent(e.target.value)}
              placeholder={`version: "1"
policies:
  - name: example-policy
    mode: enforce
    phase: request
    rules: ...
tokens:
  - name: example-token
    upstream_url: https://api.openai.com/v1
    policies: [example-policy]`}
              className="font-mono text-xs min-h-[200px] mt-1.5"
            />
          </div>

          {/* Import Button */}
          <div className="flex justify-end">
            <Button
              size="sm"
              onClick={handleImport}
              disabled={isImporting || !importContent.trim()}
              className="gap-2"
            >
              {isImporting ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Importing...
                </>
              ) : (
                <>
                  <Upload className="h-4 w-4" />
                  Import Configuration
                </>
              )}
            </Button>
          </div>

          {/* Notes */}
          <div className="bg-muted/50 p-4 rounded-lg">
            <p className="text-xs font-medium mb-2">Important Notes</p>
            <ul className="text-xs text-muted-foreground space-y-1">
              <li className="flex items-start gap-2">
                <span className="text-muted-foreground/50">•</span>
                Policies and tokens are matched by name and updated if they exist
              </li>
              <li className="flex items-start gap-2">
                <span className="text-muted-foreground/50">•</span>
                New tokens will be created as stubs - you must add credentials separately
              </li>
              <li className="flex items-start gap-2">
                <span className="text-muted-foreground/50">•</span>
                Credentials are never exported or imported for security
              </li>
              <li className="flex items-start gap-2">
                <span className="text-muted-foreground/50">•</span>
                The import will skip tokens with blocked upstream URLs (SSRF protection)
              </li>
            </ul>
          </div>
        </div>
      </section>

      {/* Import Result Dialog */}
      <Dialog open={showResultDialog} onOpenChange={setShowResultDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-base">
              <CheckCircle className="h-5 w-5 text-success" />
              Import Complete
            </DialogTitle>
            <DialogDescription>
              Your configuration has been imported successfully.
            </DialogDescription>
          </DialogHeader>
          {importResult && (
            <div className="py-4">
              <div className="grid gap-3 sm:grid-cols-2">
                <div className="p-3 border rounded-lg">
                  <div className="text-xl font-semibold tabular-nums">{importResult.policies_created}</div>
                  <div className="text-xs text-muted-foreground">Policies Created</div>
                </div>
                <div className="p-3 border rounded-lg">
                  <div className="text-xl font-semibold tabular-nums">{importResult.policies_updated}</div>
                  <div className="text-xs text-muted-foreground">Policies Updated</div>
                </div>
                <div className="p-3 border rounded-lg">
                  <div className="text-xl font-semibold tabular-nums">{importResult.tokens_created}</div>
                  <div className="text-xs text-muted-foreground">Tokens Created</div>
                </div>
                <div className="p-3 border rounded-lg">
                  <div className="text-xl font-semibold tabular-nums">{importResult.tokens_updated}</div>
                  <div className="text-xs text-muted-foreground">Tokens Updated</div>
                </div>
              </div>
              {(importResult.tokens_created > 0 || importResult.tokens_updated > 0) && (
                <div className="mt-4 p-3 bg-warning/10 border border-warning/20 rounded-lg flex gap-3">
                  <AlertCircle className="h-4 w-4 text-warning shrink-0 mt-0.5" />
                  <div className="text-sm">
                    <p className="font-medium">Credentials Required</p>
                    <p className="text-muted-foreground text-xs">
                      New tokens have been created as stubs. Visit the Tokens page to add credentials for any new tokens.
                    </p>
                  </div>
                </div>
              )}
            </div>
          )}
          <div className="flex justify-end">
            <Button size="sm" onClick={() => setShowResultDialog(false)}>Done</Button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  )
}