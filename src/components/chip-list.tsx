import { XIcon } from "lucide-react"
import { type ReactNode, useState } from "react"

import { CreditedChipLabel } from "@/components/credited-as"
import { PendingArtistItem } from "@/components/pending-artist"
import type { Option, PickerProps } from "@/components/picker"
import { Badge } from "@/components/ui/badge"
import type { PendingArtist } from "@/lib/import"

/**
 * A compact list for table rows: chips in order, each removable, and a
 * picker that adds one at the end. With `credited`, the chips are artists:
 * each shows the name it's credited with, and opens a popover to change it.
 * `pending` are the import's tag names that picked no artist, each marked
 * after the chips until it's settled.
 */
export function ChipList({
  items,
  onChange,
  picker,
  disabled,
  credited = false,
  pending = [],
  onSettle,
}: {
  items: Option[]
  onChange: (items: Option[]) => void
  picker: (props: PickerProps) => ReactNode
  disabled?: boolean
  credited?: boolean
  pending?: PendingArtist[]
  /** A pending name's artist, or null when it's removed */
  onSettle?: (name: string, artist: Option | null) => void
}) {
  const [pickerKey, setPickerKey] = useState(0)
  return (
    <div className="flex min-w-40 flex-col gap-1">
      {items.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {items.map((item) => (
            <Badge key={item.id} variant="secondary" className="gap-1">
              {credited && !disabled ? (
                <CreditedChipLabel
                  artist={item}
                  onChange={(creditedAs) =>
                    onChange(
                      items.map((i) =>
                        i.id === item.id ? { ...i, creditedAs } : i
                      )
                    )
                  }
                />
              ) : (
                (item.creditedAs ?? item.name)
              )}
              {!disabled && (
                <button
                  type="button"
                  aria-label={`Remove ${item.name}`}
                  className="hover:text-foreground"
                  onClick={() =>
                    onChange(items.filter((i) => i.id !== item.id))
                  }
                >
                  <XIcon className="size-3" />
                </button>
              )}
            </Badge>
          ))}
        </div>
      )}
      {!disabled &&
        pending.map((artist) => (
          <PendingArtistItem
            key={artist.name}
            artist={artist}
            exclude={items.map((item) => item.id)}
            compact
            onSettle={(option) => onSettle?.(artist.name, option)}
          />
        ))}
      {!disabled && (
        <div key={pickerKey}>
          {picker({
            value: null,
            exclude: items.map((item) => item.id),
            placeholder: "Add",
            onChange: (option) => {
              if (!option) return
              onChange([...items, option])
              setPickerKey((key) => key + 1)
            },
          })}
        </div>
      )}
    </div>
  )
}
