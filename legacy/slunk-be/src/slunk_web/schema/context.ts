import type { Kysely } from "kysely"
import type { DB } from "../../repo/db_types.ts"
import type { Catalog } from "../../slunk/catalog.ts"
import type { LibraryCheck } from "../../slunk/library_check.ts"
import type { Media } from "../../slunk/media.ts"
import type { Tagging } from "../../slunk/tagging.ts"
import type { Loaders } from "./loaders.ts"

/** What the app is made of, built once when the server starts. */
export type App = {
  db: Kysely<DB>
  dbPath: string
  media: Media
  catalog: Catalog
  tagging: Tagging
  libraryCheck: LibraryCheck
}

/** What every resolver gets as its context: the app, and this request's loaders. */
export type Context = App & { loaders: Loaders }
