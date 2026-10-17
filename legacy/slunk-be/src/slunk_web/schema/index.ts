import { readdirSync, readFileSync } from "node:fs"
import { join } from "node:path"
import { catalogResolvers } from "./catalog.ts"
import { healthResolvers } from "./health.ts"
import { libraryCheckResolvers } from "./library_check.ts"
import { mediaResolvers } from "./media.ts"
import { taggingResolvers } from "./tagging.ts"

/** The SDL of every .graphql file in this folder. */
export const typeDefs = readdirSync(import.meta.dirname)
  .filter((file) => file.endsWith(".graphql"))
  .sort()
  .map((file) => readFileSync(join(import.meta.dirname, file), "utf8"))

export const resolvers = [
  healthResolvers,
  mediaResolvers,
  catalogResolvers,
  taggingResolvers,
  libraryCheckResolvers,
]
