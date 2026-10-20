import { useState } from "react"

import { QueryError } from "@/components/query-error"
import { Button } from "@/components/ui/button"
import { Progress, ProgressLabel } from "@/components/ui/progress"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet"
import { Skeleton } from "@/components/ui/skeleton"
import { Spinner } from "@/components/ui/spinner"
import type {
  ExternalLibraryDetails,
  PlanLeftOut,
  PlanStep,
  SyncAction,
  SyncLabel,
} from "@/bindings"
import {
  describeChange,
  describeOutcome,
  describePlan,
  describeSpace,
  STAGE_LABELS,
} from "@/lib/external"
import { formatSize } from "@/lib/format"
import { useCommand } from "@/lib/query"
import {
  cancelSync,
  clearSync,
  ejectLibrary,
  startSync,
  type SyncRun,
  useSyncRun,
} from "@/lib/sync"

/** How many of a section's files the sheet lists */
const SHOWN = 200

const SECTIONS: [SyncAction, string][] = [
  ["add", "Add"],
  ["convert", "Convert again"],
  ["restore", "Restore"],
  ["retag", "Retag"],
  ["move", "Move"],
  ["remove", "Remove"],
]

/**
 * Sync, in a sheet (4_external_libraries.md §11): the plan first, with
 * what it adds, updates and removes, what it leaves out and whether it fits;
 * then its progress, which can be cancelled; then how it went. The sheet can
 * be closed while it runs, and the sync goes on.
 */
