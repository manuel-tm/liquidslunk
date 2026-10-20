import {
  keepPreviousData,
  MutationCache,
  QueryCache,
  QueryClient,
  useQuery,
} from "@tanstack/react-query"

import { type CommandError, commands } from "@/bindings"

/**
 * A command's error (frontend.md, "Commands"), thrown by `useCommand()` and
 * `call()`. Its `message` is what `QueryError` shows, and `kind` says what
 * went wrong.
 */
export class BackendError extends Error {
  kind: CommandError["kind"]

  constructor({ kind, message }: CommandError) {
    super(message)
    this.name = "BackendError"
    this.kind = kind
  }
}

type Commands = typeof commands
type Name = keyof Commands
type Settled<N extends Name> = Awaited<ReturnType<Commands[N]>>

/** What a command returns when it works. */
export type CommandData<N extends Name> = Extract<
  Settled<N>,
  { status: "ok" }
>["data"]

/** The commands `useCommand()` reads with: no arguments, or one `args`. */
type Query = {
  [N in Name]: Parameters<Commands[N]> extends [] | [object] ? N : never
}[Name]

type CommandOptions = {
  /** Keeps no cached copy, so a form never starts from out-of-date data. */
  fresh?: boolean
  enabled?: boolean
  /**
   * Keeps the last arguments' data while new arguments load, with
   * `isPlaceholderData` set, so what shows doesn't flicker.
   */
  keepPrevious?: boolean
}

type QueryArguments<N extends Query> =
  Parameters<Commands[N]> extends [infer Args]
    ? [args: Args, options?: CommandOptions]
    : [args?: undefined, options?: CommandOptions]

/** Calls a command, and returns its data or throws its error. */
export async function call<N extends Name>(
  name: N,
  ...args: Parameters<Commands[N]>
): Promise<CommandData<N>> {
  const command = commands[name] as (
    ...args: unknown[]
  ) => Promise<
    { status: "ok"; data: unknown } | { status: "error"; error: CommandError }
  >
  const result = await command(...args)
  if (result.status === "error") throw new BackendError(result.error)
  return result.data as CommandData<N>
}

/**
 * The one way to read from the backend (frontend.md, "Commands"): calls
 * `commands[name](args)` through TanStack Query. Its key is `[name, args]`,
 * with the name as a string, which minification can't rename.
 */
export function useCommand<N extends Query>(
  name: N,
  ...[
    args,
    { fresh = false, enabled = true, keepPrevious = false } = {},
  ]: QueryArguments<N>
) {
  return useQuery({
    queryKey: [name, args ?? null],
    queryFn: () =>
      call(
        name,
        ...((args === undefined ? [] : [args]) as Parameters<Commands[N]>)
      ),
    gcTime: fresh ? 0 : undefined,
    enabled,
    placeholderData: keepPrevious ? keepPreviousData : undefined,
  })
}

let onNoLibrary = () => {}

/** What to do when a command says no library is open: go to Welcome. */
export function whenNoLibrary(handler: () => void) {
  onNoLibrary = handler
}

function noLibrary(error: Error) {
  if (error instanceof BackendError && error.kind === "noLibrary") onNoLibrary()
}

export const queryClient: QueryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // A page shows what's cached, and fetches again (frontend.md, "Commands")
      staleTime: 0,
      // A local call that fails will fail the same way again
      retry: false,
      refetchOnWindowFocus: false,
    },
  },
  queryCache: new QueryCache({ onError: noLibrary }),
  mutationCache: new MutationCache({
    onError: noLibrary,
    // Only mounted queries fetch again, usually one or two, over local IPC
    onSuccess: () => void queryClient.invalidateQueries(),
  }),
})

/**
 * An id from the URL, parsed with `Number()`, or null when it isn't one, so
 * the page shows `NotFound` (frontend.md, "Routes and pages").
 */
export function idParam(id: string | undefined): number | null {
  const number = Number(id)
  return id && Number.isSafeInteger(number) && number > 0 ? number : null
}

const WINDOWS = navigator.userAgent.includes("Windows")

/**
 * A registered file's bytes, from the app's file protocol (frontend.md,
 * "Files").
 */
export function fileUrl(id: number | string): string {
  return WINDOWS
    ? `http://slunk.localhost/files/${id}`
    : `slunk://localhost/files/${id}`
}
