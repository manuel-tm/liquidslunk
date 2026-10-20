import { useMutation } from "@tanstack/react-query"
import {
  FolderIcon,
  HardDriveUploadIcon,
  type LucideProps,
  PlusIcon,
  SpeakerIcon,
  XIcon,
} from "lucide-react"
import { Link } from "react-router"
import { toast } from "sonner"

import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import type { ExternalKind, RuleTargetInput } from "@/bindings"
import { describeMembership } from "@/lib/external"
import { call, useCommand } from "@/lib/query"

/** An external library's kind, as an icon: a folder, or an iPod. */
export function ExternalKindIcon({
  kind,
  ...props
}: { kind: ExternalKind } & LucideProps) {
  return kind === "ipod" ? (
    <SpeakerIcon {...props} />
  ) : (
    <FolderIcon {...props} />
  )
}

/**
 * "Add to external library", with a menu of them (4_external_libraries.md
 * §11), on the release, artist and track pages and in the release list's
 * bulk edits. A release comes in whole. A toast says where it went.
 */
export function AddToExternalLibrary({
  releases,
  artists,
  tracks,
  what,
  size = "default",
}: {
  releases?: number[]
  artists?: number[]
  tracks?: number[]
  /** What's added, for the toast: "Scenery", "12 releases" */
  what: string
  size?: "default" | "sm"
}) {
  const { data: libraries } = useCommand("externalLibraries")
  const add = useMutation({
    mutationFn: (id: number) =>
      call("addToExternalLibrary", { id, releases, artists, tracks }),
  })
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="outline" size={size} />}>
        <HardDriveUploadIcon data-icon="inline-start" />
        Add to external library
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="min-w-56">
        {libraries && libraries.length > 0 && (
          // A label belongs to a group
          <DropdownMenuGroup>
            <DropdownMenuLabel>Add {what} to</DropdownMenuLabel>
            {libraries.map((library) => (
              <DropdownMenuItem
                key={library.id}
                onClick={async () => {
                  try {
                    await add.mutateAsync(library.id)
                    toast.success(`Added ${what} to ${library.name}`)
                  } catch (error) {
                    toast.error(`Can't add ${what} to ${library.name}`, {
                      description: (error as Error).message,
                    })
                  }
                }}
              >
                <ExternalKindIcon kind={library.kind} />
                {library.name}
              </DropdownMenuItem>
            ))}
          </DropdownMenuGroup>
        )}
        {libraries && libraries.length > 0 && <DropdownMenuSeparator />}
        <DropdownMenuItem render={<Link to="/external-libraries" />}>
          <PlusIcon />
          New external library…
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

/**
 * The external libraries a release, an artist or a track is in, and why:
 * "On Car, through Pink Floyd". Removing it takes out its own rule, or leaves
 * it out when a broader rule brings it in. Nothing shows while it's in none.
 */
export function ExternalMemberships({
  subject,
}: {
  subject: { release: number } | { artist: number } | { track: number }
}) {
  const { data: memberships } = useCommand("externalMemberships", subject)
  const target: RuleTargetInput =
    "release" in subject
      ? { type: "release", id: subject.release }
      : "artist" in subject
        ? { type: "artist", id: subject.artist }
        : { type: "track", id: subject.track }
  const remove = useMutation({
    mutationFn: (id: number) =>
      call("removeFromExternalLibrary", { id, target }),
  })
  if (!memberships || memberships.length === 0) return null

  return (
    <ul className="flex flex-col gap-0.5 text-xs">
      {memberships.map((membership) => {
        const { library } = membership
        // A broader rule still brings it in, so it's left out instead
        const leaveOut = membership.reason.type !== "own"
        const action = leaveOut
          ? `Leave out of ${library.name}`
          : `Remove from ${library.name}`
        return (
          <li key={library.id} className="flex items-center gap-1">
            <ExternalKindIcon
              kind={library.kind}
              className="size-3.5 text-muted-foreground"
            />
            <Link
              to={`/external-libraries/${library.id}/contents`}
              className="hover:underline"
            >
              {describeMembership(membership)}
            </Link>
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label={action}
              title={action}
              disabled={remove.isPending}
              onClick={async () => {
                try {
                  await remove.mutateAsync(library.id)
                } catch (error) {
                  toast.error(`Can't change ${library.name}`, {
                    description: (error as Error).message,
                  })
                }
              }}
            >
              <XIcon />
            </Button>
          </li>
        )
      })}
    </ul>
  )
}
