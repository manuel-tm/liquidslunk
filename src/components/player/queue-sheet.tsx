import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  PointerSensor,
  type UniqueIdentifier,
  useSensor,
  useSensors,
} from "@dnd-kit/core"
import { restrictToVerticalAxis } from "@dnd-kit/modifiers"
import {
  SortableContext,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable"
import { CSS } from "@dnd-kit/utilities"
import { GripVerticalIcon, Volume2Icon, XIcon } from "lucide-react"
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react"
import { Link } from "react-router"

import type { PlayerTrack } from "@/bindings"
import { Cover } from "@/components/cover"
import { artistUrl, Names } from "@/components/names"
import { usePlayer } from "@/components/player/provider"
import { Title } from "@/components/titles"
import { Button } from "@/components/ui/button"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet"
import { formatDuration } from "@/lib/format"
import type { Entry } from "@/lib/queue"
import { cn } from "@/lib/utils"

/**
 * The whole queue, in the order it plays (2_player.md §5.5). The entries that
 * have played are muted, and the current one is marked. Clicking a row plays
 * it. A row is dragged anywhere on it to another place, or moved a place at a
 * time with the arrow keys on its handle (Appendix D), and its button removes
 * it. It opens scrolled to the current entry.
 */
export function QueueSheet({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const { entries, current, tracks, move, clear } = usePlayer()
  const list = useRef<HTMLOListElement>(null)
  const playing = current ? entries.indexOf(current) : -1
  const length = entries.reduce(
    (sum, entry) => sum + (tracks.get(entry.trackId)?.durationMs ?? 0),
    0
  )
  const close = () => onOpenChange(false)

  // A drag starts once the pointer has moved, so a click still plays a row or
  // follows a link, and dnd-kit stops the click that ends a drag. The keyboard
  // doesn't go through dnd-kit: its keyboard sensor listens on the document,
  // and the sheet's dialog stops arrow keys from leaving it.
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } })
  )
  const indexOf = (id: UniqueIdentifier) =>
    entries.findIndex((entry) => entry.key === id)
  const titleOf = (id: UniqueIdentifier) =>
    tracks.get(entries[indexOf(id)]?.trackId)?.title ?? "The track"
  // What a move from the keyboard says to assistive tech, and the row whose
  // handle keeps the focus as the rows reorder
  const [announcement, setAnnouncement] = useState("")
  const [focusKey, setFocusKey] = useState<number | null>(null)

  // The click that ends a drag: dnd-kit keeps it from the row, but not its
  // default, and on a title or an artist, that's the browser loading the
  // link's page itself. So its default is prevented, just after a drag.
  const dragEnded = useRef(-Infinity)
  useEffect(() => {
    function onClick(event: MouseEvent) {
      if (performance.now() - dragEnded.current < 200) event.preventDefault()
    }
    document.addEventListener("click", onClick, true)
    return () => document.removeEventListener("click", onClick, true)
  }, [])

  function onDragEnd({ active, over }: DragEndEvent) {
    dragEnded.current = performance.now()
    if (!over || active.id === over.id) return
    const from = indexOf(active.id)
    move(entries[from], indexOf(over.id) - from)
  }

  function onKeyboardMove(entry: Entry, by: number) {
    const from = entries.indexOf(entry)
    const to = from + by
    if (to < 0 || to >= entries.length) return
    move(entry, by)
    setFocusKey(entry.key)
    setAnnouncement(
      `${titleOf(entry.key)} moved to place ${to + 1} of ${entries.length}.`
    )
  }

  useEffect(() => {
    if (!open) return
    // Once the sheet has rendered its rows
    const frame = requestAnimationFrame(() =>
      list.current
        ?.querySelector("[data-current]")
        ?.scrollIntoView({ block: "center" })
    )
    return () => cancelAnimationFrame(frame)
  }, [open])

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right">
        <SheetHeader>
          <SheetTitle>Queue</SheetTitle>
          <SheetDescription>
            {entries.length === 1 ? "1 track" : `${entries.length} tracks`}
            {length > 0 && ` · ${formatDuration(length)}`}
          </SheetDescription>
        </SheetHeader>
        <DndContext
          sensors={sensors}
          collisionDetection={closestCenter}
          modifiers={[restrictToVerticalAxis]}
          onDragEnd={onDragEnd}
          onDragCancel={() => (dragEnded.current = performance.now())}
          accessibility={{
            screenReaderInstructions: {
              draggable:
                "To move a track, press the up and down arrow keys, or drag it.",
            },
            announcements: {
              onDragStart: ({ active }) => `Picked up ${titleOf(active.id)}.`,
              onDragOver: ({ active, over }) =>
                over
                  ? `${titleOf(active.id)} is at place ${indexOf(over.id) + 1} of ${entries.length}.`
                  : undefined,
              onDragEnd: ({ active, over }) =>
                over
                  ? `${titleOf(active.id)} dropped at place ${indexOf(over.id) + 1} of ${entries.length}.`
                  : `${titleOf(active.id)} dropped.`,
              onDragCancel: ({ active }) =>
                `${titleOf(active.id)} went back to its place.`,
            },
          }}
        >
          <SortableContext
            items={entries.map((entry) => entry.key)}
            strategy={verticalListSortingStrategy}
          >
            <ol ref={list} className="flex-1 overflow-y-auto">
              {entries.map((entry, index) => (
                <QueueRow
                  key={entry.key}
                  entry={entry}
                  track={tracks.get(entry.trackId)}
                  played={index < playing}
                  current={index === playing}
                  focus={entry.key === focusKey}
                  onFocused={() => setFocusKey(null)}
                  onKeyboardMove={(by) => onKeyboardMove(entry, by)}
                  onNavigate={close}
                />
              ))}
            </ol>
          </SortableContext>
        </DndContext>
        <div aria-live="assertive" className="sr-only">
          {announcement}
        </div>
        <SheetFooter>
          <Button
            variant="outline"
            disabled={entries.length === 0}
            onClick={() => {
              clear()
              close()
            }}
          >
            Clear
          </Button>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  )
}

