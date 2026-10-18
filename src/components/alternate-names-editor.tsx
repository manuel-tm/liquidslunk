import { cn } from "cn"
import { useState } from "react"

import { LanguagePicker } from "@/components/language-picker"
import { move, RowActions } from "@/components/ordered-list"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { textProps } from "@/lib/languages"

/** An alternate name being edited (slunk's 3_alternate_names.md §7.6). */
export type AlternateNameRow = {
  /** Tells rows apart, before they're saved too */
  key: string
  /** Null for a name that isn't saved yet */
  id: number | null
  name: string
  language: string | null
  realName: boolean
  note: string
  /** Artist links credited with it, for a saved artist name */
  uses: number | null
}

let lastKey = 0

/** A row for a name from the backend, or a new one. */
export function alternateNameRow(name: {
  id?: number | null
  name: string
  language?: string | null
  realName?: boolean
  note?: string | null
  uses?: number | null
}): AlternateNameRow {
  lastKey += 1
  return {
    key: `name-${lastKey}`,
    id: name.id ?? null,
    name: name.name,
    language: name.language ?? null,
    realName: name.realName ?? false,
    note: name.note ?? "",
    uses: name.uses ?? null,
  }
}

// Names compare as the backend saves them: NFC, trimmed, case included
export const cleanName = (text: string) => text.normalize("NFC").trim()

const clean = cleanName

/** What's wrong with each row, or null. A blank row is left out on save. */
function rowProblems(
  main: string,
  rows: AlternateNameRow[],
  realNames: boolean
): (string | null)[] {
  const counts = new Map<string, number>()
  for (const row of rows) {
    const name = clean(row.name)
    if (name) counts.set(name, (counts.get(name) ?? 0) + 1)
  }
  return rows.map((row) => {
    const name = clean(row.name)
    if (!name) return null
    if (counts.get(name)! > 1) return "Listed twice"
    if (name === clean(main) && !(realNames && row.realName)) {
      return realNames
        ? "That's the main name. Only a real name can be both."
        : "That's the main name"
    }
    return null
  })
}

/** Whether the names can be saved: the editor marks the rows that can't. */
export function alternateNamesValid(
  main: string,
  rows: AlternateNameRow[],
  realNames: boolean
): boolean {
  return rowProblems(main, rows, realNames).every((problem) => problem === null)
}

/** The rows as `ArtistNameInput`s. */
export function artistNamesInput(rows: AlternateNameRow[]) {
  return rows.map((row) => ({
    id: row.id,
    name: row.name,
    language: row.language,
    realName: row.realName,
    note: row.note.trim() || null,
  }))
}

/** The rows as `LabelNameInput`s. */
export function labelNamesInput(rows: AlternateNameRow[]) {
  return rows.map((row) => ({
    name: row.name,
    language: row.language,
    note: row.note.trim() || null,
  }))
}

/**
 * An artist's or label's alternate names, in order: each with a language
 * and a note, and for an artist, whether it's a real name and how many links
 * it's credited on. A name can't be listed twice, or be the main name unless
 * it's a real name. With `onMakeMain`, each row can become the main name.
 */
export function AlternateNamesEditor({
  main,
  names,
  onChange,
  realNames,
  onMakeMain,
}: {
  main: string
  names: AlternateNameRow[]
  onChange: (names: AlternateNameRow[]) => void
  /** Artists have real names, labels don't */
  realNames: boolean
  onMakeMain?: (row: AlternateNameRow) => void
}) {
  const [adding, setAdding] = useState("")
  const problems = rowProblems(main, names, realNames)
  const update = (index: number, changes: Partial<AlternateNameRow>) =>
    onChange(names.map((n, i) => (i === index ? { ...n, ...changes } : n)))

  function add() {
    if (!adding.trim()) return
    onChange([...names, alternateNameRow({ name: adding })])
    setAdding("")
  }

  return (
    <div className="flex flex-col gap-1">
      {names.map((row, index) => (
        <div key={row.key} className="flex flex-col gap-1 border px-2 py-1">
          <div
            className={cn(
              "grid items-center gap-2 text-xs",
              realNames
                ? "grid-cols-[1fr_12rem_auto_1fr_auto_auto]"
                : "grid-cols-[1fr_12rem_1fr_auto_auto]"
            )}
          >
            <Input
              aria-label="Name"
              value={row.name}
              {...textProps(row.language)}
              onChange={(e) => update(index, { name: e.target.value })}
            />
            <LanguagePicker
              aria-label={`Language of ${row.name}`}
              value={row.language}
              onChange={(language) => update(index, { language })}
            />
            {realNames && (
              <label className="flex items-center gap-1.5 whitespace-nowrap">
                <Checkbox
                  checked={row.realName}
                  onCheckedChange={(realName) => update(index, { realName })}
                />
                Real name
              </label>
            )}
            <Input
              aria-label={`Note for ${row.name}`}
              placeholder="Note, like birth name"
              value={row.note}
              onChange={(e) => update(index, { note: e.target.value })}
            />
            <div
              className={cn(
                "flex items-center justify-end gap-2 whitespace-nowrap",
                // A fixed width keeps the rows' columns lined up
                onMakeMain && "w-36"
              )}
            >
              {row.uses !== null && (
                <span className="text-muted-foreground tabular-nums">
                  {row.uses === 1 ? "1 use" : `${row.uses} uses`}
                </span>
              )}
              {onMakeMain && (
                <Button
                  type="button"
                  size="xs"
                  variant="ghost"
                  disabled={!clean(row.name)}
                  onClick={() => onMakeMain(row)}
                >
                  Make main
                </Button>
              )}
            </div>
            <RowActions
              label={row.name || "the name"}
              index={index}
              count={names.length}
              onMove={(by) => onChange(move(names, index, by))}
              onRemove={() => onChange(names.filter((_, i) => i !== index))}
            />
          </div>
          {problems[index] && (
            <p className="text-xs text-destructive">{problems[index]}</p>
          )}
        </div>
      ))}
      <div className="flex gap-2">
        <Input
          aria-label="New alternate name"
          placeholder={realNames ? "A name, like ALIKA or 福居良" : "A name"}
          value={adding}
          onChange={(e) => setAdding(e.target.value)}
          onKeyDown={(event) => {
            // Enter adds the name, rather than saving the form
            if (event.key !== "Enter") return
            event.preventDefault()
            add()
          }}
        />
        <Button
          type="button"
          variant="outline"
          disabled={!adding.trim()}
          onClick={add}
        >
          Add name
        </Button>
      </div>
    </div>
  )
}
