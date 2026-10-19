import { ArrowLeftRightIcon, SplitIcon } from "lucide-react"
import { useState } from "react"

import {
  type AlternateTitleRow,
  alternateTitleRow,
} from "@/components/alternate-titles-editor"
import { Button } from "@/components/ui/button"
import { languageName, scriptLanguage, textProps } from "@/lib/languages"
import { titleParts } from "@/lib/titles"

/** A title, its language, and its alternate titles: what a split changes. */
export type TitleValues = {
  title: string
  language: string | null
  alternateTitles: AlternateTitleRow[]
}

/**
 * A title split at " = ": the first part is the main title, and the others are
 * printed alternate titles, each with the language its script gives away. The
 * main title keeps its language unless its script gives one. Null when there's
 * nothing to split.
 */
export function splitTitle(values: TitleValues): TitleValues | null {
  const parts = titleParts(values.title)
  if (!parts) return null
  const [main, ...others] = parts
  return {
    title: main,
    language: scriptLanguage(main) ?? values.language,
    alternateTitles: [
      ...others.map((title) =>
        alternateTitleRow({ title, language: scriptLanguage(title) })
      ),
      // Alternate titles it had already stay, after the parts
      ...values.alternateTitles.filter(
        (row) => !parts.includes(row.title.normalize("NFC").trim())
      ),
    ],
  }
}

/**
 * The main title and the first alternate one trade places, languages
 * included: "Swap", for a user who wants the original language first.
 */
export function swapTitle(values: TitleValues): TitleValues {
  const [first, ...rest] = values.alternateTitles
  if (!first) return values
  return {
    title: first.title,
    language: first.language,
    alternateTitles: [
      { ...first, title: values.title, language: values.language },
      ...rest,
    ],
  }
}

/** "シーナリィ" (Japanese): a part, marked with the language it'd get. */
function Part({ title }: { title: string }) {
  const language = scriptLanguage(title)
  return (
    <>
      "<span {...textProps(language)}>{title}</span>"
      {language && (
        <span className="text-muted-foreground">
          {" "}
          ({languageName(language)})
        </span>
      )}
    </>
  )
}

/**
 * The import's offer to split a title with " = " in it, under the title field.
 * Nothing is split until the user says so, since "=" can be part of a title.
 * Once it's split, "Swap" makes the other part the main title.
 */
export function SplitTitleOffer({
  values,
  onChange,
}: {
  values: TitleValues
  onChange: (values: TitleValues) => void
}) {
  const [split, setSplit] = useState(false)
  const parts = titleParts(values.title)
  const first = values.alternateTitles[0]
  if (!parts && !(split && first)) return null
  return (
    <div className="flex items-center gap-3 border px-2 py-1.5 text-xs">
      {parts ? (
        <>
          <span className="flex-1">
            This may be one title, printed in {parts.length} languages. Split
            it: <Part title={parts[0]} /> as the main title, and{" "}
            {parts.slice(1).map((part, index) => (
              <span key={part}>
                {index > 0 && ", "}
                <Part title={part} />
              </span>
            ))}{" "}
            as {parts.length > 2 ? "alternate titles" : "an alternate title"}.
          </span>
          <Button
            type="button"
            size="xs"
            variant="outline"
            onClick={() => {
              onChange(splitTitle(values)!)
              setSplit(true)
            }}
          >
            <SplitIcon />
            Split
          </Button>
        </>
      ) : (
        <>
          <span className="flex-1">
            Split at " = ". Swap to make "
            <span {...textProps(first.language)}>{first.title}</span>" the main
            title instead of "
            <span {...textProps(values.language)}>{values.title}</span>".
          </span>
          <Button
            type="button"
            size="xs"
            variant="outline"
            onClick={() => onChange(swapTitle(values))}
          >
            <ArrowLeftRightIcon />
            Swap
          </Button>
        </>
      )}
    </div>
  )
}

/**
 * A track row's split, as an icon button: it splits a title with " = " in it,
 * and once the row is split, swaps its main title with the first alternate one.
 */
export function SplitTitleButton({
  values,
  split,
  disabled,
  onChange,
}: {
  values: TitleValues
  /** The row was split, so it can be swapped */
  split: boolean
  disabled?: boolean
  onChange: (values: TitleValues, split: boolean) => void
}) {
  const next = splitTitle(values)
  const first = values.alternateTitles[0]
  if (next) {
    return (
      <Button
        type="button"
        size="icon-sm"
        variant="ghost"
        disabled={disabled}
        aria-label={`Split ${values.title}`}
        title={`Split at " = ": ${next.title} is the main title`}
        onClick={() => onChange(next, true)}
      >
        <SplitIcon />
      </Button>
    )
  }
  if (!split || !first) return null
  return (
    <Button
      type="button"
      size="icon-sm"
      variant="ghost"
      disabled={disabled}
      aria-label={`Swap ${values.title} and ${first.title}`}
      title={`Swap: make ${first.title} the main title`}
      onClick={() => onChange(swapTitle(values), true)}
    >
      <ArrowLeftRightIcon />
    </Button>
  )
}
