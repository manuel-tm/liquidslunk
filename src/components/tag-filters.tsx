import { ListFilterIcon, XIcon } from "lucide-react"
import { useState } from "react"

import { type TagOption, TagPicker } from "@/components/pickers"
import { TagValueInput } from "@/components/tag-value-input"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { useCommand } from "@/lib/query"
import {
  describeTagFilter,
  type TagFilter,
  tagFilterKey,
  type TagFilterOp,
  tagFilterRefs,
  tagNames,
} from "@/lib/tags"

const TEXT_OPS = { equals: "is", contains: "contains" }

/**
 * A list's tag filters (slunk's design §7.6), on releases or tracks: a chip for
 * each, and a button that adds one. They all have to match. The chips name
 * their tags with `tagsByRef`.
 */
export function TagFilters({
  filters,
  onAdd,
  onRemove,
}: {
  filters: TagFilter[]
  onAdd: (params: [string, string][]) => void
  onRemove: (filter: TagFilter) => void
}) {
  const refs = tagFilterRefs(filters)
  const { data: tags } = useCommand(
    "tagsByRef",
    { refs },
    { enabled: refs.length > 0 }
  )
  const names = tagNames(tags)
  return (
    <>
      {filters.map((filter) => {
        const name =
          names.get(`${filter.type}:${filter.tagId}`) ??
          `${filter.type} tag ${filter.tagId}`
        const label = describeTagFilter(name, filter)
        return (
          <Badge
            key={`${filter.key}=${filter.value}`}
            variant="secondary"
            className="h-7 gap-1"
          >
            {label}
            <button
              type="button"
              aria-label={`Remove filter ${label}`}
              className="hover:text-foreground"
              onClick={() => onRemove(filter)}
            >
              <XIcon className="size-3" />
            </button>
          </Badge>
        )
      })}
      <AddTagFilter onAdd={onAdd} />
    </>
  )
}

function AddTagFilter({
  onAdd,
}: {
  onAdd: (params: [string, string][]) => void
}) {
  const [open, setOpen] = useState(false)
  const [tag, setTag] = useState<TagOption | null>(null)
  const [op, setOp] = useState<"equals" | "contains">("equals")
  const [value, setValue] = useState("")
  const [min, setMin] = useState("")
  const [max, setMax] = useState("")

  // A parameter for each filled-in part: a text match, or a range's ends
  const parts: [TagFilterOp, string][] = !tag
    ? []
    : tag.type === "text"
      ? [[op, value]]
      : [
          ["min", min],
          ["max", max],
        ]
  const params = parts.flatMap(([part, text]): [string, string][] =>
    tag && text.trim()
      ? [[tagFilterKey(tag.type, tag.tagId, part), text.trim()]]
      : []
  )

  function reset() {
    setTag(null)
    setOp("equals")
    setValue("")
    setMin("")
    setMax("")
  }

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (!next) reset()
      }}
    >
      <PopoverTrigger render={<Button variant="outline" />}>
        <ListFilterIcon data-icon="inline-start" />
        Tag filter
      </PopoverTrigger>
      <PopoverContent align="start" className="w-80">
        <form
          className="flex flex-col gap-2"
          onSubmit={(event) => {
            event.preventDefault()
            if (params.length === 0) return
            onAdd(params)
            setOpen(false)
            reset()
          }}
        >
          <TagPicker
            id="filter-tag"
            value={tag}
            onChange={setTag}
            placeholder="Search tags"
          />
          {tag?.type === "text" && (
            <div className="flex gap-2">
              <Select
                items={TEXT_OPS}
                value={op}
                onValueChange={(next) => next && setOp(next)}
              >
                <SelectTrigger aria-label="Match" className="w-28">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {Object.entries(TEXT_OPS).map(([key, label]) => (
                    <SelectItem key={key} value={key}>
                      {label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <TagValueInput
                id="filter-value"
                aria-label="Value"
                type="text"
                tagId={tag.tagId}
                value={value}
                onValueChange={setValue}
              />
            </div>
          )}
          {tag && tag.type !== "text" && (
            <>
              <div className="grid grid-cols-2 gap-2">
                <Input
                  aria-label="From"
                  inputMode={tag.type === "number" ? "decimal" : undefined}
                  placeholder={
                    tag.type === "date" ? "From: 2024-03" : "From: 6"
                  }
                  value={min}
                  onChange={(e) => setMin(e.target.value)}
                />
                <Input
                  aria-label="To"
                  inputMode={tag.type === "number" ? "decimal" : undefined}
                  placeholder={tag.type === "date" ? "To: 2024" : "To: 8.5"}
                  value={max}
                  onChange={(e) => setMax(e.target.value)}
                />
              </div>
              <p className="text-muted-foreground">
                Either end can be left empty, and both are included.
                {tag.type === "date" &&
                  " A date can be a year or a month: to 2023 runs through Dec 31, 2023."}
              </p>
            </>
          )}
          <Button type="submit" disabled={params.length === 0}>
            Add filter
          </Button>
        </form>
      </PopoverContent>
    </Popover>
  )
}
