import type { Request, Response } from "express"
import type { Media } from "../slunk/media.ts"

/**
 * GET /raw/files/:id serves a registered file's bytes, read-only, for viewing and
 * playback. Range requests work, so audio can seek.
 */
export function filesController(media: Media) {
  return async (req: Request<{ id: string }>, res: Response) => {
    const id = Number(req.params.id)
    const file = Number.isSafeInteger(id) ? await media.getFile(id) : null
    if (!file) {
      res.sendStatus(404)
      return
    }
    res.sendFile(media.absolutePath(file), { dotfiles: "allow" }, (error) => {
      if (error && !res.headersSent) res.sendStatus(404)
    })
  }
}
