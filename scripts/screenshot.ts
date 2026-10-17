// Takes a screenshot of the running app, through the WebDriver server that
// builds with the `webdriver` feature embed (tauri-plugin-wdio-webdriver,
// on 127.0.0.1:4445). Start the app with
//
//   npm run tauri dev -- --features webdriver
//
// then, from the repository's root:
//
//   npm run screenshot -- [route] [file.png]
//
// The route defaults to the current page, and the file to screenshot.png. It
// waits for the page to settle before taking the picture. Set `THEME=dark` or
// `THEME=light` to take it in that theme.
import { writeFileSync } from "node:fs"

const DRIVER = `http://127.0.0.1:${process.env.TAURI_WEBDRIVER_PORT ?? 4445}`
const [route, file = "screenshot.png"] = process.argv.slice(2)

async function send(method: string, path: string, body?: unknown) {
  const response = await fetch(`${DRIVER}${path}`, {
    method,
    headers: { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  const json = (await response.json()) as { value: unknown }
  if (!response.ok) {
    throw new Error(`${method} ${path}: ${JSON.stringify(json.value)}`)
  }
  return json.value
}

const { sessionId } = (await send("POST", "/session", {
  capabilities: {},
})) as { sessionId: string }
const session = `/session/${sessionId}`
const run = (script: string, ...args: unknown[]) =>
  send("POST", `${session}/execute/async`, { script, args })

try {
  if (route) {
    // A client-side navigation, so the app keeps its state. The entry is
    // shaped as React Router's, with its index, so back and forward work.
    await run(
      `const [route, done] = arguments
       const idx = (window.history.state?.idx ?? 0) + 1
       const key = Math.random().toString(36).slice(2, 10)
       window.history.pushState({ usr: null, key, idx }, "", route)
       window.dispatchEvent(new PopStateEvent("popstate"))
       done()`,
      route
    )
  }
  if (process.env.THEME) {
    await run(
      `const [theme, done] = arguments
       document.documentElement.classList.toggle("dark", theme === "dark")
       done()`,
      process.env.THEME
    )
  }
  // Waits for requests to settle and images to load
  await run(
    `const done = arguments[0]
     setTimeout(done, Number(${process.env.WAIT ?? 1500}))`
  )
  const png = (await send("GET", `${session}/screenshot`)) as string
  writeFileSync(file, Buffer.from(png, "base64"))
  console.log(`Wrote ${file}`)
} finally {
  await send("DELETE", session).catch(() => {})
}