export function SyncSheet({
  library,
  open,
  onOpenChange,
}: {
  library: ExternalLibraryDetails
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const run = useSyncRun()
  const mine = run?.libraryId === library.id ? run : null

  return (
    <Sheet
      open={open}
      onOpenChange={(next) => {
        if (!next && mine && !mine.running) clearSync(library.id)
        onOpenChange(next)
      }}
    >
      <SheetContent side="right" className="sm:max-w-xl">
        <SheetHeader>
          <SheetTitle>Sync {library.name}</SheetTitle>
          <SheetDescription className="font-mono break-all">
            {library.folder}
          </SheetDescription>
        </SheetHeader>
        {mine?.running ? (
          <Running run={mine} />
        ) : mine ? (
          <Result
            library={library}
            run={mine}
            onClose={() => {
              clearSync(library.id)
              onOpenChange(false)
            }}
          />
        ) : (
          open && (
            <Plan library={library} onCancel={() => onOpenChange(false)} />
          )
        )}
      </SheetContent>
    </Sheet>
  )
}

/** The plan, worked out when the sheet opens, and again on Look again. */
function Plan({
  library,
  onCancel,
}: {
  library: ExternalLibraryDetails
  onCancel: () => void
}) {
  const {
    data: plan,
    error,
    isFetching,
    refetch,
  } = useCommand("syncPlan", { id: library.id }, { fresh: true })
  const anotherRuns = useSyncRun()?.running ?? false

  const space = plan && describeSpace(plan)
  return (
    <>
      <div className="flex flex-1 flex-col gap-4 overflow-y-auto px-4">
        {error ? (
          <QueryError error={error} title="Can't work out the sync" />
        ) : !plan || isFetching ? (
          <div className="flex flex-col gap-2">
            <p className="flex items-center gap-2 text-muted-foreground">
              <Spinner />
              Reading what&apos;s in the library and in the folder…
            </p>
            <Skeleton className="h-40 w-full" />
          </div>
        ) : (
          <>
            <div className="flex flex-col gap-1">
              <p className="text-sm">{describePlan(plan)}</p>
              {space && (
                <p
                  className={
                    plan.fits ? "text-muted-foreground" : "text-destructive"
                  }
                >
                  {space}
                </p>
              )}
            </div>
            {SECTIONS.map(([action, title]) => (
              <Section
                key={action}
                title={title}
                steps={plan.steps.filter((step) => step.action === action)}
              />
            ))}
            <LeftOut items={plan.leftOut} />
          </>
        )}
      </div>
      <SheetFooter className="flex-row justify-end">
        <Button variant="outline" onClick={onCancel}>
          Cancel
        </Button>
        <Button
          variant="outline"
          disabled={isFetching}
          onClick={() => void refetch()}
        >
          Look again
        </Button>
        <Button
          disabled={
            !plan ||
            isFetching ||
            plan.steps.length === 0 ||
            !plan.fits ||
            anotherRuns
          }
          onClick={() => plan && void startSync(library.id, plan.id)}
        >
          Sync
        </Button>
      </SheetFooter>
    </>
  )
}

function Section({ title, steps }: { title: string; steps: PlanStep[] }) {
  if (steps.length === 0) return null
  const bytes = steps.reduce((sum, step) => sum + step.bytes, 0)
  return (
    <section className="flex flex-col gap-1">
      <h3 className="font-medium">
        {title} · {steps.length}
        {bytes > 0 && (
          <span className="font-normal text-muted-foreground">
            {" "}
            · {formatSize(bytes)}
          </span>
        )}
      </h3>
      <ul className="flex flex-col gap-1.5">
        {steps.slice(0, SHOWN).map((step) => (
          <li
            key={`${step.action}:${step.path}`}
            className="flex flex-col gap-0.5"
          >
            <Named label={step.label} cover={step.trackId === null} />
            <span className="font-mono break-all text-muted-foreground">
              {step.from ? `${step.from} → ${step.path}` : step.path}
            </span>
            {step.changes.map((change) => (
              <span key={change.field} className="text-muted-foreground">
                {describeChange(change)}
              </span>
            ))}
          </li>
        ))}
      </ul>
      {steps.length > SHOWN && (
        <p className="text-muted-foreground">and {steps.length - SHOWN} more</p>
      )}
    </section>
  )
}

function LeftOut({ items }: { items: PlanLeftOut[] }) {
  if (items.length === 0) return null
  return (
    <section className="flex flex-col gap-1">
      <h3 className="font-medium">Left out · {items.length}</h3>
      <ul className="flex flex-col gap-1.5">
        {items.slice(0, SHOWN).map((item, index) => (
          <li key={index} className="flex flex-col gap-0.5">
            <Named label={item.label} cover={item.trackId === null} />
            <span className="text-muted-foreground">{item.reason}</span>
          </li>
        ))}
      </ul>
      {items.length > SHOWN && (
        <p className="text-muted-foreground">and {items.length - SHOWN} more</p>
      )}
    </section>
  )
}

/** A track by its title, artist and album, or a release's cover. */
function Named({ label, cover }: { label: SyncLabel; cover: boolean }) {
  const by = [label.artist, label.album].filter(Boolean).join(" · ")
  return (
    <span>
      {cover ? `Cover of ${label.title}` : label.title}
      {by && <span className="text-muted-foreground"> · {by}</span>}
    </span>
  )
}

function Running({ run }: { run: SyncRun }) {
  const [cancelling, setCancelling] = useState(false)
  const progress = run.progress
  const percent =
    progress && progress.total > 0
      ? (progress.done / progress.total) * 100
      : null
  return (
    <>
      <div className="flex flex-1 flex-col gap-3 overflow-y-auto px-4">
        <Progress value={percent}>
          <ProgressLabel>
            {progress ? STAGE_LABELS[progress.stage] : "Starting"}
            {cancelling && ": cancelling after this file"}
          </ProgressLabel>
          {progress && (
            <span className="ml-auto text-xs text-muted-foreground tabular-nums">
              {progress.done} of {progress.total}
            </span>
          )}
        </Progress>
        {progress?.current && <p className="break-all">{progress.current}</p>}
        {progress && (
          <p className="text-muted-foreground">
            {formatSize(progress.bytes)} written
          </p>
        )}
        <p className="text-muted-foreground">
          The library can be browsed and edited meanwhile: what changes now
          reaches the next sync. Closing this leaves the sync running.
        </p>
      </div>
      <SheetFooter className="flex-row justify-end">
        <Button
          variant="outline"
          disabled={cancelling}
          onClick={() => {
            setCancelling(true)
            void cancelSync()
          }}
        >
          Cancel sync
        </Button>
      </SheetFooter>
    </>
  )
}

function Result({
  library,
  run,
  onClose,
}: {
  library: ExternalLibraryDetails
  run: SyncRun
  onClose: () => void
}) {
  const [ejecting, setEjecting] = useState(false)
  const [ejectError, setEjectError] = useState<Error | null>(null)
  const outcome = run.outcome
  return (
    <>
      <div className="flex flex-1 flex-col gap-3 overflow-y-auto px-4">
        {run.error ? (
          <>
            <QueryError error={run.error} title="The sync stopped" />
            <p className="text-muted-foreground">
              What it had written is kept, and the next sync carries on from
              there.
            </p>
          </>
        ) : (
          outcome && (
            <>
              <p className="text-sm">
                {outcome.cancelled ? "Cancelled. " : "Synced. "}
                {describeOutcome(outcome)}
              </p>
              <p className="text-muted-foreground">
                {formatSize(outcome.bytes)} written
                {outcome.cancelled && ". The next sync carries on from there."}
              </p>
              {outcome.failed.length > 0 && (
                <section className="flex flex-col gap-1">
                  <h3 className="font-medium">
                    Couldn&apos;t write · {outcome.failed.length}
                  </h3>
                  <ul className="flex flex-col gap-1.5">
                    {outcome.failed.map((failure) => (
                      <li key={failure.path} className="flex flex-col gap-0.5">
                        <span className="font-mono break-all">
                          {failure.path}
                        </span>
                        <span className="text-muted-foreground">
                          {failure.reason}
                        </span>
                      </li>
                    ))}
                  </ul>
                </section>
              )}
            </>
          )
        )}
        {ejectError && <QueryError error={ejectError} title="Can't eject it" />}
      </div>
      <SheetFooter className="flex-row justify-end">
        {library.ejectable && !run.error && (
          <Button
            variant="outline"
            disabled={ejecting}
            onClick={async () => {
              setEjecting(true)
              setEjectError(null)
              try {
                await ejectLibrary(library.id, library.folder)
                onClose()
              } catch (caught) {
                setEjectError(caught as Error)
              } finally {
                setEjecting(false)
              }
            }}
          >
            Eject
          </Button>
        )}
        <Button onClick={onClose}>Close</Button>
      </SheetFooter>
    </>
  )
}
