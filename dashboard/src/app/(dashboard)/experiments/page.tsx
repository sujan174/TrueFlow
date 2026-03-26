"use client"

import { useEffect, useState, useRef } from "react"
import { useRouter } from "next/navigation"
import { Plus, FlaskConical, MoreHorizontal, Eye, Square } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { toast } from "sonner"
import {
  listExperiments,
  stopExperiment,
  type Experiment,
} from "@/lib/api"
import { formatRelativeTime } from "@/lib/utils"

function StatusBadge({ status }: { status: string }) {
  const isRunning = status === "running"
  return (
    <Badge
      variant={isRunning ? "success" : "secondary"}
      className="text-[10px]"
    >
      {isRunning ? "Running" : "Stopped"}
    </Badge>
  )
}

export default function ExperimentsPage() {
  const router = useRouter()
  const [experiments, setExperiments] = useState<Experiment[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const mountedRef = useRef(true)

  useEffect(() => {
    mountedRef.current = true

    const fetchExperiments = async () => {
      try {
        const data = await listExperiments()
        if (mountedRef.current) {
          setExperiments(data)
        }
      } catch (err) {
        if (mountedRef.current) {
          setError(err instanceof Error ? err.message : "Failed to load experiments")
        }
      } finally {
        if (mountedRef.current) {
          setLoading(false)
        }
      }
    }

    fetchExperiments()

    // Cleanup function to track unmount
    return () => {
      mountedRef.current = false
    }
  }, [])

  const handleStop = async (id: string) => {
    try {
      await stopExperiment(id)
      setExperiments(experiments.map((e) =>
        e.id === id ? { ...e, status: "stopped" } : e
      ))
      toast.success("Experiment stopped")
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to stop experiment")
    }
  }

  return (
    <div className="flex-1 flex flex-col min-w-0">
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-5 overflow-auto bg-background">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex flex-col gap-1">
            <h1 className="text-2xl lg:text-3xl font-bold tracking-tight">
              Experiments
            </h1>
            <p className="text-sm text-muted-foreground">
              A/B testing for models, prompts, and parameters
            </p>
          </div>
          <Button className="gap-2" onClick={() => router.push("/experiments/new")}>
            <Plus className="h-4 w-4" />
            Create Experiment
          </Button>
        </div>

        {/* Table */}
        <div className="bg-card border rounded-xl shadow-sm overflow-hidden">
          {loading ? (
            <div className="p-8 text-center text-muted-foreground">
              Loading experiments...
            </div>
          ) : error ? (
            <div className="p-8 text-center text-destructive">{error}</div>
          ) : experiments.length === 0 ? (
            <div className="p-8 text-center">
              <FlaskConical className="h-12 w-12 mx-auto text-muted-foreground/50 mb-4" />
              <p className="text-muted-foreground">No experiments yet</p>
              <p className="text-sm text-muted-foreground/70 mt-1">
                Create your first A/B test to compare model performance
              </p>
            </div>
          ) : (
            <table className="w-full">
              <thead className="bg-muted/50 border-b">
                <tr className="text-[10px] font-semibold tracking-[1px] text-muted-foreground uppercase">
                  <th className="px-4 py-3 text-left">Name</th>
                  <th className="px-4 py-3 text-left">Status</th>
                  <th className="px-4 py-3 text-left">Variants</th>
                  <th className="px-4 py-3 text-left">Created</th>
                  <th className="px-4 py-3 text-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {experiments.map((experiment) => {
                  const variants = experiment.variants || []
                  return (
                    <tr
                      key={experiment.id}
                      className="border-b last:border-0 hover:bg-muted/30 transition-colors cursor-pointer"
                      onClick={() => router.push(`/experiments/${experiment.id}`)}
                    >
                      <td className="px-4 py-3">
                        <span className="text-sm font-medium">{experiment.name}</span>
                      </td>
                      <td className="px-4 py-3">
                        <StatusBadge status={experiment.status} />
                      </td>
                      <td className="px-4 py-3">
                        <span className="text-sm text-muted-foreground">
                          {variants.length} variant{variants.length !== 1 ? "s" : ""}
                        </span>
                      </td>
                      <td className="px-4 py-3">
                        <span className="text-sm text-muted-foreground">
                          {formatRelativeTime(experiment.created_at)}
                        </span>
                      </td>
                      <td className="px-4 py-3 text-right">
                        <DropdownMenu>
                          <DropdownMenuTrigger onClick={(e) => e.stopPropagation()}>
                            <Button variant="ghost" size="icon-sm">
                              <MoreHorizontal className="h-4 w-4" />
                            </Button>
                          </DropdownMenuTrigger>
                          <DropdownMenuContent align="end">
                            <DropdownMenuItem onClick={() => router.push(`/experiments/${experiment.id}`)}>
                              <Eye className="h-4 w-4 mr-2" />
                              View Results
                            </DropdownMenuItem>
                            {experiment.status === "running" && (
                              <DropdownMenuItem
                                className="text-destructive"
                                onClick={(e) => {
                                  e.stopPropagation()
                                  handleStop(experiment.id)
                                }}
                              >
                                <Square className="h-4 w-4 mr-2" />
                                Stop
                              </DropdownMenuItem>
                            )}
                          </DropdownMenuContent>
                        </DropdownMenu>
                      </td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </div>
  )
}