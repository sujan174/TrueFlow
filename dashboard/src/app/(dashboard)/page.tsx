"use client"

import { useEffect, useState, useRef } from "react"
import { MetricCard } from "@/components/dashboard/metric-card"
import { VolumeChart } from "@/components/dashboard/volume-chart"
import { LatencyChart } from "@/components/dashboard/latency-chart"
import { TopTokensList } from "@/components/dashboard/top-tokens-list"
import { SpendCapsWidget } from "@/components/dashboard/spend-caps-widget"
import { Button } from "@/components/ui/button"
import Link from "next/link"
import {
  getAnalyticsSummary,
  getAnalyticsTimeseries,
  getTokenAnalytics,
  getTokenSpendCaps,
  getLatencyByProvider,
  formatNumber,
  formatCurrency,
  formatLatency,
} from "@/lib/api"
import type {
  AnalyticsSummary,
  AnalyticsTimeseriesPoint,
  TokenSummary,
  SpendCap,
  ProviderLatencyStat,
} from "@/lib/types/analytics"
import { RefreshCw, Activity, Timer, DollarSign, AlertTriangle, Key } from "lucide-react"
import { cn } from "@/lib/utils"

// Animated counter hook
function useAnimatedCounter(end: number, duration: number = 1000, loading: boolean = false) {
  const [count, setCount] = useState(0)
  const countRef = useRef(0)
  const startTimeRef = useRef<number | null>(null)

  useEffect(() => {
    if (loading) {
      setCount(0)
      return
    }

    const animate = (timestamp: number) => {
      if (!startTimeRef.current) startTimeRef.current = timestamp
      const progress = Math.min((timestamp - startTimeRef.current) / duration, 1)

      // Ease out expo
      const easeProgress = 1 - Math.pow(2, -10 * progress)
      const currentCount = Math.floor(easeProgress * end)

      if (currentCount !== countRef.current) {
        countRef.current = currentCount
        setCount(currentCount)
      }

      if (progress < 1) {
        requestAnimationFrame(animate)
      } else {
        setCount(end)
      }
    }

    requestAnimationFrame(animate)
  }, [end, duration, loading])

  return count
}

