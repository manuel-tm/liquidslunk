// Ticks about once a millisecond, from when it starts until it's terminated,
// for the handoff's timing (3_gapless.md §3). While the page plays audio in a
// hidden window, WebKit runs its timers about once a second, but a new
// worker's every few milliseconds. The DOM's types stand in for the worker's,
// which has the same `postMessage`.

function tick() {
  self.postMessage(null)
  setTimeout(tick, 1)
}

tick()