/** An entry: its handle, its cover, title and artists, length, and remove. */
function QueueRow({
  entry,
  track,
  played,
  current,
  focus,
  onFocused,
  onKeyboardMove,
  onNavigate,
}: {
  entry: Entry
  track: PlayerTrack | undefined
  played: boolean
  current: boolean
  /** Whether its handle takes the focus back, after a move from the keyboard */
  focus: boolean
  onFocused: () => void
  /** The arrow keys on its handle: up one place (-1) or down one (1) */
  onKeyboardMove: (by: number) => void
  /** A link in the row was followed: the sheet closes */
  onNavigate: () => void
}) {
  const { jump, remove } = usePlayer()
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: entry.key })
  const title = track?.title ?? "the track"
  const handle = useRef<HTMLButtonElement | null>(null)
  const setHandle = useCallback(
    (node: HTMLButtonElement | null) => {
      handle.current = node
      setActivatorNodeRef(node)
    },
    [setActivatorNodeRef]
  )

  // Reordering the rows can take the focus off the handle that moved
  useLayoutEffect(() => {
    if (!focus) return
    handle.current?.focus()
    onFocused()
  }, [focus, onFocused])

  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Translate.toString(transform), transition }}
      data-current={current || undefined}
      className={cn(
        "flex cursor-pointer touch-none items-center gap-2 py-1.5 pr-4 pl-2 select-none",
        played && "text-muted-foreground",
        current ? "bg-muted" : "bg-popover hover:bg-muted/50",
        // Above the rows it passes
        isDragging && "relative z-10 cursor-grabbing shadow-md"
      )}
      // The whole row drags: its links, cover and buttons too
      {...listeners}
      // The row plays it, but not its links and buttons
      onClick={(event) => {
        if (!(event.target as Element).closest("a, button")) jump(entry)
      }}
    >
      {/* Where the keyboard moves the row, and a sign that it drags */}
      <button
        ref={setHandle}
        type="button"
        className="shrink-0 p-1 text-muted-foreground hover:text-foreground focus-visible:text-foreground"
        aria-label={`Move ${title}`}
        {...attributes}
        onKeyDown={(event) => {
          if (event.altKey || event.ctrlKey || event.metaKey) return
          const by =
            event.key === "ArrowUp" ? -1 : event.key === "ArrowDown" ? 1 : 0
          if (!by) return
          event.preventDefault()
          onKeyboardMove(by)
        }}
      >
        <GripVerticalIcon className="size-4" />
      </button>
      <button
        type="button"
        className="relative shrink-0"
        aria-label={`Play ${title}`}
        onClick={() => jump(entry)}
      >
        <Cover
          file={track?.cover}
          alt=""
          className={cn("size-8", played && "opacity-60")}
        />
        {current && (
          <span className="absolute inset-0 flex items-center justify-center bg-background/60">
            <Volume2Icon className="size-4 text-primary" />
          </span>
        )}
      </button>
      <div className="flex min-w-0 flex-1 flex-col">
        {track && (
          <>
            <Link
              to={`/tracks/${track.id}`}
              className="truncate hover:underline"
              title={track.title}
              draggable={false}
              onClick={onNavigate}
            >
              <Title
                title={track.title}
                language={track.titleLanguage ?? track.release.titleLanguage}
              />
            </Link>
            {track.performers.length > 0 && (
              <span
                className="truncate text-muted-foreground"
                onClickCapture={onNavigate}
              >
                <Names items={track.performers} to={artistUrl} />
              </span>
            )}
          </>
        )}
      </div>
      <span className="tabular-nums">{formatDuration(track?.durationMs)}</span>
      <Button
        variant="ghost"
        size="icon-xs"
        aria-label={`Remove ${title}`}
        onClick={() => remove(entry)}
      >
        <XIcon />
      </Button>
    </li>
  )
}
