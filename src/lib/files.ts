import { toast } from "sonner"

import { call } from "@/lib/query"

/** Opens a registered file in its default app (frontend.md, "Files"). */
export async function openFile(id: number | string) {
  try {
    await call("openFile", { id: Number(id) })
  } catch (error) {
    toast.error("Can't open the file", {
      description: (error as Error).message,
    })
  }
}

/** Shows a registered file in Finder. */
export async function revealFile(id: number | string) {
  try {
    await call("revealFile", { id: Number(id) })
  } catch (error) {
    toast.error("Can't show the file", {
      description: (error as Error).message,
    })
  }
}
