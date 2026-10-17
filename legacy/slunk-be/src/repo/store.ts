import type { Kysely } from "kysely"
import type { CatalogStore } from "../slunk/catalog/repository.ts"
import type { TaggingStore } from "../slunk/tagging/repository.ts"
import {
  createArtistsRepository,
  createLabelsRepository,
} from "./artists_repository.ts"
import type { DB } from "./db_types.ts"
import { createFilesRepository } from "./files_repository.ts"
import {
  createCreditsRepository,
  createFileLinksRepository,
  createStatsRepository,
} from "./links_repositories.ts"
import { createListsRepository } from "./lists_repository.ts"
import { createReleasesRepository } from "./releases_repository.ts"
import { createTagsRepository } from "./tags_repository.ts"
import { createTracksRepository } from "./tracks_repository.ts"

/** What every context's store interface describes, in one. */
export interface Store extends CatalogStore, TaggingStore {
  transaction<T>(fn: (store: Store) => Promise<T>): Promise<T>
}

/** Every SQLite repository over one connection, or over one transaction. */
export function createStore(db: Kysely<DB>): Store {
  const store: Store = {
    artists: createArtistsRepository(db),
    labels: createLabelsRepository(db),
    releases: createReleasesRepository(db),
    tracks: createTracksRepository(db),
    lists: createListsRepository(db),
    credits: createCreditsRepository(db),
    stats: createStatsRepository(db),
    files: createFilesRepository(db),
    fileLinks: createFileLinksRepository(db),
    tags: createTagsRepository(db),
    // Inside a transaction already, nested work just joins it
    transaction: (fn) =>
      db.isTransaction
        ? fn(store)
        : db.transaction().execute((trx) => fn(createStore(trx))),
  }
  return store
}
