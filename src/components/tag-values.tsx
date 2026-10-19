import { Link } from "react-router"

import type { TagType } from "@/bindings"
import { formatTagValue, tagValueUrl } from "@/lib/tags"

/**
 * A tag's values, "Pop, Rock", each linking to the list filtered by it.
 */
export function TagValues({
  type,
  tagId,
  values,
  list = "/releases",
}: {
  type: TagType
  tagId: number | string
  values: string[]
  list?: "/releases" | "/tracks"
}) {
  return values.map((value, index) => (
    <span key={value}>
      {index > 0 && ", "}
      <Link
        to={tagValueUrl(type, tagId, value, list)}
        className="hover:underline"
      >
        {formatTagValue(type, value)}
      </Link>
    </span>
  ))
}
