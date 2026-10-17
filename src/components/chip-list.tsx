import { XIcon } from "lucide-react"
import { type ReactNode, useState } from "react"

import { CreditedChipLabel } from "@/components/credited-as"
import type { Option, PickerProps } from "@/components/picker"
import { Badge } from "@/components/ui/badge"

/**
 * A compact list for table rows: chips in order, each removable, and a
 * picker that adds one at the end. With `credited`, the chips are artists:
 * each shows the name it's credited with, and opens a popover to change it.
 * With `typed`, the picker has each of those names typed in, in turn.
 */
export function ChipList({
  items,
  onChange,
  picker,
  disabled,
  credited = false,
  typed = [],
}: {
  items: Option[]
  onChange: (items: Option[]) => void
  picker: (props: PickerProps) => ReactNode
  disabled?: boolean
  credited?: boolean
  /** Names typed into the picker in turn, a new one after each pick */
  typed?: string[]
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
      {!disabled && (
        <div key={pickerKey}>
          {picker({
            value: null,
            exclude: items.map((item) => item.id),
            placeholder: "Add",
            defaultQuery: typed[pickerKey],
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
