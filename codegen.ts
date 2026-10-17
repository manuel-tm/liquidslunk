import type { CodegenConfig } from "@graphql-codegen/cli"

// Typed GraphQL documents in src/gql/, from slunk's backend's SDL and the
// graphql() calls in src/. Run `npm run codegen` after changing either.
const config: CodegenConfig = {
  schema: "legacy/slunk-be/src/slunk_web/schema/*.graphql",
  documents: ["src/**/*.{ts,tsx}", "!src/gql/**"],
  generates: {
    "src/gql/": {
      preset: "client",
      presetConfig: { fragmentMasking: false },
      config: { useTypeImports: true, enumsAsTypes: true },
    },
  },
}

export default config
