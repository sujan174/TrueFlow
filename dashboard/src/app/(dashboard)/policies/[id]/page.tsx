"use client"

import { useEffect, useState } from "react"
import { useRouter, useParams } from "next/navigation"
import { ArrowLeft, History, Loader2 } from "lucide-react"
import Link from "next/link"
import { Button } from "@/components/ui/button"
import { PolicyForm } from "@/components/policies/policy-form"
import {
  listPolicies,
  updatePolicy,
  getPolicyVersions,
  type PolicyRow,
  type PolicyVersionRow,
  type UpdatePolicyRequest,
} from "@/lib/api"
import { Badge } from "@/components/ui/badge"
import { toast } from "sonner"

export default function EditPolicyPage() {
  const router = useRouter()
  const params = useParams()
  const policyId = params.id as string

  const [policy, setPolicy] = useState<PolicyRow | null>(null)
  const [versions, setVersions] = useState<PolicyVersionRow[]>([])
  const [loading, setLoading] = useState(true)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [showHistory, setShowHistory] = useState(false)

  useEffect(() => {
    async function fetchPolicy() {
      try {
        // Fetch policies and find the one we need
        const policies = await listPolicies({ limit: 1000 })
        const found = policies.find((p) => p.id === policyId)
        if (!found) {
          router.push("/policies")
          return
        }
        setPolicy(found)

        // Fetch version history
        try {
          const versionHistory = await getPolicyVersions(policyId)
          setVersions(versionHistory)
        } catch {
          // Version history is optional
        }
      } catch (error) {
        toast.error(error instanceof Error ? error.message : "Failed to load policy")
        router.push("/policies")
      } finally {
        setLoading(false)
      }
    }
    fetchPolicy()
  }, [policyId, router])

  const handleSubmit = async (data: UpdatePolicyRequest) => {
    setIsSubmitting(true)
    try {
      await updatePolicy(policyId, data)
      // Refresh policy data
      const policies = await listPolicies({ limit: 1000 })
      const updated = policies.find((p) => p.id === policyId)
      if (updated) setPolicy(updated)

      // Refresh version history
      try {
        const versionHistory = await getPolicyVersions(policyId)
        setVersions(versionHistory)
      } catch {
        // Version history is optional
      }

      toast.success("Policy updated successfully!")
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to update policy")
    } finally {
      setIsSubmitting(false)
    }
  }

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
      </div>
    )
  }

  if (!policy) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <p className="text-muted-foreground">Policy not found</p>
      </div>
    )
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-4">
            <Link href="/policies">
              <Button variant="ghost" size="icon-sm">
                <ArrowLeft className="h-4 w-4" />
              </Button>
            </Link>
            <div className="flex flex-col gap-1">
              <h1 className="text-2xl lg:text-3xl font-bold tracking-tight">
                Edit Policy
              </h1>
              <p className="text-sm text-muted-foreground">
                {policy.name}
              </p>
            </div>
          </div>
          <Button
            variant="outline"
            onClick={() => setShowHistory(!showHistory)}
            className="gap-2"
          >
            <History className="h-4 w-4" />
            Version History ({versions.length})
          </Button>
        </div>

        <div className="grid gap-6 lg:grid-cols-3">
          {/* Form */}
          <div className="lg:col-span-2">
            <PolicyForm
              initialData={policy}
              onSubmit={handleSubmit}
              isSubmitting={isSubmitting}
            />
          </div>

          {/* Version History */}
          {showHistory && (
            <div className="space-y-4">
              <h2 className="text-lg font-semibold">Version History</h2>
              {versions.length === 0 ? (
                <p className="text-sm text-muted-foreground">No version history yet</p>
              ) : (
                <div className="space-y-2">
                  {versions.map((version) => (
                    <div
                      key={version.id}
                      className="p-3 border rounded-lg bg-muted/30"
                    >
                      <div className="flex items-center justify-between mb-2">
                        <Badge variant="outline">v{version.version}</Badge>
                        <span className="text-xs text-muted-foreground">
                          {new Date(version.created_at).toLocaleDateString()}
                        </span>
                      </div>
                      <p className="text-sm font-medium">{version.name}</p>
                      <p className="text-xs text-muted-foreground mt-1">
                        {version.rules.length} rule{version.rules.length !== 1 ? "s" : ""} • {version.mode} • {version.phase}
                      </p>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}