"use client"

import { useState } from "react"
import { useRouter } from "next/navigation"
import { ArrowLeft } from "lucide-react"
import Link from "next/link"
import { Button } from "@/components/ui/button"
import { PolicyForm } from "@/components/policies/policy-form"
import { createPolicy, type CreatePolicyRequest, type UpdatePolicyRequest } from "@/lib/api"
import { useProject } from "@/contexts/project-context"
import { toast } from "sonner"

export default function NewPolicyPage() {
  const router = useRouter()
  const { selectedProject } = useProject()
  const [isSubmitting, setIsSubmitting] = useState(false)

  const handleSubmit = async (data: CreatePolicyRequest | UpdatePolicyRequest) => {
    if (!data.name) {
      throw new Error("Policy name is required")
    }
    setIsSubmitting(true)
    try {
      const response = await createPolicy({
        name: data.name,
        mode: data.mode,
        phase: data.phase,
        rules: data.rules || [],
        project_id: selectedProject?.id,
      })
      router.push(`/policies/${response.id}`)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to create policy")
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center gap-4">
          <Link href="/policies">
            <Button variant="ghost" size="icon-sm">
              <ArrowLeft className="h-4 w-4" />
            </Button>
          </Link>
          <div className="flex flex-col gap-1">
            <h1 className="text-2xl lg:text-3xl font-bold tracking-tight">
              Create Policy
            </h1>
            <p className="text-sm text-muted-foreground">
              Define rules for request validation, routing, and guardrails
            </p>
          </div>
        </div>

        {/* Form */}
        <div className="max-w-3xl">
          <PolicyForm onSubmit={handleSubmit} isSubmitting={isSubmitting} />
        </div>
      </div>
    </div>
  )
}