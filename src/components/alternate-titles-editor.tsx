import { LanguagesIcon } from "lucide-react"
import { useState } from "react"

import { LanguagePicker } from "@/components/language-picker"
import { move, RowActions } from "@/components/ordered-list"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"
import { textProps } from "@/lib/languages"

/** An alternate title being edited (slunk's 3_alternate_names.md §7.6). */
export type AlternateTitleRow = {
  /** Tells rows apart */
  key: string
  title: string
  language: string | null
  /** On the release as published, rather than added in liquidslunk */
  printed: boolean
}

let lastKey = 0

/** A row for a title from the API, or a new one. */
export function alternateTitleRow(title: {
  title: string
  language?: string | null
  printed?: boolean
}): AlternateTitleRow {
  lastKey += 1
  return {
    key: `title-${lastKey}`,
    title: title.title,
    language: title.language ?? null,
    printed: title.printed ?? true,
  }
}

// Titles compare as the backend saves them: NFC, trimmed, case included
const clean = (text: string) => text.normalize("NFC").trim()

/** What's wrong with each row, or null. A blank row is left out on save. */
function rowProblems(main: string, rows: AlternateTitleRow[]) {
  const counts = new Map<string, number>()
  for (const row of rows) {
    const title = clean(row.title)
    if (title) counts.set(title, (counts.get(title) ?? 0) + 1)
  }
  return rows.map((row) => {
    const title = clean(row.title)
    if (!title) return null
    if (counts.get(title)! > 1) return "Listed twice"
    if (title === clean(main)) return "That's the main title"
    return null
  })
}

/** Whether the titles can be saved: the editor marks the rows that can't. */
export function alternateTitlesValid(
  main: string,
  rows: AlternateTitleRow[]
): boolean {
  return rowProblems(main, rows).every((problem) => problem === null)
}

/** The rows as `AlternateTitleInput`s. */
export function alternateTitlesInput(rows: AlternateTitleRow[]) {
  return rows.map(({ title, language, printed }) => ({
    title,
    language,
    printed,
  }))
}

/**
 * A release's, track's or disc's alternate titles, in order: each with a
 * language, and whether it's printed on the release. One that isn't, like a
 * translation, shows in italics. A title can't be listed twice, or be the
 * main title.
 */
export function AlternateTitlesEditor({
  main,
  titles,
  onChange,
}: {
  main: string
  titles: AlternateTitleRow[]
  onChange: (titles: AlternateTitleRow[]) => void
}) {
  const [adding, setAdding] = useState("")
  const problems = rowProblems(main, titles)
  const update = (index: number, changes: Partial<AlternateTitleRow>) =>
    onChange(titles.map((t, i) => (i === index ? { ...t, ...changes } : t)))

  function add() {
    if (!adding.trim()) return
    onChange([...titles, alternateTitleRow({ title: adding })])
    setAdding("")
  }

  return (
    <div className="flex flex-col gap-1">
      {titles.map((row, index) => (
        <div key={row.key} className="flex flex-col gap-1 border px-2 py-1">
          <div className="grid grid-cols-[1fr_12rem_auto_auto] items-center gap-2 text-xs">
            <Input
              aria-label="Alternate title"
              value={row.title}
              className={row.printed ? undefined : "italic"}
              {...textProps(row.language)}
              onChange={(e) => update(index, { title: e.target.value })}
            />
            <LanguagePicker
              aria-label={`Language of ${row.title}`}
              value={row.language}
              onChange={(language) => update(index, { language })}
            />
            <label className="flex items-center gap-1.5 whitespace-nowrap">
              <Checkbox
                checked={row.printed}
                onCheckedChange={(printed) => update(index, { printed })}
              />
              Printed
            </label>
            <RowActions
              label={row.title || "the title"}
              index={index}
              count={titles.length}
              onMove={(by) => onChange(move(titles, index, by))}
              onRemove={() => onChange(titles.filter((_, i) => i !== index))}
            />
          </div>
          {problems[index] && (
            <p className="text-xs text-destructive">{problems[index]}</p>
          )}
        </div>
      ))}
      <div className="flex gap-2">
        <Input
          aria-label="New alternate title"
          placeholder="A title, like シーナリィ or a translation"
          value={adding}
          onChange={(e) => setAdding(e.target.value)}
          onKeyDown={(event) => {
            // Enter adds the title, rather than saving the form
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
          Add title
        </Button>
      </div>
    </div>
  )
}

/**
 * A "Titles" button for a row of a table, like the tracks editor's
 * (slunk's 3_alternate_names.md §7.6): it opens a popover with the main title's
 * language and the alternate titles, so the row stays compact. It shows how
 * many alternate titles there are.
 */
export function TitlesPopover({
  main,
  language,
  titles,
  onChange,
  languagePlaceholder = "The release's",
  disabled,
}: {
  main: string
  language: string | null
  titles: AlternateTitleRow[]
  onChange: (language: string | null, titles: AlternateTitleRow[]) => void
  languagePlaceholder?: string
  disabled?: boolean
}) {
  const valid = alternateTitlesValid(main, titles)
  return (
    <Popover>
      <PopoverTrigger
        disabled={disabled}
        render={
          <Button
            type="button"
            size="sm"
            variant={valid ? "ghost" : "destructive"}
            aria-label={`Titles of ${main || "the track"}`}
          />
        }
      >
        <LanguagesIcon />
        {titles.length > 0 && titles.length}
      </PopoverTrigger>
      <PopoverContent align="start" className="w-[40rem] gap-3 text-xs">
        <div className="grid grid-cols-[auto_14rem] items-center gap-3">
          <span className="font-medium">Language</span>
          <LanguagePicker
            aria-label={`Language of ${main}`}
            placeholder={languagePlaceholder}
            value={language}
            onChange={(next) => onChange(next, titles)}
          />
        </div>
        <span className="font-medium">Alternate titles</span>
        <AlternateTitlesEditor
          main={main}
          titles={titles}
          onChange={(next) => onChange(language, next)}
        />
      </PopoverContent>
    </Popover>
  )
}
