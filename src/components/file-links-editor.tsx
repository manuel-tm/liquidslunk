import { XIcon } from "lucide-react"
import { useState } from "react"

import type { Option } from "@/components/picker"
import { FilePicker } from "@/components/pickers"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"

export type FileLinkRow = { file: Option; note: string }

/** File links: a registered file each, with an optional note. */
export function FileLinksEditor({
  links,
  onChange,
}: {
  links: FileLinkRow[]
  onChange: (links: FileLinkRow[]) => void
}) {
  const [pickerKey, setPickerKey] = useState(0)

  return (
    <div className="flex flex-col gap-1">
      {links.map((link, index) => (
        <div
          key={link.file.id}
          className="grid grid-cols-[1fr_16rem_auto] items-center gap-2 border px-2 py-1 text-xs"
        >
          <span className="font-mono break-all">{link.file.name}</span>
          <Input
            aria-label={`Note for ${link.file.name}`}
            placeholder="Note, like back cover"
            value={link.note}
            onChange={(event) =>
              onChange(
                links.map((l, i) =>
                  i === index ? { ...l, note: event.target.value } : l
                )
              )
            }
          />
          <Button
            type="button"
            size="icon-xs"
            variant="ghost"
            aria-label={`Remove ${link.file.name}`}
            onClick={() => onChange(links.filter((_, i) => i !== index))}
          >
            <XIcon />
          </Button>
        </div>
      ))}
      <div key={pickerKey}>
        <FilePicker
          value={null}
          placeholder="Link a registered file"
          exclude={links.map((link) => link.file.id)}
          onChange={(file) => {
            if (!file) return
            onChange([...links, { file, note: "" }])
            setPickerKey((key) => key + 1)
          }}
        />
      </div>
    </div>
  )
}

/** The rows as the API's FileLinkInput. */
export function fileLinksInput(links: FileLinkRow[]) {
  return links.map((link) => ({
    fileId: link.file.id,
    note: link.note.trim() || null,
  }))
}
