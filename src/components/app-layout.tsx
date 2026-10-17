import { Navigate, Outlet } from "react-router"

import { AppSidebar } from "@/components/app-sidebar"
import { useHistoryShortcuts } from "@/components/history-buttons"
import { QueryError } from "@/components/query-error"
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"
import { useCommand } from "@/lib/query"

/**
 * The app shell (slunk's design §7.1): the sidebar, and the page beside it. Without
 * an open library, it sends the user to Welcome (design §6.1). It's keyed by
 * the library, so switching libraries mounts every page again.
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
      <AppSidebar />
      {/* min-w-0 lets wide tables scroll inside the page, not widen it */}
      <SidebarInset className="min-w-0">
        <Outlet />
      </SidebarInset>
    </SidebarProvider>
  )
}
