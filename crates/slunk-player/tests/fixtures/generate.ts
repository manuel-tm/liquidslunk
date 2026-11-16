// Generates the engine's fixtures (backend.md, "The player engine"): tracks
// cut from one continuous signal, so a join between them can be checked
// sample for sample, in the formats a test can't write itself.
//
// Run from the repository root, on macOS:
//
//   node crates/slunk-player/tests/fixtures/generate.ts
//
// The signal is integer arithmetic, a triangle wave on each channel, so the
// tests work out the same samples, and check what they decode against them
// (tests/engine.rs): FLAC and Apple Lossless must decode to them exactly.
// AAC, which is lossy, is checked for its length, which its encoder's delay
// and padding would change. afconvert makes the audio, except the MP3's: this
// Mac has no MP3 encoder, so its frames are silence written here, with a LAME
// tag that says the encoder's delay and padding. The M4As hold afconvert's
// time of creation, so they change each time this runs; the rest don't.
import { execFileSync } from "node:child_process"
import { mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

const HERE = import.meta.dirname
const RATE = 44100
// Each track's frames. FLAC's frames are 4,608 here, so each ends with a
// short one, which WebKit padded to a whole one.
const TRACKS = [5003, 6421, 4900]
// The MP3's audio frames, and the delay and padding its LAME tag says
const MP3_FRAMES = 20
const MP3_DELAY = 576
const MP3_PADDING = 1000

const temp = mkdtempSync(join(tmpdir(), "slunk-player-"))
try {
  let first = 0
  for (const [index, frames] of TRACKS.entries()) {
    const source = join(temp, `join-${index + 1}.wav`)
    writeFileSync(source, wav(signal(first, frames)))
    first += frames
    const name = `join-${index + 1}`
    afconvert(source, "flac", "flac", join(HERE, `${name}.flac`))
    afconvert(source, "m4af", "alac", join(HERE, `${name}.m4a`))
    if (index === 0) {
      afconvert(source, "m4af", "aac", join(HERE, "aac.m4a"))
    }
  }
  writeFileSync(join(HERE, "lame.mp3"), mp3())
} finally {
  rmSync(temp, { recursive: true, force: true })
}

/**
 * The signal from frame `first`, `frames` long, as interleaved 16-bit
 * stereo: triangle waves of periods 211 and 173 frames, at 12,000.
 * tests/engine.rs works out the same.
 */
function signal(first: number, frames: number): Buffer {
  const pcm = Buffer.alloc(frames * 4)
  for (let i = 0; i < frames; i++) {
    pcm.writeInt16LE(triangle(first + i, 211), i * 4)
    pcm.writeInt16LE(triangle(first + i, 173), i * 4 + 2)
  }
  return pcm
}

function triangle(frame: number, period: number): number {
  const amplitude = 12000
  const phase = frame % period
  const half = Math.floor(period / 2)
  return phase < half
    ? Math.trunc((4 * amplitude * phase) / period) - amplitude
    : 3 * amplitude - Math.trunc((4 * amplitude * phase) / period)
}

function wav(pcm: Buffer): Buffer {
  const format = Buffer.alloc(16)
  format.writeUInt16LE(1, 0) // PCM
  format.writeUInt16LE(2, 2) // Stereo
  format.writeUInt32LE(RATE, 4)
  format.writeUInt32LE(RATE * 4, 8)
  format.writeUInt16LE(4, 12)
  format.writeUInt16LE(16, 14)
  return chunk(
    "RIFF",
    Buffer.concat([
      Buffer.from("WAVE"),
      chunk("fmt ", format),
      chunk("data", pcm),
    ])
  )
}

function chunk(id: string, body: Buffer): Buffer {
  const header = Buffer.alloc(8)
  header.write(id, 0, "latin1")
  header.writeUInt32LE(body.length, 4)
  return Buffer.concat([header, body])
}

function afconvert(
  source: string,
  format: string,
  data: string,
  target: string
) {
  execFileSync("afconvert", ["-f", format, "-d", data, source, target])
}

/**
 * Silent MPEG-1 Layer III frames, mono at 32 kbps and 44.1 kHz, after an
 * Info frame with a LAME tag: its encoder string, then at 0x15 of the tag,
 * 12 bits of delay and 12 of padding. Its CRC is 0, which readers take as
 * none.
 */
function mp3(): Buffer {
  const frame = () => {
    const bytes = Buffer.alloc(104) // 144 * 32000 / 44100, without padding
    bytes.set([0xff, 0xfb, 0x10, 0xc4])
    return bytes
  }
  const info = frame()
  // After the header and mono's 17 bytes of side information
  let at = 4 + 17
  info.write("Info", at, "latin1")
  info.writeUInt32BE(0x3, at + 4) // Frames and bytes
  info.writeUInt32BE(MP3_FRAMES, at + 8)
  info.writeUInt32BE((MP3_FRAMES + 1) * info.length, at + 12)
  at += 16
  info.write("LAME3.100", at, "latin1")
  const trim = (MP3_DELAY << 12) | MP3_PADDING
  info.writeUIntBE(trim, at + 0x15, 3)
  return Buffer.concat([info, ...Array.from({ length: MP3_FRAMES }, frame)])
}
