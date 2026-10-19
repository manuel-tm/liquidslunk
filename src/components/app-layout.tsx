import { Navigate, Outlet } from "react-router"

import { AppSidebar } from "@/components/app-sidebar"
import { useHistoryShortcuts } from "@/components/history-buttons"
import { PlayerProvider } from "@/components/player/provider"
import { QueryError } from "@/components/query-error"
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"
import { useCommand } from "@/lib/query"

/**
 * The app shell: the sidebar, and the page beside it, inside the player
 * (2_player.md §4.1). Without an open library, it sends the user to Welcome
 * (design §6.1). It's keyed by the library, so switching libraries mounts
 * every page and the player again.
 */
export function AppLayout() {
  const status = useCommand("libraryStatus")
  useHistoryShortcuts()

  if (status.error) {
    return (
      <main className="p-6">
        <QueryError error={status.error} />
      </main>
    )
  }
  if (!status.data) return null
  if (!status.data.library) return <Navigate to="/library" replace />

  return (
    <SidebarProvider key={status.data.library.path}>
      <PlayerProvider>
        <AppSidebar />
        {/* min-w-0 lets wide tables scroll inside the page, not widen it */}
        <SidebarInset className="min-w-0">
          <Outlet />
        </SidebarInset>
      </PlayerProvider>
    </SidebarProvider>
  )
}
