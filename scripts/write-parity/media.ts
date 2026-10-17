// Phase 1's steps: the media context, files and the library on disk. The
// library's files are in index.ts (`makeFiles`).

/** A step: the same request through GraphQL and through a command. */
export type Step = {
  name: string
  graphql: string
  /** Given the library's folder, for absolute paths */
  variables?: (library: string) => object
  /** The command's name in the bindings */
  command: string
  args?: (library: string) => object
  /** GraphQL returns an id, a string, where the command returns a number */
  returnsId?: boolean
}

export type Phase = {
  /** Requests through GraphQL, before the library is copied */
  prepare: { graphql: string; variables?: object }[]
  steps: Step[]
}

const FILE = "id path name type sizeBytes mtime sha256"

const files = (name: string, args: Record<string, unknown> = {}): Step => ({
  name: `files ${name}`,
  graphql: `query ($page: Int, $perPage: Int, $sort: String, $q: String, $type: FileType, $unused: Boolean, $withoutTrack: Boolean) {
    files(page: $page, perPage: $perPage, sort: $sort, q: $q, type: $type, unused: $unused, withoutTrack: $withoutTrack) {
      total items { id path type sizeBytes mtime sha256 track { id title } }
    }
  }`,
  variables: () => args,
  command: "files",
  args: () => args,
})

const file = (id: number): Step => ({
  name: `file ${id}`,
  graphql: `query ($id: ID!) {
    file(id: $id) { ${FILE} onDisk uses { kind entityId name note } }
  }`,
  variables: () => ({ id }),
  command: "file",
  args: () => ({ id }),
})

const fileCounts: Step = {
  name: "fileCounts",
  graphql: "{ fileCounts { total unused } }",
  command: "fileCounts",
}

const folder = (path: string): Step => ({
  name: `folder "${path}"`,
  graphql: `query ($path: String!) {
    folder(path: $path) {
      path absolutePath parent
      folders { name path hasRegisteredFiles }
      files { name path type sizeBytes file { id } }
    }
  }`,
  variables: () => ({ path }),
  command: "folder",
  args: () => ({ path }),
})

const folderFiles = (path: string): Step => ({
  name: `folderFiles "${path}"`,
  graphql: `query ($path: String!) {
    folderFiles(path: $path) { name path type sizeBytes file { id } }
  }`,
  variables: () => ({ path }),
  command: "folderFiles",
  args: () => ({ path }),
})

const register = (
  name: string,
  paths: (library: string) => string[]
): Step => ({
  name: `registerFiles ${name}`,
  graphql: `mutation ($paths: [String!]!) { registerFiles(paths: $paths) { ${FILE} } }`,
  variables: (library) => ({ paths: paths(library) }),
  command: "registerFiles",
  args: (library) => ({ paths: paths(library) }),
})

const update = (
  name: string,
  id: number,
  input: { path?: string; type?: string }
): Step => ({
  name: `updateFile ${id} ${name}`,
  graphql: `mutation ($id: ID!, $input: UpdateFileInput!) {
    updateFile(id: $id, input: $input) { ${FILE} }
  }`,
  variables: () => ({ id, input }),
  command: "updateFile",
  args: () => ({ id, ...input }),
})

const remove = (id: number): Step => ({
  name: `deleteFile ${id}`,
  graphql: "mutation ($id: ID!) { deleteFile(id: $id) }",
  variables: () => ({ id }),
  command: "deleteFile",
  args: () => ({ id }),
  returnsId: true,
})

export const media: Phase = {
  // A track's audio (1), an artist's picture (2) and a release's file link
  // (3), which keep their files from being deleted
  prepare: [
    {
      graphql: `mutation { registerFiles(paths: ["music/Ryo Fukui/2 Early Summer.flac"]) { id } }`,
    },
    {
      graphql: `mutation { createTrack(input: { releaseId: 1, fileId: 1, trackNumber: 1, title: "Early Summer" }) { id } }`,
    },
    {
      graphql: `mutation { setArtistPicture(id: 1, picture: { path: "music/Ryo Fukui/cover.jpg" }) { id } }`,
    },
    {
      graphql: `mutation { setReleaseFiles(id: 1, input: { files: [{ path: "music/Ryo Fukui/rip.log", note: "Rip log" }] }) { id } }`,
    },
  ],
  steps: [
    fileCounts,
    files("by default"),
    file(1),
    file(2),
    file(999),
    folder(""),
    folder("music"),
    folder("music/Ryo Fukui"),
    folder("./music//Ryo Fukui/CD1/"),
    folder(".."),
    folder("music/Elsewhere"),
    folder("nowhere"),
    folder("music/Ryo Fukui/rip.log"),
    folderFiles("music"),
    folderFiles(""),
    folderFiles("nowhere"),

    register("normalized, each once", () => [
      "music/Ryo Fukui/10 Mellow Dream.flac",
      "./music//Ryo Fukui/CD1/1 Scenery.mp3",
      "music/Ryo Fukui/10 Mellow Dream.flac",
      "music/Ryo Fukui/2 Early Summer.flac",
    ]),
    register("above the library and through a symlink", () => [
      "../shared/a.m4a",
      "music/Elsewhere/sub/b.png",
    ]),
    register("by absolute path", (library) => [
      `${library}/music/Alika/01 virmalised.m4a`,
    ]),
    register("with odd names", () => [
      "music/Alika/Café.flac",
      "music/Alika/50%_off.txt",
    ]),
    register("nothing", () => []),
    register("a folder", () => ["music/Alika/50%_off.txt", "."]),
    register("a folder's path", () => ["music/Alika"]),
    register("a missing file", () => ["music/gone.flac"]),

    fileCounts,
    files("by default"),
    files("by path", { sort: "path" }),
    files("by path, descending, searched", { sort: "-path", q: "EARLY" }),
    files("searched for a LIKE pattern", { q: "50%_" }),
    files("searched in either form", { q: "Café" }),
    files("of a type", { type: "image" }),
    files("unused music", { type: "music", unused: true }),
    files("without a track", { withoutTrack: true, sort: "path" }),
    files("paged", { page: 2, perPage: 3, sort: "path" }),
    files("past the end", { page: 9 }),
    files("by size", { sort: "size" }),
    files("on page 0", { page: 0 }),
    file(4),
    folder("music/Ryo Fukui"),
    folder(".."),
    folderFiles("music"),

    update("to another type", 4, { type: "other" }),
    update("from a track's audio to an image", 1, { type: "image" }),
    update("from a picture to other", 2, { type: "other" }),
    update("to a file that moved", 1, {
      path: "music/moved/2 Early Summer.flac",
      type: "music",
    }),
    update("to a path that's taken", 1, {
      path: "./music/Ryo Fukui/CD1/1 Scenery.mp3",
    }),
    update("to a missing file", 1, { path: "music/nothing.flac" }),
    update("at the same path", 4, {
      path: "music/Ryo Fukui/../Ryo Fukui/10 Mellow Dream.flac",
      type: "music",
    }),
    update("that doesn't exist", 999, { type: "music" }),
    file(1),

    remove(4),
    remove(2),
    remove(3),
    remove(999),
    file(4),
    fileCounts,
    files("by path", { sort: "path" }),
  ],
}
