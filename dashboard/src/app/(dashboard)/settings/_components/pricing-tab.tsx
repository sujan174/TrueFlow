"use client"

import { useState, useEffect } from "react"
import { toast } from "sonner"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
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
import { Loader2, Plus, MoreHorizontal, Pencil, Trash2, DollarSign } from "lucide-react"
import {
  listPricing,
  upsertPricing,
  deletePricing,
  type PricingEntry,
  type UpsertPricingRequest,
} from "@/lib/api"
import { cn } from "@/lib/utils"

function formatDate(dateString: string): string {
  return new Date(dateString).toLocaleDateString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
  })
}

export function PricingTab() {
  const [pricing, setPricing] = useState<PricingEntry[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [showDialog, setShowDialog] = useState(false)
  const [showDeleteDialog, setShowDeleteDialog] = useState(false)
  const [editingEntry, setEditingEntry] = useState<PricingEntry | null>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [formData, setFormData] = useState<UpsertPricingRequest>({
    provider: "",
    model_pattern: "",
    input_per_m: 0,
    output_per_m: 0,
  })

  useEffect(() => {
    loadPricing()
  }, [])

  async function loadPricing() {
    try {
      const data = await listPricing()
      setPricing(data)
    } catch (error) {
      toast.error("Failed to load pricing")
      console.error(error)
    } finally {
      setIsLoading(false)
    }
  }

  function openCreateDialog() {
    setEditingEntry(null)
    setFormData({ provider: "", model_pattern: "", input_per_m: 0, output_per_m: 0 })
    setShowDialog(true)
  }

  function openEditDialog(entry: PricingEntry) {
    setEditingEntry(entry)
    setFormData({
      provider: entry.provider,
      model_pattern: entry.model_pattern,
      input_per_m: Number(entry.input_per_m),
      output_per_m: Number(entry.output_per_m),
    })
    setShowDialog(true)
  }

  function openDeleteDialog(entry: PricingEntry) {
    setEditingEntry(entry)
    setShowDeleteDialog(true)
  }

  async function handleSubmit() {
    if (!formData.provider || !formData.model_pattern) {
      toast.error("Provider and model pattern are required")
      return
    }

    setIsSubmitting(true)
    try {
      await upsertPricing(formData)
      toast.success(editingEntry ? "Pricing rule updated" : "Pricing rule created")
      setShowDialog(false)
      await loadPricing()
    } catch (error) {
      toast.error("Failed to save pricing rule")
      console.error(error)
    } finally {
      setIsSubmitting(false)
    }
  }

  async function handleDelete() {
    if (!editingEntry) return

    setIsSubmitting(true)
    try {
      await deletePricing(editingEntry.id)
      toast.success("Pricing rule deleted")
      setShowDeleteDialog(false)
      setEditingEntry(null)
      await loadPricing()
    } catch (error) {
      toast.error("Failed to delete pricing rule")
      console.error(error)
    } finally {
      setIsSubmitting(false)
    }
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
          <h3 className="text-sm font-medium">Pricing Rules</h3>
          <p className="text-xs text-muted-foreground mt-0.5">
            Custom cost calculation rules for model usage
          </p>
        </div>
        <Button size="sm" onClick={openCreateDialog} className="gap-2">
          <Plus className="h-4 w-4" />
          Add Rule
        </Button>
      </div>

      {/* Pricing Table */}
      <div className="border rounded-lg">
        {pricing.length === 0 ? (
          <div className="p-12 text-center">
            <div className="w-12 h-12 rounded-full bg-muted flex items-center justify-center mx-auto mb-4">
              <DollarSign className="h-6 w-6 text-muted-foreground" />
            </div>
            <p className="text-sm font-medium mb-1">No custom pricing rules</p>
            <p className="text-xs text-muted-foreground">
              Default provider pricing will be used
            </p>
          </div>
        ) : (
          <table className="w-full">
            <thead>
              <tr className="border-b bg-muted/30">
                <th className="text-left px-4 py-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  Provider
                </th>
                <th className="text-left px-4 py-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  Model Pattern
                </th>
                <th className="text-right px-4 py-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  Input $/M
                </th>
                <th className="text-right px-4 py-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  Output $/M
                </th>
                <th className="text-center px-4 py-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  Status
                </th>
                <th className="w-12"></th>
              </tr>
            </thead>
            <tbody className="divide-y">
              {pricing.map((entry) => (
                <tr key={entry.id} className="hover:bg-muted/30 transition-colors">
                  <td className="px-4 py-3">
                    <span className="text-sm font-medium">{entry.provider}</span>
                  </td>
                  <td className="px-4 py-3">
                    <code className="text-xs bg-muted px-2 py-1 rounded font-mono">
                      {entry.model_pattern}
                    </code>
                  </td>
                  <td className="px-4 py-3 text-right">
                    <span className="text-xs font-mono tabular-nums">${Number(entry.input_per_m).toFixed(4)}</span>
                  </td>
                  <td className="px-4 py-3 text-right">
                    <span className="text-xs font-mono tabular-nums">${Number(entry.output_per_m).toFixed(4)}</span>
                  </td>
                  <td className="px-4 py-3 text-center">
                    <span className={cn(
                      "inline-flex px-2 py-0.5 text-xs font-medium rounded-full",
                      entry.is_active ? "bg-success/10 text-success" : "bg-muted text-muted-foreground"
                    )}>
                      {entry.is_active ? "Active" : "Inactive"}
                    </span>
                  </td>
                  <td className="px-4 py-3">
                    <DropdownMenu>
                      <DropdownMenuTrigger>
                        <Button variant="ghost" size="icon-sm">
                          <MoreHorizontal className="h-4 w-4" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem onClick={() => openEditDialog(entry)}>
                          <Pencil className="mr-2 h-4 w-4" />
                          Edit
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          onClick={() => openDeleteDialog(entry)}
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

      {/* Create/Edit Dialog */}
      <Dialog open={showDialog} onOpenChange={setShowDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle className="text-base">{editingEntry ? "Edit Pricing Rule" : "Add Pricing Rule"}</DialogTitle>
            <DialogDescription>
              Define custom pricing for a model pattern. The pattern is a regex that matches model names.
            </DialogDescription>
          </DialogHeader>
          <div className="py-4 space-y-4">
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="provider" className="text-xs">Provider</Label>
                <Input
                  id="provider"
                  value={formData.provider}
                  onChange={(e) => setFormData({ ...formData, provider: e.target.value })}
                  placeholder="openai, anthropic, etc."
                  className="h-9"
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="model_pattern" className="text-xs">Model Pattern (regex)</Label>
                <Input
                  id="model_pattern"
                  value={formData.model_pattern}
                  onChange={(e) => setFormData({ ...formData, model_pattern: e.target.value })}
                  placeholder="gpt-4.*"
                  className="h-9"
                />
              </div>
            </div>
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="input_per_m" className="text-xs">Input Cost ($/M tokens)</Label>
                <Input
                  id="input_per_m"
                  type="number"
                  step="0.0001"
                  value={formData.input_per_m}
                  onChange={(e) => setFormData({ ...formData, input_per_m: parseFloat(e.target.value) || 0 })}
                  className="h-9"
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="output_per_m" className="text-xs">Output Cost ($/M tokens)</Label>
                <Input
                  id="output_per_m"
                  type="number"
                  step="0.0001"
                  value={formData.output_per_m}
                  onChange={(e) => setFormData({ ...formData, output_per_m: parseFloat(e.target.value) || 0 })}
                  className="h-9"
                />
              </div>
            </div>
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="outline" size="sm" onClick={() => setShowDialog(false)}>
              Cancel
            </Button>
            <Button size="sm" onClick={handleSubmit} disabled={isSubmitting}>
              {isSubmitting ? (
                <>
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  Saving...
                </>
              ) : (
                "Save"
              )}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      {/* Delete Confirmation Dialog */}
      <Dialog open={showDeleteDialog} onOpenChange={setShowDeleteDialog}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle className="text-base">Delete Pricing Rule</DialogTitle>
            <DialogDescription>
              Are you sure you want to delete this pricing rule? The system will fall back to default pricing for matching models.
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
    </div>
  )
}