import { basename } from "node:path"
import type { Resolvers } from "./resolvers_types.ts"

export const mediaResolvers: Resolvers = {
  Query: {
    files: async (_parent, args, { media }) => {
      const { files, total } = await media.listFiles(args)
      return { items: files, total }
    },
    file: (_parent, { id }, { media }) => media.getFile(Number(id)),
    fileCounts: (_parent, _args, { media }) => media.fileCounts(),
    folder: (_parent, { path }, { media }) => media.browseFolder(path),
    folderFiles: (_parent, { path }, { media }) => media.scanFolder(path),
  },
  Mutation: {
    registerFiles: (_parent, { paths }, { media }) =>
      media.registerFiles(paths),
    updateFile: (_parent, { id, input }, { media }) =>
      media.updateFile(Number(id), input),
    deleteFile: async (_parent, { id }, { media }) => {
      await media.deleteFile(Number(id))
      return id
    },
  },
  File: {
    name: (file) => basename(file.path),
    onDisk: (file, _args, { media }) => media.fileOnDisk(file),
    track: (file, _args, { loaders }) => loaders.trackOfFile.load(file.id),
    uses: (file, _args, { media }) => media.fileUses(file.id),
  },
}
