import type { Resolvers } from "./resolvers_types.ts"

export const libraryCheckResolvers: Resolvers = {
  Query: {
    libraryCheck: async (_parent, _args, { libraryCheck }) => {
      const check = await libraryCheck.run()
      return {
        fileCount: check.fileCount,
        missingFiles: check.missing,
        changedFiles: check.changed,
        unregisteredFolders: check.unregistered.map((folder) => ({
          path: folder.path,
          files: folder.files.map((file) => ({ ...file, file: null })),
        })),
        releasesWithoutArtists: check.releasesWithoutArtists,
        tracksWithoutPerformers: check.tracksWithoutPerformers,
      }
    },
  },

  Mutation: {
    relinkFiles: (_parent, { moves }, { libraryCheck }) =>
      libraryCheck.relinkFiles(
        moves.map((move) => ({ fileId: Number(move.fileId), path: move.path }))
      ),
    rehashFiles: (_parent, { ids }, { libraryCheck }) =>
      libraryCheck.rehashFiles(ids.map(Number)),
  },
}
