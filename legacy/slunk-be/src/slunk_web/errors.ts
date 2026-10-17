import type { GraphQLFormattedError } from "graphql"

/**
 * SQLite's constraint errors, in words a form can show. The triggers' own
 * messages are already readable ("a track's file must be music"), so they
 * pass through, as does anything else.
 */
const MESSAGES: [RegExp, string][] = [
  [
    /UNIQUE constraint failed: tracks\.file_id/,
    "That file is already a track's audio",
  ],
  [
    /UNIQUE constraint failed: tracks\.release_id, tracks\.cd_number, tracks\.track_number/,
    "That disc already has a track with that number",
  ],
  [
    /UNIQUE constraint failed: files\.path/,
    "A file is already registered at that path",
  ],
  [
    /UNIQUE constraint failed: (release_formats|release_media|\w+_tags)\.name/,
    "There's already one with that name, ignoring case",
  ],
  [
    /UNIQUE constraint failed: \w+_credits\.\w+_id, \w+_credits\.role/,
    "Two credits have the same role, ignoring case",
  ],
  [
    /UNIQUE constraint failed: (artist|label)_alternate_names\.\w+, \w+\.name/,
    "It already has that alternate name",
  ],
  [
    /UNIQUE constraint failed: \w+\.\w+_id, \w+\.\w+_id/,
    "Each one can only be listed once",
  ],
  [
    /CHECK constraint failed: \("(name_)?language"[\s\S]*/,
    "That isn't a language tag, like en, ja or ja-Latn",
  ],
  [/CHECK constraint failed: (name|title|role)$/, "The $1 can't be empty"],
  [
    /CHECK constraint failed: (track_number|cd_number) > 0/,
    "Disc and track numbers start at 1",
  ],
  [
    /FOREIGN KEY constraint failed/,
    "It refers to something that doesn't exist, or something still uses it",
  ],
]

export function formatError(
  error: GraphQLFormattedError
): GraphQLFormattedError {
  for (const [pattern, message] of MESSAGES) {
    if (pattern.test(error.message)) {
      return { ...error, message: error.message.replace(pattern, message) }
    }
  }
  return error
}
