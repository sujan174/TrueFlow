"use client"

import { cn } from "@/lib/utils"

const tabs = [
  { name: "Overview", id: "Overview" },
  { name: "Traffic", id: "Traffic" },
  { name: "Cost", id: "Cost" },
  { name: "Users & Tokens", id: "UsersTokens" },
  { name: "Cache", id: "Cache" },
  { name: "Models", id: "Models" },
  { name: "Security", id: "Security" },
  { name: "HITL", id: "HITL" },
  { name: "Errors", id: "Errors" },
]

interface TabBarProps {
  activeTab: string
  onTabChange?: (tabId: string) => void
}

export function TabBar({ activeTab, onTabChange }: TabBarProps) {
  return (
    <div className="flex items-center gap-0.5 p-1 bg-card/50 backdrop-blur-sm border rounded-xl overflow-x-auto scrollbar-thin">
      {tabs.map((tab) => {
        const isActive = tab.id === activeTab
        return (
          <button
            key={tab.id}
            onClick={() => onTabChange?.(tab.id)}
            className={cn(
              "relative px-4 py-2 rounded-lg text-[13px] font-medium transition-all duration-200 whitespace-nowrap focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
              isActive
                ? "bg-primary text-primary-foreground font-semibold shadow-sm"
                : "text-muted-foreground hover:text-foreground hover:bg-muted/80"
            )}
            type="button"
          >
            {tab.name}
          </button>
        )
      })}
    </div>
  )
}