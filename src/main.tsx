import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import { ApolloProvider } from "@apollo/client/react"
import { QueryClientProvider } from "@tanstack/react-query"
import { RouterProvider } from "react-router/dom"

import "./index.css"
import { ThemeProvider } from "@/components/theme-provider.tsx"
import { Toaster } from "@/components/ui/sonner.tsx"
import { TooltipProvider } from "@/components/ui/tooltip.tsx"
import { apollo } from "@/lib/apollo.ts"
import { queryClient, whenNoLibrary } from "@/lib/query.ts"
import { router } from "@/router.tsx"

// A command that needs a library says when none is open: go to Welcome
whenNoLibrary(() => void router.navigate("/library"))

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider>
      <QueryClientProvider client={queryClient}>
        <ApolloProvider client={apollo}>
          <TooltipProvider>
            <RouterProvider router={router} />
            <Toaster />
          </TooltipProvider>
        </ApolloProvider>
      </QueryClientProvider>
    </ThemeProvider>
  </StrictMode>
)
