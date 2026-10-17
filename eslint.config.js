import js from '@eslint/js'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import tseslint from 'typescript-eslint'
import { defineConfig, globalIgnores } from 'eslint/config'

export default defineConfig([
  // Generated files, builds, and slunk as of its last commit, read-only
  globalIgnores([
    'dist',
    'target',
    'legacy',
    'src-tauri',
    'src/gql',
    'src/bindings.ts',
  ]),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      globals: globals.browser,
    },
    rules: {
      'react-refresh/only-export-components': 'off',
    },
  },
  {
    // Scripts that Node runs: screenshots, the parity fixtures' generator
    files: ['scripts/**/*.ts', 'crates/**/*.ts'],
    languageOptions: {
      globals: globals.node,
    },
  },
])
