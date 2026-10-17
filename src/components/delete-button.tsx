import { useState } from "react"
import { useNavigate } from "react-router"
import { toast } from "sonner"

import { QueryError } from "@/components/query-error"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog"
import { Button } from "@/components/ui/button"

/**
 * The read page's Delete button and its confirmation (slunk's design §7.2). While
 * something blocks the delete, the dialog says what instead.
 */
export function DeleteButton({
  name,
  blockers,
  onDelete,
  then,
  detail,
}: {
  /** What's deleted, as the dialog names it */
  name: string
  /** What keeps it from being deleted, from the backend */
  blockers: string[]
  onDelete: () => Promise<unknown>
  /** Where to go afterwards: the list */
  then: string
  /** More about what deleting does */
  detail?: string
}) {
  const navigate = useNavigate()
  const [error, setError] = useState<Error | null>(null)
  const [busy, setBusy] = useState(false)
  const blocked = blockers.length > 0

  return (
    <AlertDialog onOpenChange={() => setError(null)}>
      <AlertDialogTrigger render={<Button variant="destructive" />}>
        Delete
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            {blocked ? capitalize(`${name} is in use`) : `Delete ${name}?`}
          </AlertDialogTitle>
          <AlertDialogDescription>
            {blocked
              ? "It can't be deleted while:"
              : (detail ?? "This can't be undone.")}
          </AlertDialogDescription>
        </AlertDialogHeader>
        {blocked && (
          <ul className="flex list-disc flex-col gap-1 pl-4 text-xs">
            {blockers.map((blocker) => (
              <li key={blocker}>{blocker}</li>
            ))}
          </ul>
        )}
        {error && <QueryError error={error} title="Can't delete it" />}
        <AlertDialogFooter>
          <AlertDialogCancel>{blocked ? "Close" : "Cancel"}</AlertDialogCancel>
          {!blocked && (
            <AlertDialogAction
              variant="destructive"
              disabled={busy}
              onClick={async () => {
                setBusy(true)
                try {
                  await onDelete()
                } catch (caught) {
                  setError(caught as Error)
                  return
                } finally {
                  setBusy(false)
                }
                toast.success(`Deleted ${name}`)
                navigate(then)
              }}
            >
              Delete
            </AlertDialogAction>
          )}
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}

function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1)
}
