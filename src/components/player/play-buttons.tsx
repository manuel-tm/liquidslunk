import {
  ListEndIcon,
  ListPlusIcon,
  ListStartIcon,
  PauseIcon,
  PlayIcon,
} from "lucide-react"
import { toast } from "sonner"

import { usePlayer } from "@/components/player/provider"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { call } from "@/lib/query"

/**
 * A track's play button, in a list of tracks: it plays the list from that
 * track, or pauses it while it's the one playing (2_player.md §5.2).
 */
export function TrackPlayButton({
  trackIds,
  index,
  title,
  className,
}: {
  /** The list's tracks, in the order they play */
  trackIds: number[]
  index: number
  title: string
  className?: string
}) {
  const player = usePlayer()
  const current = player.current?.trackId === trackIds[index]
  const playing = current && player.playing
  return (
    <Button
      variant="ghost"
      size="icon-xs"
      className={className}
      aria-label={playing ? `Pause ${title}` : `Play ${title}`}
      onClick={() => (current ? player.toggle() : player.play(trackIds, index))}
    >
      {playing ? <PauseIcon /> : <PlayIcon />}
    </Button>
  )
}

/**
 * Play next and Add to queue, for a track or a release's tracks (2_player.md
 * §5.3), with a toast that says what was added. `name` names the one track,
 * or the release.
 */
export function QueueMenu({
  trackIds,
  name,
  className,
}: {
  trackIds: number[]
  name: string
  className?: string
}) {
  const { playNext, addToQueue } = usePlayer()
  const what = trackIds.length === 1 ? name : `${trackIds.length} tracks`
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            variant="ghost"
            size="icon-xs"
            className={className}
            aria-label={`Queue ${name}`}
            title="Play next, or add to the queue"
            disabled={trackIds.length === 0}
          />
        }
      >
        <ListPlusIcon />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem
          onClick={() => {
            playNext(trackIds)
            toast(
              trackIds.length === 1 ? `${what} plays next` : `${what} play next`
            )
          }}
        >
          <ListStartIcon />
          Play next
        </DropdownMenuItem>
        <DropdownMenuItem
          onClick={() => {
            addToQueue(trackIds)
            toast(`Added ${what} to the queue`)
          }}
        >
          <ListEndIcon />
          Add to queue
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

/**
 * Plays a release from its first track, or from a random one when shuffled,
 * for the release table's covers (2_player.md §5.2). It asks for the
 * release's tracks when it's pressed.
 */
export function ReleasePlayButton({
  id,
  title,
  className,
}: {
  id: number
  title: string
  className?: string
}) {
  const { play } = usePlayer()

  async function playRelease() {
    try {
      const trackIds = await call("releaseTrackIds", { id })
      if (trackIds.length === 0) toast(`${title} has no tracks yet`)
      else play(trackIds)
    } catch (error) {
      toast.error(`Can't play ${title}`, {
        description: (error as Error).message,
      })
    }
  }

  return (
    <Button
      variant="secondary"
      size="icon-sm"
      className={className}
      aria-label={`Play ${title}`}
      onClick={() => void playRelease()}
    >
      <PlayIcon />
    </Button>
  )
}
