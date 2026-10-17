import { useSearchParams } from "react-router"

type Change = string | number | boolean | null

/**
 * A list's state, kept in the query string so it survives a refresh, the back
 * button and a shared link (slunk's design §7.2): `page`, `sort`, `q` and filters.
 */
export function useListParams() {
  const [params, setParams] = useSearchParams()

  /**
   * Sets parameters. Null, "", false and page 1 remove one. Any change but
   * the page goes back to page 1. `replace` keeps typing out of the history.
   */
  function update(changes: Record<string, Change>, { replace = false } = {}) {
    setParams(
      (current) => {
        const next = new URLSearchParams(current)
        if (!("page" in changes)) next.delete("page")
        for (const [key, value] of Object.entries(changes)) {
          const empty = value === null || value === "" || value === false
          if (empty || (key === "page" && value === 1)) next.delete(key)
          else next.set(key, value === true ? "1" : String(value))
        }
        return next
      },
      { replace }
    )
  }

  /**
   * Adds parameters, keeping those with the same name, for filters that can
   * repeat (`text.3=Pop&text.3=Rock`). Goes back to page 1.
   */
  function append(entries: [string, string][]) {
    setParams((current) => {
      const next = new URLSearchParams(current)
      next.delete("page")
      for (const [key, value] of entries) next.append(key, value)
      return next
    })
  }

  /** Removes one value of a parameter that can repeat. Goes back to page 1. */
  function remove(key: string, value: string) {
    setParams((current) => {
      const next = new URLSearchParams(current)
      next.delete("page")
      next.delete(key, value)
      return next
    })
  }

  return {
    params,
    page: Math.max(1, Number(params.get("page")) || 1),
    sort: params.get("sort"),
    q: params.get("q") ?? "",
    update,
    append,
    remove,
  }
}
