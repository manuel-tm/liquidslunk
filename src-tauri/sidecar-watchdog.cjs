// Preloaded into slunk's backend when it runs as the app's sidecar, with
// `node --require` (src/sidecar.rs). When the app that started it is gone
// without stopping it, as when `tauri dev` restarts the app after a change,
// the backend is orphaned: this stops it. SIGTERM lets it close its database,
// which removes the -wal and -shm files.
const parent = process.ppid

setInterval(() => {
  if (process.ppid !== parent) process.kill(process.pid, "SIGTERM")
}, 500).unref()
