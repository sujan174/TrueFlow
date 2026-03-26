"use client"

import { useState, useEffect } from "react"
import { toast } from "sonner"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Loader2, Plus, MoreHorizontal, Trash2, Send, Copy, Check, ExternalLink } from "lucide-react"
import {
  listWebhooks,
  createWebhook,
  deleteWebhook,
  testWebhook,
  type Webhook,
} from "@/lib/api"
import { WEBHOOK_EVENT_TYPES } from "@/lib/types/settings"
import { cn } from "@/lib/utils"

function formatDate(dateString: string): string {
  return new Date(dateString).toLocaleDateString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
  })
}

export function WebhooksTab() {
  const [webhooks, setWebhooks] = useState<Webhook[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [showCreateDialog, setShowCreateDialog] = useState(false)
  const [showDeleteDialog, setShowDeleteDialog] = useState(false)
  const [showTestDialog, setShowTestDialog] = useState(false)
  const [webhookToDelete, setWebhookToDelete] = useState<Webhook | null>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [newWebhookUrl, setNewWebhookUrl] = useState("")
  const [newWebhookEvents, setNewWebhookEvents] = useState<string[]>([])
  const [testUrl, setTestUrl] = useState("")
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null)
  const [isTesting, setIsTesting] = useState(false)
  const [newlyCreatedSecret, setNewlyCreatedSecret] = useState<string | null>(null)
  const [copiedSecret, setCopiedSecret] = useState(false)

  useEffect(() => {
    loadWebhooks()
  }, [])

  async function loadWebhooks() {
    try {
      const data = await listWebhooks()
      setWebhooks(data)
    } catch (error) {
      toast.error("Failed to load webhooks")
      console.error(error)
    } finally {
      setIsLoading(false)
    }
  }

  function openCreateDialog() {
    setNewWebhookUrl("")
    setNewWebhookEvents([])
    setNewlyCreatedSecret(null)
    setShowCreateDialog(true)
  }

  function openDeleteDialog(webhook: Webhook) {
    setWebhookToDelete(webhook)
    setShowDeleteDialog(true)
  }

  function openTestDialog(url?: string) {
    setTestUrl(url || "")
    setTestResult(null)
    setShowTestDialog(true)
  }

  function toggleEvent(event: string) {
    setNewWebhookEvents((prev) =>
      prev.includes(event) ? prev.filter((e) => e !== event) : [...prev, event]
    )
  }

  async function handleCreate() {
    if (!newWebhookUrl) {
      toast.error("Webhook URL is required")
      return
    }

    setIsSubmitting(true)
    try {
      const webhook = await createWebhook({
        url: newWebhookUrl,
        events: newWebhookEvents.length > 0 ? newWebhookEvents : undefined,
      })

      if (webhook.signing_secret) {
        setNewlyCreatedSecret(webhook.signing_secret)
      }

      toast.success("Webhook created successfully")
      await loadWebhooks()
    } catch (error) {
      toast.error("Failed to create webhook")
      console.error(error)
    } finally {
      setIsSubmitting(false)
    }
  }

  async function handleDelete() {
    if (!webhookToDelete) return

    setIsSubmitting(true)
    try {
      await deleteWebhook(webhookToDelete.id)
      toast.success("Webhook deleted")
      setShowDeleteDialog(false)
      setWebhookToDelete(null)
      await loadWebhooks()
    } catch (error) {
      toast.error("Failed to delete webhook")
      console.error(error)
    } finally {
      setIsSubmitting(false)
    }
  }

  async function handleTest() {
    if (!testUrl) {
      toast.error("Please enter a URL to test")
      return
    }

    setIsTesting(true)
    setTestResult(null)
    try {
      const result = await testWebhook(testUrl)
      setTestResult(result)
    } catch (error) {
      setTestResult({ success: false, message: "Failed to send test webhook" })
      console.error(error)
    } finally {
      setIsTesting(false)
    }
  }

  async function copyToClipboard(text: string) {
    await navigator.clipboard.writeText(text)
    setCopiedSecret(true)
    setTimeout(() => setCopiedSecret(false), 2000)
    toast.success("Copied to clipboard")
  }

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-16">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    )
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium">Webhooks</h3>
          <p className="text-xs text-muted-foreground mt-0.5">
            Configure webhooks to receive event notifications
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={() => openTestDialog()} className="gap-2">
            <Send className="h-4 w-4" />
            Test
          </Button>
          <Button size="sm" onClick={openCreateDialog} className="gap-2">
            <Plus className="h-4 w-4" />
            Add Webhook
          </Button>
        </div>
      </div>

      {/* Webhooks Table */}
      <div className="border rounded-lg">
        {webhooks.length === 0 ? (
          <div className="p-12 text-center">
            <div className="w-12 h-12 rounded-full bg-muted flex items-center justify-center mx-auto mb-4">
              <Send className="h-6 w-6 text-muted-foreground" />
            </div>
            <p className="text-sm font-medium mb-1">No webhooks configured</p>
            <p className="text-xs text-muted-foreground">
              Add a webhook to receive event notifications
            </p>
          </div>
        ) : (
          <table className="w-full">
            <thead>
              <tr className="border-b bg-muted/30">
                <th className="text-left px-4 py-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  URL
                </th>
                <th className="text-left px-4 py-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  Events
                </th>
                <th className="text-left px-4 py-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  Created
                </th>
                <th className="w-12"></th>
              </tr>
            </thead>
            <tbody className="divide-y">
              {webhooks.map((webhook) => (
                <tr key={webhook.id} className="hover:bg-muted/30 transition-colors">
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-2">
                      <code className="text-xs font-mono text-muted-foreground truncate max-w-[280px]">
                        {webhook.url}
                      </code>
                      <a
                        href={webhook.url}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-muted-foreground hover:text-foreground"
                      >
                        <ExternalLink className="h-3.5 w-3.5" />
                      </a>
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex flex-wrap gap-1">
                      {webhook.events.length === 0 ? (
                        <span className="inline-flex px-2 py-0.5 text-xs rounded bg-muted text-muted-foreground">
                          All events
                        </span>
                      ) : (
                        webhook.events.slice(0, 2).map((event) => (
                          <span key={event} className="inline-flex px-2 py-0.5 text-xs rounded bg-muted text-muted-foreground">
                            {event}
                          </span>
                        ))
                      )}
                      {webhook.events.length > 2 && (
                        <span className="text-xs text-muted-foreground">
                          +{webhook.events.length - 2}
                        </span>
                      )}
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <span className="text-xs text-muted-foreground">{formatDate(webhook.created_at)}</span>
                  </td>
                  <td className="px-4 py-3">
                    <DropdownMenu>
                      <DropdownMenuTrigger>
                        <Button variant="ghost" size="icon-sm">
                          <MoreHorizontal className="h-4 w-4" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem onClick={() => openTestDialog(webhook.url)}>
                          <Send className="mr-2 h-4 w-4" />
                          Test Webhook
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          onClick={() => openDeleteDialog(webhook)}
                          className="text-destructive"
                        >
                          <Trash2 className="mr-2 h-4 w-4" />
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

      {/* Create Dialog */}
      <Dialog open={showCreateDialog} onOpenChange={setShowCreateDialog}>
        <DialogContent className={newlyCreatedSecret ? "sm:max-w-lg" : ""}>
          <DialogHeader>
            <DialogTitle className="text-base">{newlyCreatedSecret ? "Webhook Created" : "Add Webhook"}</DialogTitle>
            <DialogDescription>
              {newlyCreatedSecret
                ? "Your webhook has been created. Save the signing secret below - it will only be shown once."
                : "Configure a webhook to receive event notifications"}
            </DialogDescription>
          </DialogHeader>

          {newlyCreatedSecret ? (
            <div className="py-4">
              <div className="bg-muted p-4 rounded-lg">
                <div className="flex items-center justify-between mb-2">
                  <Label className="text-xs font-medium">Signing Secret</Label>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => copyToClipboard(newlyCreatedSecret)}
                    className="h-7 gap-1 text-xs"
                  >
                    {copiedSecret ? (
                      <>
                        <Check className="h-3.5 w-3.5" />
                        Copied
                      </>
                    ) : (
                      <>
                        <Copy className="h-3.5 w-3.5" />
                        Copy
                      </>
                    )}
                  </Button>
                </div>
                <code className="block text-xs bg-background p-3 rounded border break-all font-mono">
                  {newlyCreatedSecret}
                </code>
              </div>
              <p className="text-xs text-muted-foreground mt-3">
                Use this secret to verify webhook signatures. The signature is included in the{" "}
                <code className="text-xs bg-muted px-1 rounded">X-TrueFlow-Signature</code> header.
              </p>
            </div>
          ) : (
            <div className="py-4 space-y-4">
              <div className="space-y-2">
                <Label htmlFor="webhook-url" className="text-xs">Webhook URL</Label>
                <Input
                  id="webhook-url"
                  type="url"
                  value={newWebhookUrl}
                  onChange={(e) => setNewWebhookUrl(e.target.value)}
                  placeholder="https://your-server.com/webhook"
                  className="h-9"
                />
              </div>
              <div className="space-y-2">
                <Label className="text-xs">Events to Subscribe</Label>
                <p className="text-xs text-muted-foreground">
                  Select which events to receive. If none selected, all events will be sent.
                </p>
                <div className="grid gap-2 sm:grid-cols-2">
                  {WEBHOOK_EVENT_TYPES.map((event) => (
                    <div key={event.value} className="flex items-center space-x-2">
                      <Checkbox
                        id={event.value}
                        checked={newWebhookEvents.includes(event.value)}
                        onCheckedChange={() => toggleEvent(event.value)}
                      />
                      <label htmlFor={event.value} className="text-xs cursor-pointer">
                        {event.label}
                      </label>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          )}

          <div className="flex justify-end gap-2">
            {newlyCreatedSecret ? (
              <Button size="sm" onClick={() => setShowCreateDialog(false)}>Done</Button>
            ) : (
              <>
                <Button variant="outline" size="sm" onClick={() => setShowCreateDialog(false)}>
                  Cancel
                </Button>
                <Button size="sm" onClick={handleCreate} disabled={isSubmitting}>
                  {isSubmitting ? (
                    <>
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                      Creating...
                    </>
                  ) : (
                    "Create"
                  )}
                </Button>
              </>
            )}
          </div>
        </DialogContent>
      </Dialog>

      {/* Delete Dialog */}
      <Dialog open={showDeleteDialog} onOpenChange={setShowDeleteDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle className="text-base">Delete Webhook</DialogTitle>
            <DialogDescription>
              Are you sure you want to delete this webhook? Event notifications will stop being sent to this URL.
            </DialogDescription>
          </DialogHeader>
          <div className="flex justify-end gap-2">
            <Button variant="outline" size="sm" onClick={() => setShowDeleteDialog(false)}>
              Cancel
            </Button>
            <Button variant="destructive" size="sm" onClick={handleDelete} disabled={isSubmitting}>
              {isSubmitting ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  Deleting...
                </>
              ) : (
                "Delete"
              )}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      {/* Test Dialog */}
      <Dialog open={showTestDialog} onOpenChange={setShowTestDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle className="text-base">Test Webhook</DialogTitle>
            <DialogDescription>
              Send a test event to verify your webhook endpoint is working correctly.
            </DialogDescription>
          </DialogHeader>
          <div className="py-4 space-y-4">
            <div className="space-y-2">
              <Label htmlFor="test-url" className="text-xs">Webhook URL</Label>
              <Input
                id="test-url"
                type="url"
                value={testUrl}
                onChange={(e) => setTestUrl(e.target.value)}
                placeholder="https://your-server.com/webhook"
                className="h-9"
              />
            </div>
            {testResult && (
              <div
                className={cn(
                  "p-3 rounded-lg",
                  testResult.success ? "bg-success/10 text-success" : "bg-destructive/10 text-destructive"
                )}
              >
                <p className="text-xs font-medium">{testResult.success ? "Success" : "Failed"}</p>
                <p className="text-xs mt-1">{testResult.message}</p>
              </div>
            )}
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="outline" size="sm" onClick={() => setShowTestDialog(false)}>
              Close
            </Button>
            <Button size="sm" onClick={handleTest} disabled={isTesting}>
              {isTesting ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  Sending...
                </>
              ) : (
                <>
                  <Send className="mr-2 h-4 w-4" />
                  Send Test
                </>
              )}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  )
}