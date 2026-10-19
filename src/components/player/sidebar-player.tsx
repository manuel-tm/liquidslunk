import {
  PauseIcon,
  PlayIcon,
  SkipBackIcon,
  SkipForwardIcon,
  Volume1Icon,
  Volume2Icon,
  VolumeXIcon,
} from "lucide-react"
import { Link } from "react-router"

import type { PlayerTrack } from "@/bindings"
import { Cover } from "@/components/cover"
import { artistUrl, Names } from "@/components/names"
import { usePlayer, usePlayerTime } from "@/components/player/provider"
import { Title } from "@/components/titles"
import { Button } from "@/components/ui/button"
import { Progress } from "@/components/ui/progress"
import { Slider } from "@/components/ui/slider"
import { Spinner } from "@/components/ui/spinner"
import { shownName } from "@/lib/credited"
import { formatDuration } from "@/lib/format"

/** A slider's value, which Base UI gives as a number or an array. */
function first(value: number | readonly number[]) {
  return typeof value === "number" ? value : value[0]
}

/** Seconds as `m:ss`, played ones rounded down. */
function clock(seconds: number) {
  return formatDuration(Math.floor(seconds) * 1000)
}

/**
 * The player's controls, in the sidebar's header, between the library menu
 * and Import (2_player.md §4.2): what's playing, how far it is, previous,
 * play/pause and next, and the volume. With nothing queued, one line.
 */
export function SidebarPlayer() {
  const { current, track } = usePlayer()

  if (!current) {
    return (
      <div className="flex items-center gap-2 px-2 text-xs text-muted-foreground">
        <Cover file={null} alt="" className="size-8" />
        Nothing playing
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-2 px-2">
      <NowPlaying track={track} />
      <Played track={track} />
      <Buttons />
      <VolumeControl />
    </div>
  )
}

/** The cover, then the track, who performs it, and the release. */
function NowPlaying({ track }: { track: PlayerTrack | null }) {
  if (!track) {
    return (
      <div className="flex items-center gap-2">
        <Cover file={null} alt="" className="size-12" />
      </div>
    )
  }
  const language = track.titleLanguage ?? track.release.titleLanguage
  const performers = track.performers
    .map((performer) => shownName(performer).name)
    .join(", ")
  return (
    <div className="flex min-w-0 items-center gap-2 text-xs">
      <Link
        to={`/releases/${track.release.id}`}
        className="shrink-0"
        title={track.release.title}
      >
        <Cover
          file={track.cover}
          alt={`${track.release.title} cover`}
          className="size-12"
        />
      </Link>
      <div className="flex min-w-0 flex-col">
        <Link
          to={`/tracks/${track.id}`}
          className="truncate font-medium hover:underline"
          title={track.title}
        >
          <Title title={track.title} language={language} />
        </Link>
        {track.performers.length > 0 && (
          <span className="truncate" title={performers}>
            <Names items={track.performers} to={artistUrl} />
          </span>
        )}
        <Link
          to={`/releases/${track.release.id}`}
          className="truncate text-muted-foreground hover:underline"
          title={track.release.title}
        >
          <Title
            title={track.release.title}
            language={track.release.titleLanguage}
          />
        </Link>
      </div>
    </div>
  )
}

/**
 * The time played and the track's length, with a bar between them. It doesn't
 * seek: WebKit seeks FLAC files by estimate (2_player.md Appendix B). The
 * length is the element's once it has read the file, and the catalog's until
 * then.
 */
function Played({ track }: { track: PlayerTrack | null }) {
  const { time, duration: read } = usePlayerTime()
  const duration =
    Number.isFinite(read) && read > 0 ? read : (track?.durationMs ?? 0) / 1000
  const played = Math.min(time, duration)
  // Rounded, as the tracklist rounds it
  const length = formatDuration(duration ? duration * 1000 : null)

  return (
    <div className="flex flex-col gap-1">
      <Progress
        aria-label="Played"
        value={played}
        max={duration || 1}
        getAriaValueText={() => `${clock(played)} of ${length}`}
      />
      <div className="flex justify-between text-[0.625rem] text-muted-foreground tabular-nums">
        <span>{clock(played)}</span>
        <span>{length}</span>
      </div>
    </div>
  )
}

/** Previous, play/pause and next. */
function Buttons() {
  const { playing, waiting, toggle, previous, next } = usePlayer()
  return (
    <div className="flex items-center justify-center gap-2">
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label="Previous"
        title="Previous (⌘←)"
        onClick={previous}
      >
        <SkipBackIcon />
      </Button>
      <Button
        size="icon"
        aria-label={playing ? "Pause" : "Play"}
        title={playing ? "Pause (Space)" : "Play (Space)"}
        onClick={toggle}
      >
        {playing && waiting ? (
          <Spinner />
        ) : playing ? (
          <PauseIcon />
        ) : (
          <PlayIcon />
        )}
      </Button>
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label="Next"
        title="Next (⌘→)"
        onClick={next}
      >
        <SkipForwardIcon />
      </Button>
    </div>
  )
}

/** Mute, and the volume. */
function VolumeControl() {
  const { volume, muted, setVolume, toggleMuted } = usePlayer()
  const silent = muted || volume === 0
  return (
    <div className="flex items-center gap-2">
      <Button
        variant="ghost"
        size="icon-xs"
        aria-label={muted ? "Unmute" : "Mute"}
        title={muted ? "Unmute" : "Mute"}
        onClick={toggleMuted}
      >
        {silent ? (
          <VolumeXIcon />
        ) : volume < 0.5 ? (
          <Volume1Icon />
        ) : (
          <Volume2Icon />
        )}
      </Button>
      <Slider
        title="Volume (⌘↑, ⌘↓)"
        value={[muted ? 0 : Math.round(volume * 100)]}
        min={0}
        max={100}
        thumb={{
          getAriaLabel: () => "Volume",
          getAriaValueText: (_, value) => `${Math.round(value)}%`,
        }}
        onValueChange={(value) => setVolume(first(value) / 100)}
      />
    </div>
  )
}
