import type { Server } from "node:http"
import { ApolloServer } from "@apollo/server"
import { expressMiddleware } from "@as-integrations/express5"
import express from "express"
import { formatError } from "./errors.ts"
import { filesController } from "./files_controller.ts"
import type { App, Context } from "./schema/context.ts"
import { resolvers, typeDefs } from "./schema/index.ts"
import { createLoaders } from "./schema/loaders.ts"

const LOCAL_HOSTS = new Set(["localhost", "127.0.0.1", "[::1]"])

/**
 * Starts Express: Apollo at /graphql, and the files' bytes at /raw/files/:id,
 * outside the frontend's /files pages (design Appendix C). It
 * listens on 127.0.0.1 only: Slunk is local, and nothing else on the network
 * should reach it.
 */
export async function startEndpoint(app: App, port: number): Promise<Server> {
  const apollo = new ApolloServer<Context>({ typeDefs, resolvers, formatError })
  await apollo.start()

  const server = express()
  // A web page can point a domain it controls at 127.0.0.1 (DNS rebinding).
  // Its requests then carry that domain as their Host, so they're refused.
  server.use((req, res, next) => {
    if (LOCAL_HOSTS.has(req.hostname)) next()
    else res.sendStatus(403)
  })
  server.use(
    "/graphql",
    express.json(),
    expressMiddleware(apollo, {
      context: async () => ({ ...app, loaders: createLoaders(app) }),
    })
  )
  server.get("/raw/files/:id", filesController(app.media))

  return new Promise((resolve, reject) => {
    const listener = server.listen(port, "127.0.0.1", (error) =>
      error ? reject(error) : resolve(listener)
    )
  })
}
