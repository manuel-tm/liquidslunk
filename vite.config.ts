/// <reference types="vitest/config" />
import { resolve } from "node:path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

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
    watch: {
      ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**"],
    },
  },
  // `npm test`: the frontend's plain functions, in Node, with no DOM
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
  },
})
