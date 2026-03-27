"use client"

import { useState } from "react"
import { useRouter } from "next/navigation"
import { ArrowLeft, Plus, Trash2, AlertCircle } from "lucide-react"
import Link from "next/link"
import { Button } from "@/components/ui/button"
import { toast } from "sonner"
import {
  createExperiment,
  type ExperimentVariant,
} from "@/lib/api"

interface VariantInput {
  id: string
  name: string
  weight: number
  model: string
  bodyFields: string
  bodyFieldsError?: string
}

function generateId(): string {
  return Math.random().toString(36).substring(2, 9)
}

// Validate JSON string and return error message if invalid
function validateJson(value: string): string | undefined {
  if (!value.trim()) return undefined
  try {
    JSON.parse(value)
    return undefined
  } catch {
    return "Invalid JSON format"
  }
}

export default function NewExperimentPage() {
  const router = useRouter()
  const [name, setName] = useState("")
  const [nameError, setNameError] = useState<string | undefined>()
  const [variants, setVariants] = useState<VariantInput[]>([
    { id: generateId(), name: "control", weight: 50, model: "", bodyFields: "" },
    { id: generateId(), name: "treatment", weight: 50, model: "", bodyFields: "" },
  ])
  const [isSubmitting, setIsSubmitting] = useState(false)

  const addVariant = () => {
    setVariants([
      ...variants,
      { id: generateId(), name: `variant-${variants.length}`, weight: 0, model: "", bodyFields: "" },
    ])
  }

  const removeVariant = (id: string) => {
    if (variants.length <= 2) {
      toast.error("An experiment must have at least 2 variants")
      return
    }
    setVariants(variants.filter((v) => v.id !== id))
  }

  const updateVariant = (id: string, field: keyof VariantInput, value: string | number) => {
    setVariants(variants.map((v): VariantInput => {
      if (v.id !== id) return v

      // Validate JSON on bodyFields change
      if (field === "bodyFields") {
        const jsonError = validateJson(value as string)
        return { ...v, [field]: value, bodyFieldsError: jsonError } as VariantInput
      }

      return { ...v, [field]: value } as VariantInput
    }))
  }

  // Validate experiment name
  const validateName = (value: string) => {
    if (value.includes("__experiment__")) {
      setNameError("Name cannot contain '__experiment__'")
    } else if (value.length > 100) {
      setNameError("Name must be 100 characters or less")
    } else {
      setNameError(undefined)
    }
  }

  const handleNameChange = (value: string) => {
    setName(value)
    validateName(value)
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()

    if (!name.trim()) {
      toast.error("Please enter an experiment name")
      return
    }

    if (nameError) {
      toast.error(nameError)
      return
    }

    if (variants.length < 2) {
      toast.error("An experiment must have at least 2 variants")
      return
    }

    // Validate variants have names
    const emptyNames = variants.filter((v) => !v.name.trim())
    if (emptyNames.length > 0) {
      toast.error("All variants must have a name")
      return
    }

    // Check for JSON errors
    const jsonErrors = variants.filter((v) => v.bodyFieldsError)
    if (jsonErrors.length > 0) {
      toast.error("Please fix JSON errors before submitting")
      return
    }

    // Validate weights are non-zero
    const zeroWeights = variants.filter((v) => v.weight === 0)
    if (zeroWeights.length > 0) {
      toast.error("All variants must have a weight greater than 0")
      return
    }

    // Build the request
    const experimentVariants: ExperimentVariant[] = variants.map((v) => {
      const variant: ExperimentVariant = {
        name: v.name.trim(),
        weight: v.weight,
      }

      if (v.model.trim()) {
        variant.model = v.model.trim()
      }

      if (v.bodyFields.trim()) {
        variant.set_body_fields = JSON.parse(v.bodyFields)
      }

      return variant
    })

    setIsSubmitting(true)
    try {
      const experiment = await createExperiment({
        name: name.trim(),
        variants: experimentVariants,
      })
      toast.success("Experiment created successfully")
      router.push(`/experiments/${experiment.id}`)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to create experiment")
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center gap-4">
          <Link href="/experiments">
            <Button variant="ghost" size="icon-sm">
              <ArrowLeft className="h-4 w-4" />
            </Button>
          </Link>
          <div className="flex flex-col gap-1">
            <h1 className="text-2xl lg:text-3xl font-bold tracking-tight">
              New Experiment
            </h1>
            <p className="text-sm text-muted-foreground">
              Create an A/B test to compare model performance
            </p>
          </div>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="max-w-2xl space-y-6">
          {/* Name Field */}
          <div className="space-y-2">
            <label className="text-sm font-medium">
              Experiment Name
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => handleNameChange(e.target.value)}
              className={`w-full px-3 py-2 text-sm border rounded-lg bg-background ${nameError ? 'border-destructive' : ''}`}
              placeholder="e.g., gpt4-vs-claude-coding"
              required
            />
            {nameError ? (
              <p className="text-xs text-destructive flex items-center gap-1">
                <AlertCircle className="h-3 w-3" />
                {nameError}
              </p>
            ) : (
              <p className="text-xs text-muted-foreground">
                A unique name to identify this experiment (max 100 characters)
              </p>
            )}
          </div>

          {/* Variants Section */}
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <label className="text-sm font-medium">Variants</label>
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={addVariant}
                className="gap-1"
              >
                <Plus className="h-3 w-3" />
                Add Variant
              </Button>
            </div>

            <div className="space-y-3">
              {variants.map((variant, index) => (
                <div
                  key={variant.id}
                  className="bg-muted/30 border rounded-lg p-4 space-y-3"
                >
                  <div className="flex items-center justify-between">
                    <span className="text-xs font-semibold text-muted-foreground uppercase">
                      Variant {index + 1}
                    </span>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-xs"
                      onClick={() => removeVariant(variant.id)}
                      className="text-muted-foreground hover:text-destructive"
                    >
                      <Trash2 className="h-3 w-3" />
                    </Button>
                  </div>

                  <div className="grid grid-cols-2 gap-3">
                    <div className="space-y-1">
                      <label className="text-xs text-muted-foreground">Name</label>
                      <input
                        type="text"
                        value={variant.name}
                        onChange={(e) => updateVariant(variant.id, "name", e.target.value)}
                        className="w-full px-2 py-1.5 text-sm border rounded bg-background"
                        placeholder="control"
                        required
                      />
                    </div>
                    <div className="space-y-1">
                      <label className="text-xs text-muted-foreground">Weight (%)</label>
                      <input
                        type="number"
                        min="1"
                        max="100"
                        value={variant.weight}
                        onChange={(e) => updateVariant(variant.id, "weight", parseInt(e.target.value) || 0)}
                        className={`w-full px-2 py-1.5 text-sm border rounded bg-background ${variant.weight === 0 ? 'border-destructive' : ''}`}
                        placeholder="50"
                        required
                      />
                    </div>
                  </div>

                  <div className="space-y-1">
                    <label className="text-xs text-muted-foreground">Model Override (optional)</label>
                    <input
                      type="text"
                      value={variant.model}
                      onChange={(e) => updateVariant(variant.id, "model", e.target.value)}
                      className="w-full px-2 py-1.5 text-sm border rounded bg-background"
                      placeholder="e.g., gpt-4o, claude-3-5-sonnet"
                    />
                  </div>

                  <div className="space-y-1">
                    <label className="text-xs text-muted-foreground">Body Fields Override (JSON, optional)</label>
                    <textarea
                      value={variant.bodyFields}
                      onChange={(e) => updateVariant(variant.id, "bodyFields", e.target.value)}
                      className={`w-full px-2 py-1.5 text-sm border rounded bg-background font-mono text-xs ${variant.bodyFieldsError ? 'border-destructive' : ''}`}
                      placeholder='{"temperature": 0.7}'
                      rows={2}
                    />
                    {variant.bodyFieldsError && (
                      <p className="text-xs text-destructive flex items-center gap-1">
                        <AlertCircle className="h-3 w-3" />
                        {variant.bodyFieldsError}
                      </p>
                    )}
                  </div>
                </div>
              ))}
            </div>

            <p className="text-xs text-muted-foreground">
              Traffic is split based on relative weights. The variant is selected deterministically based on request ID.
            </p>
          </div>

          {/* Submit Buttons */}
          <div className="flex items-center gap-3 pt-4">
            <Button type="submit" disabled={isSubmitting}>
              {isSubmitting ? "Creating..." : "Create Experiment"}
            </Button>
            <Link href="/experiments">
              <Button type="button" variant="outline">
                Cancel
              </Button>
            </Link>
          </div>
        </form>
      </div>
    </div>
  )
}