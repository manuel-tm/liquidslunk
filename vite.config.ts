import { resolve } from "node:path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

// slunk's backend serves the routes not ported yet. In debug builds, the app
// starts it on port 4000 when a library opens (design §8).
const proxy = {
  "/graphql": "http://127.0.0.1:4000",
}

// https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "./src"),
    },
  },
  // Keeps Rust's errors on screen
  clearScreen: false,
  server: {
    // Tauri expects this port (devUrl in src-tauri/tauri.conf.json)
    port: 5173,
    strictPort: true,
    proxy,
    watch: {
      ignored: [
        "**/src-tauri/**",
        "**/crates/**",
        "**/legacy/**",
        "**/target/**",
      ],
    },
  },
})
