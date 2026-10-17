import { NavLink } from "react-router"

import { cn } from "@/lib/utils"

/** Tabs that are routes (slunk's design §7.2: each tab is a route). */
export function TabLinks({
  tabs,
}: {
  tabs: { label: string; to: string; count?: number }[]
}) {
  return (
    <nav className="flex gap-1 text-xs">
      {tabs.map((tab) => (
        <NavLink
          key={tab.to}
          to={tab.to}
          end
          className={({ isActive }) =>
            cn(
              "border-b-2 px-2 py-1 text-muted-foreground transition-colors hover:text-foreground",
              isActive
                ? "border-primary font-medium text-foreground"
                : "border-transparent"
            )
          }
        >
          {tab.label}
          {tab.count !== undefined && (
            <span className="ml-1 text-muted-foreground tabular-nums">
              {tab.count}
            </span>
          )}
        </NavLink>
      ))}
    </nav>
  )
}
