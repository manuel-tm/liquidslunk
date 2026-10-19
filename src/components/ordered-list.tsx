import { ArrowDownIcon, ArrowUpIcon, XIcon } from "lucide-react"
import { type ReactNode, useState } from "react"

import { CreditedAsInput } from "@/components/credited-as"
import type { Option, PickerProps } from "@/components/picker"
import { Button } from "@/components/ui/button"
import { textProps } from "@/lib/languages"

/** Moves an item in a list by `by` places. */
export function move<T>(items: T[], index: number, by: number): T[] {
  const next = [...items]
  const [item] = next.splice(index, 1)
  next.splice(index + by, 0, item)
  return next
}

/** Up, down and remove buttons for a row of an ordered list. */
export function RowActions({
  label,
  index,
  count,
  onMove,
  onRemove,
}: {
  label: string
  index: number
  count: number
  onMove: (by: number) => void
  onRemove: () => void
}) {
  return (
    <div className="flex shrink-0">
      <Button
        type="button"
        size="icon-xs"
        variant="ghost"
        aria-label={`Move ${label} up`}
        disabled={index === 0}
        onClick={() => onMove(-1)}
      >
        <ArrowUpIcon />
      </Button>
      <Button
        type="button"
        size="icon-xs"
        variant="ghost"
        aria-label={`Move ${label} down`}
        disabled={index === count - 1}
        onClick={() => onMove(1)}
      >
        <ArrowDownIcon />
      </Button>
      <Button
        type="button"
        size="icon-xs"
        variant="ghost"
        aria-label={`Remove ${label}`}
        onClick={onRemove}
      >
        <XIcon />
      </Button>
    </div>
  )
}

/**
 * Picked rows, in order: the order sets `position`. A picker underneath adds
 * one. With `credited`, the rows are artists, each with the name it's credited
 * with. With `typed`, like the import's tag names that picked no artist, the
 * picker has each one typed in, in turn.
 */
export function OrderedList({
  items,
  onChange,
  picker,
  credited = false,
  typed = [],
}: {
  items: Option[]
  onChange: (items: Option[]) => void
  picker: (props: PickerProps) => ReactNode
  credited?: boolean
  /** Names typed into the picker in turn, a new one after each pick */
  typed?: string[]
}) {
  // A new key clears the picker after each pick
  const [pickerKey, setPickerKey] = useState(0)

  return (
    <div className="flex flex-col gap-1">
      {items.map((item, index) => (
        <div
          key={item.id}
          className="flex items-center gap-2 border px-2 py-1 text-xs"
        >
          <span className="flex-1" {...textProps(item.lang)}>
            {item.name}
          </span>
          {credited && (
            <CreditedAsInput
              artist={item}
              className="h-7 w-56"
              onChange={(creditedAs) =>
                onChange(
                  items.map((i, place) =>
                    place === index ? { ...i, creditedAs } : i
                  )
                )
              }
            />
          )}
          <RowActions
            label={item.name}
            index={index}
            count={items.length}
            onMove={(by) => onChange(move(items, index, by))}
            onRemove={() => onChange(items.filter((_, i) => i !== index))}
          />
        </div>
      ))}
      <div key={pickerKey}>
        {picker({
          value: null,
          exclude: items.map((item) => item.id),
          defaultQuery: typed[pickerKey],
          onChange: (option) => {
            if (!option) return
            onChange([...items, option])
            setPickerKey((key) => key + 1)
          },
        })}
      </div>
    </div>
  )
}
