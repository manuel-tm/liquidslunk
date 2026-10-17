import { useMutation } from "@apollo/client/react"
import { CheckIcon, LanguagesIcon } from "lucide-react"
import { useState } from "react"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { graphql } from "@/gql"
import type { OfferedName } from "@/lib/import"
import { languageName, scriptLanguage, textProps } from "@/lib/languages"

const AddArtistName = graphql(`
  mutation AddArtistName($id: ID!, $name: String!, $language: String) {
    addArtistName(id: $id, name: $name, language: $language) {
      id
      alternateNames {
        id
        name
        language
        realName
        note
        uses
      }
    }
  }
`)

const keyOf = (offer: OfferedName) => `${offer.artist.id}:${offer.name}`

/**
 * The other parts of artist tags like "Ryo Fukui = 福居良", offered as
 * alternate names of the artists they matched (slunk's 3_alternate_names.md §7.7).
 * Each is added on its own, with the language its script gives away.
 */
export function OtherNamesOffer({ offers }: { offers: OfferedName[] }) {
  const [added, setAdded] = useState<Set<string>>(new Set())
  const [add, { loading, error }] = useMutation(AddArtistName)
  if (offers.length === 0) return null
  return (
    <Alert className="max-w-2xl">
      <LanguagesIcon />
      <AlertTitle>Other names in the tags</AlertTitle>
      <AlertDescription className="flex flex-col gap-1">
        {offers.map((offer) => {
          const language = scriptLanguage(offer.name)
          return (
            <div key={keyOf(offer)} className="flex items-center gap-3">
              <span className="flex-1">
                "<span {...textProps(language)}>{offer.name}</span>"
                {language && ` (${languageName(language)})`}, for{" "}
                {offer.artist.name}
              </span>
              {added.has(keyOf(offer)) ? (
                <span className="flex items-center gap-1">
                  <CheckIcon className="size-3" />
                  Added
                </span>
              ) : (
                <Button
                  type="button"
                  size="xs"
                  variant="outline"
                  disabled={loading}
                  onClick={async () => {
                    try {
                      await add({
                        variables: {
                          id: offer.artist.id,
                          name: offer.name,
                          language,
                        },
                      })
                      setAdded((current) => new Set(current).add(keyOf(offer)))
                    } catch {
                      // Shown below
                    }
                  }}
                >
                  Add to {offer.artist.name}'s names
                </Button>
              )}
            </div>
          )
        })}
        {error && <p className="text-destructive">{error.message}</p>}
      </AlertDescription>
    </Alert>
  )
}
