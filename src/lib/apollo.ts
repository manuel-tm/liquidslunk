import {
  ApolloClient,
  ApolloLink,
  HttpLink,
  InMemoryCache,
} from "@apollo/client"
import { OperationTypeNode } from "graphql"
import { tap } from "rxjs"

import { afterCommandMutation, queryClient } from "@/lib/query"

/**
 * Lists the API always returns whole. A new one replaces the cached one,
 * which Apollo does anyway, but would warn about when the list got shorter.
 */
function replaced(...fields: string[]) {
  return {
    fields: Object.fromEntries(
      fields.map((field) => [field, { merge: false }])
    ),
  }
}

/**
 * A GraphQL mutation can change what a command reads, so TanStack Query
 * invalidates everything after one (design §8).
 */
const invalidateCommands = new ApolloLink((operation, forward) =>
  operation.operationType === OperationTypeNode.MUTATION
    ? forward(operation).pipe(tap(() => void queryClient.invalidateQueries()))
    : forward(operation)
)

/**
 * slunk's GraphQL API, for the routes not ported yet. In development, Vite
 * proxies /graphql to it, and the app runs it as its sidecar (design §8).
 */
export const apollo = new ApolloClient({
  link: ApolloLink.from([
    invalidateCommands,
    new HttpLink({ uri: "/graphql" }),
  ]),
  cache: new InMemoryCache({
    typePolicies: {
      Release: replaced(
        "alternateTitles",
        "artists",
        "labels",
        "formats",
        "media",
        "tracks",
        "discTitles",
        "credits",
        "files",
        "tags"
      ),
      Track: replaced(
        "alternateTitles",
        "artists",
        "features",
        "performers",
        "credits",
        "files"
      ),
      Artist: replaced(
        "alternateNames",
        "files",
        "appearances",
        "credits",
        "formatCounts"
      ),
      Label: replaced("alternateNames"),
    },
  }),
  // A page shows what's cached at once, and asks the backend again, so it
  // never shows a list from before a change
  defaultOptions: { watchQuery: { fetchPolicy: "cache-and-network" } },
})

// A command's mutation can change what a query shows, so Apollo fetches its
// active queries again (design §8)
afterCommandMutation(() => void apollo.refetchQueries({ include: "active" }))
