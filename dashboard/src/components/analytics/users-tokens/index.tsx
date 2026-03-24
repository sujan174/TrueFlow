"use client"

import { UserGrowthChart } from "./user-growth-chart"
import { ActiveTokensCard } from "./active-tokens-card"
import { RateLimitAlert } from "./rate-limit-alert"
import { RequestsPerUserChart } from "./requests-per-user-chart"
import { EngagementTiersCard } from "./engagement-tiers-card"
import { CTARow } from "./cta-row"

export function UsersTokensTabContent() {
  return (
    <div className="flex-1 flex flex-col gap-6">
      {/* Top Row: User Growth + Active Tokens + Rate Limit Alert */}
      <div className="h-[280px] flex gap-4">
        <div className="flex-1">
          <UserGrowthChart />
        </div>
        <div className="w-[240px] flex flex-col gap-4">
          <ActiveTokensCard />
          <RateLimitAlert />
        </div>
      </div>

      {/* Middle Row: Requests Per User + Engagement Tiers */}
      <div className="h-[280px] flex gap-4">
        <div className="flex-1">
          <RequestsPerUserChart />
        </div>
        <div className="w-[320px]">
          <EngagementTiersCard />
        </div>
      </div>

      {/* CTA Row */}
      <CTARow />
    </div>
  )
}

export {
  UserGrowthChart,
  ActiveTokensCard,
  RateLimitAlert,
  RequestsPerUserChart,
  EngagementTiersCard,
  CTARow,
}