// Generates expected.json: what Node's Intl.getCanonicalLocales gives for
// language tags, which slunk saved names' and titles' languages with
// (backend.md, "Languages"). It's what tests/languages.rs checks
// languages.rs against:
// - `aliases`: every 2- and 3-letter lowercase code that it changes, with
//   what it gives. Every other one stays as it is.
// - `cases`: tags around them, with scripts, regions, variants, extensions
//   and their deprecated values, and what it refuses (null).
//
// Run from the repository root:
//
//   node crates/slunk/tests/fixtures/languages/generate.ts
//
// What it gives depends on Node's ICU, so expected.json records the
// versions.
import { writeFileSync } from "node:fs"
import { join } from "node:path"

const LETTERS = "abcdefghijklmnopqrstuvwxyz"

function canonical(tag: string): string | null {
  try {
    return Intl.getCanonicalLocales(tag)[0]
  } catch {
    return null
  }
}

const aliases: Record<string, string | null> = {}
for (const a of LETTERS) {
  for (const b of LETTERS) {
    for (const code of [a + b, ...[...LETTERS].map((c) => a + b + c)]) {
      const result = canonical(code)
      if (result !== code) aliases[code] = result
    }
  }
}

const languages = [
  ..."ja en zh sr hy ru sh iw no sgn cnr bh tw jaw art und".split(" "),
]
const scripts = ["Latn", "Cyrl", "Hant", "kana", "Qaaa", "Zzzz"]
const regions = ["US", "jp", "SU", "DD", "YU", "CS", "BU", "419", "062", "XK"]
const variants = [
  "1901",
  "1996",
  "posix",
  "hepburn",
  "arevela",
  "arevmda",
  "rozaj-biske",
  "biske-rozaj",
  "1996-1901",
  "1901-posix",
  "posix-hepburn",
  "lojban",
  "guoyu",
  "oed",
  "bokmal",
]
const keywords: [string, string[]][] = [
  ["ca", ["gregory", "islamicc", "ISLAMICC", "ethiopic-amete-alem", "yes"]],
  ["ks", ["primary", "tertiary", "level2", "yes"]],
  ["ms", ["imperial", "metric"]],
  ["kb", ["yes", "true", "no", "false", "yes-yes"]],
  ["co", ["phonebk", "yes"]],
  ["nu", ["latn", "traditio"]],
  ["sd", ["cn11", "frg", "usca", "gbsct"]],
  ["rg", ["cn11zzzz", "gbzzzz"]],
  [
    "tz",
    [
      ..."aqams aukns caffs camtr canpg capnt cathu cayzf cet cnckg cnhrb cnkhg cst6cdt cuba eet egypt eire est est5edt factory gaza gmt0 hongkong hst iceland iran israel jamaica japan libya met mncoq mst mst7mdt navajo poland portugal prc pst8pdt roc rok turkey uaozh uauzh uct umjon usnavajo wet zulu".split(
        " "
      ),
      "utc",
      "usnyc",
      "asia",
    ],
  ],
]
const others = [
  ..."ja JA Ja jA ja-Latn JA-latn ja-LATN-jp ja-Latn-JP-hepburn".split(" "),
  ..."bh BH Bh tw TW jaw JAW fil FIL fil-PH in IN iw iw-IL no NO sh Sh".split(
    " "
  ),
  ..."zh-Hans-CN zh-hant-tw ZH-HANT-TW sr-latn-rs az-cyrl-az es-419 ar-001".split(
    " "
  ),
  ..."art-lojban zh-guoyu zh-min-nan i-klingon en-GB-oed sgn-BR".split(" "),
  ..."x-foo en-x-foo en-gb-x-private EN-GB-x-Foo en-x-posix".split(" "),
  ..."en-a-bbb-x-a-ccc EN-a-AAA-b-BBB en-z-zz en-a-aa-a-bb en-u-aa-u-bb".split(
    " "
  ),
  ..."en-u-attr-co-phonebk en-u-attr1-attr2-ca-islamicc en-u-ca en-u-kb".split(
    " "
  ),
  ..."en-u-ca-japanese-ca-buddhist en-u-ks-tertiary-kn-yes en-u-kb-yes-ca-islamicc".split(
    " "
  ),
  ..."en-u-CA-Gregory EN-U-KB-YES en-U-CA-GREGORY-NU-LATN".split(" "),
  ..."en-u-ca-gregorian en-u-co-phonebook en-u-ks-identical".split(" "),
  ..."en-t-ja en-t-iw en-t-iw-u-ca-islamicc en-t-ja-m0-names en-t-m0-names".split(
    " "
  ),
  ..."en-t-d0-name en-t-ja-m0-names-x-foo en-t-m0-yes en-t-ja-d0-yes".split(
    " "
  ),
  ..."en-US-POSIX en-posix-u-va-foo en-posix-x-foo en-posix-t-ja".split(" "),
  ..."en-US-posix-u-ca-gregory de-1901-posix-1996 de-1901-1901".split(" "),
  ..."und und-Latn root japanese abcd abcdefghi a toolongsubtag".split(" "),
  ..."JAPANESE-latn abcde abcdefgh japanese-x-foo abcde-US-posix abcde-SU".split(
    " "
  ),
  ..."abcde-u-ca-islamicc abcde-aaland abcde-1901-1901 abcde-hepburn-heploc".split(
    " "
  ),
  ..."ja_JP zh_Hant en- -en en--US ja-u- zh-cmn zh-yue no-bok".split(" "),
  "",
  " ja",
  "ja ",
  "ja ",
  "日本語",
  "ja-JP-u-ca-japanese",
  "ja-u-nu-latn-ca-japanese",
]

const inputs = new Set(others)
for (const language of languages) {
  for (const script of scripts) inputs.add(`${language}-${script}`)
  for (const region of regions) inputs.add(`${language}-${region}`)
}
for (const language of ["en", "ja-Latn", "hy", "sl", "de-CH", "art", "sgn"]) {
  for (const variant of variants) inputs.add(`${language}-${variant}`)
}
for (const [key, values] of keywords) {
  for (const value of values) inputs.add(`en-u-${key}-${value}`)
}

const cases = [...inputs].map((input) => [input, canonical(input)])
const expected = {
  generatedBy: "crates/slunk/tests/fixtures/languages/generate.ts",
  node: process.versions.node,
  icu: process.versions.icu,
  cldr: process.versions.cldr,
  aliases,
  cases,
}
writeFileSync(
  join(import.meta.dirname, "expected.json"),
  `${JSON.stringify(expected, null, 1)}\n`
)
console.log(
  `${Object.keys(aliases).length} aliases and ${cases.length} cases, with Node ${process.versions.node}`
)
