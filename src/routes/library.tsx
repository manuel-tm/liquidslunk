import { open } from "@tauri-apps/plugin-dialog"
import {
  ArrowLeftIcon,
  CircleAlertIcon,
  FolderOpenIcon,
  FolderPlusIcon,
  LibraryBigIcon,
  WrenchIcon,
} from "lucide-react"
import { useEffect, useState } from "react"
import { Link, useNavigate } from "react-router"

import type { LibraryStatus, OpenProgress, RecentLibrary } from "@/bindings"
import { QueryError } from "@/components/query-error"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Item,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemMedia,
  ItemTitle,
} from "@/components/ui/item"
import { Spinner } from "@/components/ui/spinner"
import {
  createLibrary,
  endLaunch,
  launchLibrary,
  libraryName,
  openLibrary,
  useLaunch,
} from "@/lib/library"
import { useCommand } from "@/lib/query"

/** What Welcome is doing: opening a library, and whether it's upgrading it. */
type Busy = { name: string; migrating: boolean }

/**
 * `/library`, Welcome (frontend.md, "Routes and pages"): choose a library,
 * open an existing one or create one in a folder. At launch, it opens the one
 * the settings remember without asking. It's outside the app's layout, so it
 * has no sidebar.
 */
export function Welcome() {
  const navigate = useNavigate()
  const status = useCommand("libraryStatus")
  const recent = useCommand("recentLibraries")
  const [busy, setBusy] = useState<Busy | null>(null)
  const [error, setError] = useState<Error | null>(null)
  const [existing, setExisting] = useState<string | null>(null)
  const launch = useLaunch()

  const onProgress = (progress: OpenProgress) => {
    if (progress.type === "migrating") {
      setBusy((busy) => busy && { ...busy, migrating: true })
    }
  }

  async function openAt(path: string) {
    endLaunch()
    setError(null)
    setExisting(null)
    setBusy({ name: libraryName(path), migrating: false })
    try {
      await openLibrary(path, onProgress)
      navigate("/", { replace: true })
    } catch (caught) {
      setError(caught as Error)
      setBusy(null)
      void recent.refetch()
    }
  }

  async function createIn(folder: string) {
    endLaunch()
    setError(null)
    setBusy({ name: libraryName(`${folder}/slunk.db`), migrating: false })
    try {
      const created = await createLibrary(folder, onProgress)
      if (created.type === "exists") {
        setExisting(created.path)
        setBusy(null)
      } else {
        navigate("/", { replace: true })
      }
    } catch (caught) {
      setError(caught as Error)
      setBusy(null)
    }
  }

  // At launch, the library the settings remember opens without asking
  useEffect(() => {
    if (!status.data) return
    void launchLibrary(status.data).then(
      (opened) => opened && navigate("/", { replace: true })
    )
  }, [status.data, navigate])
  const saved = status.data?.saved
  const opening: Busy | null =
    busy ??
    (launch.state === "opening"
      ? launch
      : launch.state === "waiting" &&
          status.data &&
          !status.data.library &&
          saved?.found
        ? { name: libraryName(saved.path), migrating: false }
        : null)
  const failure = error ?? (launch.state === "failed" ? launch.error : null)

  const devLibrary = status.data?.devLibrary ?? undefined

  /** Looks for the saved library again, and opens it if it's back. */
  async function retry() {
    const { data } = await status.refetch()
    if (data?.saved?.found) await openAt(data.saved.path)
  }

  async function chooseLibrary() {
    const path = await open({
      title: "Open a library",
      filters: [{ name: "Slunk library", extensions: ["db"] }],
      defaultPath: devLibrary,
    })
    if (typeof path === "string") await openAt(path)
  }

  async function chooseFolder() {
    const folder = await open({
      title: "Choose a folder for the new library",
      directory: true,
      canCreateDirectories: true,
      defaultPath: devLibrary,
    })
    if (typeof folder === "string") await createIn(folder)
  }

  return (
    <main className="flex min-h-svh items-center justify-center bg-background p-6">
      <Card className="w-full max-w-xl">
        <CardHeader>
          <CardTitle className="flex items-center gap-2 font-heading text-xl">
            <LibraryBigIcon />
            liquidslunk
          </CardTitle>
          <CardDescription>
            Choose a library: a folder with a slunk.db, which catalogs the music
            around it. liquidslunk only reads your music files.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          {status.data?.library && !opening && (
            <Link
              to="/"
              className="inline-flex items-center gap-1 text-xs hover:underline"
            >
              <ArrowLeftIcon className="size-3" />
              Back to {status.data.library.name}
            </Link>
          )}
          {devLibrary && <DevelopmentNote folder={devLibrary} />}
          {status.error && <QueryError error={status.error} />}
          {failure && (
            <QueryError error={failure} title="Can't open the library" />
          )}
          {!opening && !failure && (
            <Missing status={status.data} onRetry={() => void retry()} />
          )}
          {opening ? (
            <p className="flex items-center gap-2 text-sm" role="status">
              <Spinner />
              {opening.migrating
                ? "Backing up and upgrading the library…"
                : `Opening ${opening.name}…`}
            </p>
          ) : (
            <div className="flex flex-wrap gap-2">
              <Button onClick={() => void chooseLibrary()}>
                <FolderOpenIcon data-icon="inline-start" />
                Open a library…
              </Button>
              <Button variant="outline" onClick={() => void chooseFolder()}>
                <FolderPlusIcon data-icon="inline-start" />
                New library…
              </Button>
            </div>
          )}
          <RecentLibraries
            libraries={recent.data ?? []}
            disabled={opening !== null}
            onOpen={(path) => void openAt(path)}
          />
        </CardContent>
      </Card>
      <AlertDialog
        open={existing !== null}
        onOpenChange={(open) => !open && setExisting(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>This folder has a library</AlertDialogTitle>
            <AlertDialogDescription className="break-all">
              There's a library in it already, at {existing}. Open it instead?
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => existing && void openAt(existing)}
            >
              Open it
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </main>
  )
}

/** The saved library, when it isn't there right now. */
function Missing({
  status,
  onRetry,
}: {
  status: LibraryStatus | undefined
  onRetry: () => void
}) {
  if (!status || status.library || !status.saved || status.saved.found) {
    return null
  }
  return (
    <Alert>
      <CircleAlertIcon />
      <AlertTitle>Couldn't find the library</AlertTitle>
      <AlertDescription className="flex flex-col items-start gap-2">
        <span className="break-all">
          It was at {status.saved.path}. If it's on a drive that isn't
          connected, connect it and try again.
        </span>
        <Button variant="outline" size="sm" onClick={onRetry}>
          Try again
        </Button>
      </AlertDescription>
    </Alert>
  )
}

/**
 * Debug builds keep their libraries in `dev-library/` (getting-started.md,
 * "Running the app").
 */
function DevelopmentNote({ folder }: { folder: string }) {
  return (
    <Alert>
      <WrenchIcon />
      <AlertTitle>Development build</AlertTitle>
      <AlertDescription>
        Open the library in dev-library/music/, or create one in a new folder of
        dev-library/. Never open a real music library from a development build.
        The dialogs start in <span className="break-all">{folder}</span>.
      </AlertDescription>
    </Alert>
  )
}

/** The last five libraries opened. One that isn't there right now is dimmed. */
function RecentLibraries({
  libraries,
  disabled,
  onOpen,
}: {
  libraries: RecentLibrary[]
  disabled: boolean
  onOpen: (path: string) => void
}) {
  if (libraries.length === 0) return null
  return (
    <section className="flex flex-col gap-2">
      <h2 className="text-xs font-medium text-muted-foreground">
        Recent libraries
      </h2>
      <ItemGroup>
        {libraries.map((library) => (
          <Item
            key={library.path}
            variant="outline"
            size="sm"
            render={
              <button
                type="button"
                disabled={disabled || !library.found}
                onClick={() => onOpen(library.path)}
              />
            }
            className="text-left enabled:cursor-pointer enabled:hover:bg-muted/50 disabled:opacity-50"
          >
            <ItemMedia variant="icon">
              <LibraryBigIcon />
            </ItemMedia>
            <ItemContent className="min-w-0">
              <ItemTitle>
                {library.name}
                {!library.found && (
                  <span className="font-normal text-muted-foreground">
                    · Not found
                  </span>
                )}
              </ItemTitle>
              <ItemDescription className="truncate">
                {library.folder}
              </ItemDescription>
            </ItemContent>
          </Item>
        ))}
      </ItemGroup>
    </section>
  )
}
