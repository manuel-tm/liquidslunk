// Languages of names and titles: BCP 47 tags like ja or ja-Latn. The browser
// names them, so liquidslunk keeps no list.

const names = new Intl.DisplayNames(["en"], { type: "language" })

/** A tag's name: "Japanese (Latin)" for ja-Latn, or the tag if it has none. */
export function languageName(tag: string): string {
  try {
    return names.of(tag) ?? tag
  } catch {
    return tag
  }
}

/**
 * A tag in canonical form, as the backend saves it ("JA-latn" is "ja-Latn"),
 * or null if it isn't a language tag.
 */
export function canonicalTag(text: string): string | null {
  try {
    const [tag] = Intl.getCanonicalLocales(text.trim())
    const language = tag.split("-")[0]
    return /^[a-z]{2,3}$/.test(language) && language !== "und" ? tag : null
  } catch {
    return null
  }
}

/**
 * The language a name's or title's script gives away, as the backend guesses it
 * for a credited name: kana are only Japanese, and Hangul only Korean. Han
 * characters could be Chinese or Japanese, so they give none.
 */
export function scriptLanguage(text: string): string | null {
  if (/[\p{Script=Hiragana}\p{Script=Katakana}]/u.test(text)) return "ja"
  if (/\p{Script=Hangul}/u.test(text)) return "ko"
  return null
}

/**
 * `lang` and `dir` for a name or title. `lang` picks the glyphs, since Japanese
 * and Chinese share code points but not shapes. `dir="auto"` keeps a
 * right-to-left name from moving the dashes and commas around it.
 */
export function textProps(language: string | null | undefined) {
  return { lang: language ?? undefined, dir: "auto" as const }
}

/**
 * What the language picker offers besides the tags in use: every ISO 639-1
 * language, and the script variants music needs most.
 */
export const LANGUAGES = [
  ..."aa ab ae af ak am an ar as av ay az ba be bg bi bm bn bo br bs ca ce ch co cr cs cu cv cy da de dv dz ee el en eo es et eu fa ff fi fj fo fr fy ga gd gl gn gu gv ha he hi ho hr ht hu hy hz ia id ie ig ii ik io is it iu ja jv ka kg ki kj kk kl km kn ko kr ks ku kv kw ky la lb lg li ln lo lt lu lv mg mh mi mk ml mn mr ms mt my na nb nd ne ng nl nn no nr nv ny oc oj om or os pa pi pl ps pt qu rm rn ro ru rw sa sc sd se sg si sk sl sm sn so sq sr ss st su sv sw ta te tg th ti tk tl tn to tr ts tt tw ty ug uk ur uz ve vi vo wa wo xh yi yo za zh zu".split(
    " "
  ),
  "ja-Latn",
  "zh-Hans",
  "zh-Hant",
  "sr-Cyrl",
  "sr-Latn",
  "ko-Latn",
  "ru-Latn",
].sort((a, b) => languageName(a).localeCompare(languageName(b)))
