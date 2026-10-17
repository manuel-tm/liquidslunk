import { SearchIcon } from "lucide-react"
import { useEffect, useEffectEvent, useState } from "react"

import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "@/components/ui/input-group"

/** A search box for `?q=`. It waits for a pause in typing before searching. */
export function SearchBox({
  value,
  placeholder,
  onSearch,
}: {
  value: string
  placeholder: string
  onSearch: (q: string) => void
}) {
  const [text, setText] = useState(value)

  // Follows `?q=` when it changes from outside, e.g. with the back button
  const [shown, setShown] = useState(value)
  if (value !== shown) {
    setShown(value)
    setText(value)
  }

  const search = useEffectEvent((q: string) => onSearch(q))
  useEffect(() => {
    if (text === value) return
    const timer = setTimeout(() => search(text), 300)
    return () => clearTimeout(timer)
  }, [text, value])

  return (
    <InputGroup className="w-64">
      <InputGroupAddon>
        <SearchIcon />
      </InputGroupAddon>
      <InputGroupInput
        type="search"
        value={text}
        placeholder={placeholder}
        onChange={(event) => setText(event.target.value)}
      />
    </InputGroup>
  )
}
