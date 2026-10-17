import { latestMigration } from "../../repo/migrator.ts"
import type { Resolvers } from "./resolvers_types.ts"

// Health is about the backend itself, not the domain, so it asks the repo
// directly instead of going through a context.
export const healthResolvers: Resolvers = {
  Query: {
    health: async (_parent, _args, { db, dbPath }) => ({
      database: dbPath,
      migration: await latestMigration(db),
    }),
  },
}