export default function DashboardPage() {
  const [loading, setLoading] = useState(true)
  const [summary, setSummary] = useState<AnalyticsSummary | null>(null)
  const [timeseries, setTimeseries] = useState<AnalyticsTimeseriesPoint[]>([])
  const [tokens, setTokens] = useState<TokenSummary[]>([])
  const [spendCaps, setSpendCaps] = useState<SpendCap[]>([])
  const [providerLatency, setProviderLatency] = useState<ProviderLatencyStat[]>([])
  const [mounted, setMounted] = useState(false)
  const [refreshing, setRefreshing] = useState(false)

  useEffect(() => {
    setMounted(true)
  }, [])

  const fetchData = async (isRefresh = false) => {
    if (isRefresh) setRefreshing(true)
    else setLoading(true)

    try {
      const [summaryData, timeseriesData, tokensData, capsData, latencyData] = await Promise.all([
        getAnalyticsSummary(168).catch(() => null),
        getAnalyticsTimeseries(168).catch(() => []),
        getTokenAnalytics().catch(() => []),
        getTokenSpendCaps().catch(() => []),
        getLatencyByProvider(168).catch(() => []),
      ])

      if (summaryData) setSummary(summaryData)
      setTimeseries(timeseriesData)
      setTokens(tokensData)
      setSpendCaps(capsData)
      setProviderLatency(latencyData)
    } catch (error) {
      console.error("Failed to fetch dashboard data:", error)
    } finally {
      setLoading(false)
      setRefreshing(false)
    }
  }

  useEffect(() => {
    fetchData()
  }, [])

  // Animated counters
  const animatedRequests = useAnimatedCounter(summary?.total_requests || 0, 1500, loading)
  const animatedErrors = useAnimatedCounter(summary?.error_count || 0, 1000, loading)

  const handleRefresh = () => {
    fetchData(true)
  }

  return (
    <div className="flex-1 flex flex-col">
      {/* Main Content */}
      <div className="flex-1 p-6 lg:p-8 flex flex-col gap-6 overflow-auto">
        {/* Header Row */}
        <div className="flex items-center justify-between flex-wrap gap-4">
          <div
            className={cn(
              "flex flex-col gap-1 transition-all duration-700",
              mounted ? "opacity-100 translate-y-0" : "opacity-0 -translate-y-4"
            )}
          >
            <span className="text-[11px] font-semibold tracking-[2px] text-primary uppercase">
              GATEWAY OVERVIEW
            </span>
            <h1 className="text-2xl lg:text-3xl font-bold tracking-tight">
              Mission Control
            </h1>
          </div>
          <div className="flex items-center gap-3">
            <Button
              variant="outline"
              size="sm"
              onClick={handleRefresh}
              loading={refreshing}
              className="gap-2"
            >
              <RefreshCw className={cn("h-4 w-4", refreshing && "animate-spin")} />
              Refresh
            </Button>
          </div>
        </div>

        {/* Metric Ribbon */}
        <div
          className={cn(
            "flex flex-wrap items-center gap-4 lg:gap-8 bg-card border rounded-xl px-4 lg:px-6 py-4 shadow-sm transition-all duration-700",
            mounted ? "opacity-100 translate-y-0" : "opacity-0 translate-y-4",
            "stagger-1"
          )}
        >
          <MetricCard
            label="TOTAL REQUESTS"
            value={loading ? "..." : formatNumber(animatedRequests)}
            icon={Activity}
            variant="requests"
            href="/analytics?tab=Traffic"
          />
          <div className="hidden lg:block w-px h-10 bg-border" />
          <MetricCard
            label="AVG LATENCY"
            value={loading ? "..." : summary ? formatLatency(summary.avg_latency) : "0ms"}
            icon={Timer}
            variant="latency"
            href="/analytics?tab=Overview"
          />
          <div className="hidden lg:block w-px h-10 bg-border" />
          <MetricCard
            label="TOTAL SPEND"
            value={loading ? "..." : summary ? formatCurrency(summary.total_cost) : "$0.00"}
            icon={DollarSign}
            variant="spend"
            href="/analytics?tab=Cost"
          />
          <div className="hidden lg:block w-px h-10 bg-border" />
          <MetricCard
            label="VIOLATIONS"
            value={loading ? "..." : animatedErrors.toString()}
            icon={AlertTriangle}
            variant="alerts"
            href="/analytics?tab=Security"
          />
          <div className="hidden lg:block w-px h-10 bg-border" />
          <MetricCard
            label="ACTIVE TOKENS"
            value={loading ? "..." : tokens.length.toString()}
            icon={Key}
            variant="tokens"
            href="/analytics?tab=UsersTokens"
          />
        </div>

        {/* Charts Row */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
          {/* Volume Chart */}
          <div
            className={cn(
              "lg:col-span-2 bg-card border rounded-xl flex flex-col shadow-sm transition-all duration-700",
              mounted ? "opacity-100 translate-y-0" : "opacity-0 translate-y-4",
              "stagger-2"
            )}
          >
            <div className="h-12 px-5 flex items-center justify-between border-b">
              <div className="flex items-center gap-2">
                <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
                  REQUEST VOLUME
                </span>
                <span className="text-[10px] font-mono text-muted-foreground">7 days</span>
              </div>
            </div>
            <div className="flex-1 p-5 flex items-center justify-center min-h-[280px]">
              <VolumeChart data={timeseries} loading={loading} />
            </div>
          </div>

          {/* Latency Chart */}
          <div
            className={cn(
              "bg-card border rounded-xl flex flex-col shadow-sm transition-all duration-700",
              mounted ? "opacity-100 translate-y-0" : "opacity-0 translate-y-4",
              "stagger-3"
            )}
          >
            <div className="h-11 px-4 flex items-center justify-between border-b">
              <div className="flex items-center gap-2">
                <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
                  LATENCY BY PROVIDER
                </span>
                <span className="text-[10px] font-mono text-muted-foreground">7 days</span>
              </div>
            </div>
            <div className="flex-1 p-4 min-h-[280px]">
              <LatencyChart data={providerLatency} loading={loading} />
            </div>
          </div>
        </div>

        {/* Bottom Row */}
        <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-4">
          {/* Top Tokens */}
          <div
            className={cn(
              "bg-card border rounded-xl flex flex-col shadow-sm transition-all duration-700",
              mounted ? "opacity-100 translate-y-0" : "opacity-0 translate-y-4",
              "stagger-4"
            )}
          >
            <div className="h-12 px-5 flex items-center justify-between border-b">
              <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
                TOP TOKENS BY ACTIVITY
              </span>
              <span className="text-xs text-muted-foreground hover:text-foreground cursor-pointer">
                View all →
              </span>
            </div>
            <div className="flex-1 p-5 overflow-auto min-h-[260px]">
              <TopTokensList tokens={tokens} loading={loading} />
            </div>
          </div>

          {/* At Risk of Capping */}
          <div
            className={cn(
              "bg-card border rounded-xl flex flex-col shadow-sm transition-all duration-700",
              mounted ? "opacity-100 translate-y-0" : "opacity-0 translate-y-4",
              "stagger-5"
            )}
          >
            <div className="h-12 px-5 flex items-center border-b">
              <span className="text-[10px] font-semibold tracking-[1.5px] text-muted-foreground uppercase">
                AT RISK OF CAPPING
              </span>
            </div>
            <div className="flex-1 p-5 overflow-auto min-h-[260px]">
              <SpendCapsWidget caps={spendCaps} loading={loading} />
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}