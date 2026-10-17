import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react"
import { useEffect } from "react"
import { useLocation, useNavigate } from "react-router"

import { Button } from "@/components/ui/button"

/**
 * Where the window is in its history. React Router keeps the entry's index
 * in `history.state`, and `history.length` counts the entries after it too.
 */
function position() {
  const index = (window.history.state as { idx?: number } | null)?.idx ?? 0
  return { back: index > 0, forward: index < window.history.length - 1 }
}

/**
 * Back and forward (design §7.1): a desktop window has no browser buttons,
 * and going back is how a list is found again with its page, sort and
 * filters.
 */
export function HistoryButtons() {
  const navigate = useNavigate()
  // Renders again on every navigation, so the buttons stay right
  useLocation()
  const { back, forward } = position()

  return (
    <div className="flex">
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label="Back"
        title="Back (⌘[)"
        disabled={!back}
        onClick={() => void navigate(-1)}
      >
        <ChevronLeftIcon />
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label="Forward"
        title="Forward (⌘])"
        disabled={!forward}
        onClick={() => void navigate(1)}
      >
        <ChevronRightIcon />
      </Button>
    </div>
  )
}

/** ⌘[ goes back and ⌘] goes forward, as in a browser. */
export function useHistoryShortcuts() {
  const navigate = useNavigate()
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (!event.metaKey || event.altKey || event.ctrlKey || event.shiftKey) {
        return
      }
      if (event.key === "[") {
        event.preventDefault()
        void navigate(-1)
      } else if (event.key === "]") {
        event.preventDefault()
        void navigate(1)
      }
    }
    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [navigate])
}
