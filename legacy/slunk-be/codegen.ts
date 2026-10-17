import type { CodegenConfig } from "@graphql-codegen/cli"

const CATALOG = "../../slunk/catalog/repository.ts"
const MEDIA = "../../slunk/media/repository.ts"
const TAGS = "../../slunk/tagging/repository.ts"

// Resolver types from the SDL: `npm run codegen` after changing a .graphql file
const config: CodegenConfig = {
  schema: "src/slunk_web/schema/*.graphql",
  generates: {
    "src/slunk_web/schema/resolvers_types.ts": {
      plugins: ["typescript", "typescript-resolvers"],
      config: {
        contextType: "./context.ts#Context",
        useTypeImports: true,
        enumsAsTypes: true,
        // Resolvers can return the database's numeric ids
        scalars: { ID: { input: "string", output: "string | number" } },
        // GraphQL fills in arguments and input fields that have defaults
        avoidOptionals: { defaultValue: true },
        // What resolvers return for these types, before their field
        // resolvers. Imported as `…Model`, so names don't clash.
        mapperTypeSuffix: "Model",
        mappers: {
          File: `${MEDIA}#FileRecord`,
          FileLink: `${MEDIA}#FileLink`,
          Artist: `${CATALOG}#Artist`,
          Label: `${CATALOG}#Label`,
          ArtistName: `${CATALOG}#AlternateName`,
          LabelName: `${CATALOG}#AlternateName`,
          Release: `${CATALOG}#Release`,
          Track: `${CATALOG}#Track`,
          Credit: `${CATALOG}#Credit`,
          CreditedArtist: `${CATALOG}#ArtistLink`,
          ListItem: `${CATALOG}#ListItem`,
          ReleaseFormat: `${CATALOG}#ListItem`,
          ReleaseMedium: `${CATALOG}#ListItem`,
          Appearance: `${CATALOG}#Appearance`,
          ArtistCredit: `${CATALOG}#ArtistCredit`,
          FormatCount: `${CATALOG}#FormatCount`,
          Tag: `${TAGS}#Tag`,
          TextTag: `${TAGS}#Tag`,
          NumberTag: `${TAGS}#Tag`,
          DateTag: `${TAGS}#Tag`,
          TagValues: "../../slunk/tagging.ts#OwnerTag",
        },
      },
    },
  },
}

export default config
