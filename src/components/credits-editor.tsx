import { ArrowLeftIcon, XIcon } from "lucide-react"
import { type ComponentProps, useId, useState } from "react"

import { move, RowActions } from "@/components/ordered-list"
import { CreditedChipLabel } from "@/components/credited-as"
import type { Option } from "@/components/picker"
import { ArtistPicker, useCreditRoles } from "@/components/pickers"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { creditedInput } from "@/lib/credited"
import { roleKey } from "@/lib/format"

/** A credit line: a role as printed, and the artists who hold it, in order. */
export type CreditLine = { role: string; artists: Option[] }

/** What's wrong with each line, or null. */
function lineProblems(lines: CreditLine[]): (string | null)[] {
  const roles = new Map<string, number>()
  for (const { role } of lines) {
    roles.set(roleKey(role), (roles.get(roleKey(role)) ?? 0) + 1)
  }
  return lines.map(({ role, artists }) =>
    !role.trim()
      ? "Needs a role"
      : roles.get(roleKey(role))! > 1
        ? "Another line has this role, ignoring case"
        : artists.length === 0
          ? "Needs an artist"
          : null
  )
}

/** Whether the lines can be saved: the editor marks those that can't. */
export function creditsValid(lines: CreditLine[]): boolean {
  return lineProblems(lines).every((problem) => problem === null)
}

/** The lines as `CreditInput`s. */
export function creditsInput(lines: CreditLine[]) {
  return lines.map(({ role, artists }) => ({
    role,
    artists: creditedInput(artists),
  }))
}

/**
 * Credit lines, in display order (slunk's 2_credits.md §7): a role as printed, then
 * its artists. Underneath, a role and an artist add a line, or add the
 * artist to the line that already has the role.
 */
export function CreditsEditor({
  credits,
  onChange,
}: {
  credits: CreditLine[]
  onChange: (credits: CreditLine[]) => void
}) {
  const problems = lineProblems(credits)
  return (
    <div className="flex flex-col gap-1">
      {credits.map((line, index) => (
        // By index, so editing a role doesn't remount its field
        <LineRow
          key={index}
          line={line}
          problem={problems[index]}
          index={index}
          count={credits.length}
          onChange={(changed) =>
            onChange(credits.map((l, i) => (i === index ? changed : l)))
          }
          onMove={(by) => onChange(move(credits, index, by))}
          onRemove={() => onChange(credits.filter((_, i) => i !== index))}
        />
      ))}
      <LineAdder credits={credits} onChange={onChange} />
    </div>
  )
}

function LineRow({
  line,
  problem,
  index,
  count,
  onChange,
  onMove,
  onRemove,
}: {
  line: CreditLine
  problem: string | null
  index: number
  count: number
  onChange: (line: CreditLine) => void
  onMove: (by: number) => void
  onRemove: () => void
}) {
  // A new key clears the picker after each pick
  const [pickerKey, setPickerKey] = useState(0)
  const { role, artists } = line
  const label = role.trim() || "credit"
  return (
    <div className="flex flex-col gap-1 border px-2 py-1 text-xs">
      <div className="flex items-start gap-2">
        <RoleInput
          aria-label={`Role of line ${index + 1}`}
          className="w-40 shrink-0"
          value={role}
          onValueChange={(changed) => onChange({ ...line, role: changed })}
          aria-invalid={problem !== null && problem !== "Needs an artist"}
        />
        <div className="flex flex-1 flex-wrap items-center gap-1">
          {artists.map((artist, place) => (
            <Badge key={artist.id} variant="secondary" className="gap-1">
              {place > 0 && (
                <button
                  type="button"
                  aria-label={`Move ${artist.name} earlier`}
                  className="hover:text-foreground"
                  onClick={() =>
                    onChange({ ...line, artists: move(artists, place, -1) })
                  }
                >
                  <ArrowLeftIcon className="size-3" />
                </button>
              )}
              <CreditedChipLabel
                artist={artist}
                onChange={(creditedAs) =>
                  onChange({
                    ...line,
                    artists: artists.map((a) =>
                      a.id === artist.id ? { ...a, creditedAs } : a
                    ),
                  })
                }
              />
              <button
                type="button"
                aria-label={`Remove ${artist.name} from ${label}`}
                className="hover:text-foreground"
                onClick={() =>
                  onChange({
                    ...line,
                    artists: artists.filter((a) => a.id !== artist.id),
                  })
                }
              >
                <XIcon className="size-3" />
              </button>
            </Badge>
          ))}
          <div key={pickerKey} className="w-40">
            <ArtistPicker
              value={null}
              exclude={artists.map((artist) => artist.id)}
              placeholder="Add artist"
              onChange={(artist) => {
                if (!artist) return
                onChange({ ...line, artists: [...artists, artist] })
                setPickerKey((key) => key + 1)
              }}
            />
          </div>
        </div>
        <RowActions
          label={label}
          index={index}
          count={count}
          onMove={onMove}
          onRemove={onRemove}
        />
      </div>
      {problem && <p className="text-destructive">{problem}</p>}
    </div>
  )
}

function LineAdder({
  credits,
  onChange,
}: {
  credits: CreditLine[]
  onChange: (credits: CreditLine[]) => void
}) {
  const [role, setRole] = useState("")
  const [artist, setArtist] = useState<Option | null>(null)
  const [adderKey, setAdderKey] = useState(0)
  const ready = role.trim() !== "" && artist !== null

  function add() {
    if (!role.trim() || !artist) return
    const index = credits.findIndex(
      (line) => roleKey(line.role) === roleKey(role)
    )
    if (index === -1) {
      onChange([...credits, { role: role.trim(), artists: [artist] }])
    } else if (!credits[index].artists.some((a) => a.id === artist.id)) {
      onChange(
        credits.map((line, i) =>
          i === index ? { ...line, artists: [...line.artists, artist] } : line
        )
      )
    }
    setRole("")
    setArtist(null)
    setAdderKey((key) => key + 1)
  }

  return (
    <div key={adderKey} className="grid grid-cols-[1fr_1fr_auto] gap-2">
      <RoleInput
        aria-label="Role"
        placeholder="Role"
        value={role}
        onValueChange={setRole}
        onKeyDown={(event) => {
          // Enter adds the credit, rather than saving the form
          if (event.key !== "Enter") return
          event.preventDefault()
          add()
        }}
      />
      <ArtistPicker value={artist} onChange={setArtist} placeholder="Artist" />
      <Button type="button" variant="outline" disabled={!ready} onClick={add}>
        Add credit
      </Button>
    </div>
  )
}

/**
 * A role typed as printed. It suggests every spelling already in use, like a
 * text tag's value; whatever is typed is the role.
 */
function RoleInput({
  value,
  onValueChange,
  ...props
}: Omit<ComponentProps<typeof Input>, "value" | "list" | "onChange"> & {
  value: string
  onValueChange: (value: string) => void
}) {
  const id = useId()
  // Only the field being typed in asks for suggestions
  const [focused, setFocused] = useState(false)
  const suggestions = useCreditRoles(value, !focused)
  return (
    <>
      <Input
        {...props}
        list={`${id}-roles`}
        value={value}
        onChange={(e) => onValueChange(e.target.value)}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
      />
      <datalist id={`${id}-roles`}>
        {suggestions.map((suggestion) => (
          <option key={suggestion} value={suggestion} />
        ))}
      </datalist>
    </>
  )
}
