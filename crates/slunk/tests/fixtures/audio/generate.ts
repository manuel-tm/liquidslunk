// Generates the audio fixtures: a second or so of silence in each format this
// Mac makes without ffmpeg, with tags set (backend.md, "Audio").
//
// expected.json is what slunk's reader, with music-metadata, read from each
// file, which tests/audio.rs checks lofty's reading against. It was written by
// this script while slunk's backend was in the repository, and isn't written
// again: the files this makes hold the same audio and tags each time. A new
// fixture needs its expected durations and tags added by hand.
//
// Run from the repository root, on macOS:
//
//   node crates/slunk/tests/fixtures/audio/generate.ts
//
// The audio comes from afconvert, macOS's converter, except the MP3's, whose
// silent frames are written here. The M4As hold afconvert's time of creation,
// so they change each time this runs; the rest don't. The tags are written here
// too, byte by byte, so they don't come from lofty, which the test checks:
// Vorbis comments, ID3v2.3, ID3v2.4, ID3v1, MP4 atoms, RIFF INFO and AIFF's
// text chunks.
import { execFileSync } from "node:child_process"
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

const HERE = import.meta.dirname

// 8 kHz mono keeps the files small. 8005 samples is 1000.625 ms, which
// rounds up, where a duration cut to the millisecond wouldn't.
const RATE = 8000
const ODD = 8005

const temp = mkdtempSync(join(tmpdir(), "liquidslunk-audio-"))
try {
  const fixtures: Record<string, Buffer> = {
    "flac.flac": flac(convert(ODD, "flac", "flac", "flac"), [
      ["TITLE", "Early Summer"],
      ["ARTIST", "Ryo Fukui"],
      ["ARTIST", "Satoshi Denpo"],
      ["ALBUM", "Scenery"],
      ["ALBUMARTIST", "Ryo Fukui Trio"],
      ["TRACKNUMBER", "3"],
      ["TRACKTOTAL", "6"],
      ["DISCNUMBER", "1"],
      ["DISCSUBTITLE", "Live at Slowboat"],
      ["DATE", "1976-03-01"],
    ]),
    "flac-variants.flac": flac(convert(RATE, "flac", "flac", "flac"), [
      ["title", "Lowercase Keys"],
      ["ARTIST", "Alika & Nublu"],
      ["ARTISTS", "Alika"],
      ["ARTISTS", "Nublu"],
      ["ALBUM ARTIST", "Alika"],
      ["TRACKNUMBER", "03/12"],
      ["DISCNUMBER", "2/2"],
      ["DATE", "2006"],
    ]),
    "untagged.flac": convert(RATE, "flac", "flac", "flac"),
    "aac.m4a": mp4(convert(RATE, "m4af", "aac", "m4a"), [
      text("©nam", "Early Summer"),
      text("©ART", "Ryo Fukui"),
      text("©alb", "Scenery"),
      text("aART", "Ryo Fukui Trio"),
      pair("trkn", 3, 6),
      pair("disk", 1, 2),
      text("©day", "1976-03-01T08:00:00Z"),
      freeform("DISCSUBTITLE", ["Live at Slowboat"]),
    ]),
    "alac.m4a": mp4(convert(RATE, "m4af", "alac", "m4a"), [
      text("©nam", "virmalised"),
      text("©ART", "Alika & Nublu"),
      freeform("ARTISTS", ["Alika", "Nublu"]),
      pair("trkn", 12, 0),
      text("©day", "2006"),
    ]),
    "id3v24.mp3": Buffer.concat([
      id3v2(4, [
        ["TIT2", "Early Summer"],
        ["TPE1", "Ryo Fukui\0Satoshi Denpo"],
        ["TALB", "Scenery"],
        ["TPE2", "Ryo Fukui Trio"],
        ["TRCK", "3/6"],
        ["TPOS", "1/2"],
        ["TSST", "Live at Slowboat"],
        ["TDRC", "1976-03-01"],
      ]),
      mpeg(39),
    ]),
    "id3v23.mp3": Buffer.concat([
      id3v2(3, [
        ["TIT2", "Highway to Hell"],
        // ID3v2.3's artists are split at "/", album artists aren't
        ["TPE1", "AC/DC"],
        ["TPE2", "AC/DC"],
        ["TALB", "Highway to Hell"],
        ["TRCK", "1"],
        ["TPOS", "1"],
        ["TSST", "Side A"],
        // The year alone: music-metadata didn't add TDAT's day to it
        ["TYER", "1979"],
        ["TDAT", "2707"],
      ]),
      mpeg(39),
      // ID3v1's title loses to ID3v2's, but its track number wins, as the
      // last one read
      id3v1({ title: "ID3v1 Title", artist: "AC/DC", year: "1979", track: 7 }),
    ]),
    // Without an Info frame, as older encoders wrote them: music-metadata
    // estimated a constant bitrate's duration from the file's size, without
    // the ID3v1 tag, and counted a variable bitrate's frames
    "cbr.mp3": Buffer.concat([
      id3v2(4, [["TIT2", "Constant"]]),
      mpeg(39, { info: false }),
      id3v1({ title: "Constant", artist: "", year: "", track: 0 }),
    ]),
    "vbr.mp3": Buffer.concat([
      id3v2(4, [["TIT2", "Variable"]]),
      mpeg(40, { info: false, variable: true }),
    ]),
    // At 44.1 kHz: lofty doesn't read ADTS at 8 kHz
    "adts.aac": Buffer.concat([
      id3v2(4, [["TIT2", "Raw AAC"]]),
      convert(44100, "adts", "aac", "aac", 44100),
    ]),
    "wav.wav": wav(
      silence(RATE),
      [
        ["INAM", "Early Summer"],
        ["IART", "Ryo Fukui"],
        ["IPRD", "Scenery"],
        ["ICRD", "1976"],
        ["ITRK", "3"],
      ],
      id3v2(4, [
        // ID3v2 outranks RIFF INFO, and its track number is read last
        ["TIT2", "From ID3v2"],
        ["TPE2", "Ryo Fukui Trio"],
        ["TSST", "Live at Slowboat"],
        ["TRCK", "4"],
      ])
    ),
    "aiff.aiff": aiff(
      convert(ODD, "AIFF", "BEI16", "aiff"),
      [
        ["NAME", "Early Summer"],
        ["AUTH", "Ryo Fukui"],
      ],
      id3v2(4, [
        ["TALB", "Scenery"],
        ["TPE2", "Ryo Fukui Trio"],
        ["TRCK", "3/6"],
        ["TPOS", "1/2"],
        ["TSST", "Live at Slowboat"],
        ["TDRC", "1976-03"],
      ])
    ),
  }

  for (const [name, bytes] of Object.entries(fixtures)) {
    writeFileSync(join(HERE, name), bytes)
  }
} finally {
  rmSync(temp, { recursive: true, force: true })
}

/** Silent 16-bit mono PCM. */
function silence(samples: number): Buffer {
  return Buffer.alloc(samples * 2)
}

/** A WAV file, with RIFF INFO and an ID3v2 chunk when given. */
function wav(
  pcm: Buffer,
  info: [string, string][] = [],
  id3?: Buffer,
  rate = RATE
): Buffer {
  const format = Buffer.alloc(16)
  format.writeUInt16LE(1, 0) // PCM
  format.writeUInt16LE(1, 2) // Mono
  format.writeUInt32LE(rate, 4)
  format.writeUInt32LE(rate * 2, 8)
  format.writeUInt16LE(2, 12)
  format.writeUInt16LE(16, 14)
  const chunks = [riffChunk("fmt ", format), riffChunk("data", pcm)]
  if (info.length > 0) {
    const items = info.map(([id, value]) =>
      riffChunk(id, Buffer.from(`${value}\0`, "latin1"))
    )
    chunks.push(
      riffChunk("LIST", Buffer.concat([Buffer.from("INFO"), ...items]))
    )
  }
  if (id3) chunks.push(riffChunk("id3 ", id3))
  return riffChunk("RIFF", Buffer.concat([Buffer.from("WAVE"), ...chunks]))
}

function riffChunk(id: string, body: Buffer): Buffer {
  const header = Buffer.alloc(8)
  header.write(id, 0, "latin1")
  header.writeUInt32LE(body.length, 4)
  return Buffer.concat([header, body, pad(body)])
}

/** A file in another format, converted from silence by afconvert. */
function convert(
  samples: number,
  format: string,
  data: string,
  extension: string,
  rate = RATE
): Buffer {
  const source = join(temp, `source-${samples}-${rate}.wav`)
  writeFileSync(source, wav(silence(samples), [], undefined, rate))
  const target = join(temp, `converted.${extension}`)
  execFileSync("afconvert", ["-f", format, "-d", data, source, target])
  return readFileSync(target)
}

/** A FLAC file with its Vorbis comments replaced. */
function flac(file: Buffer, comments: [string, string][]): Buffer {
  if (file.toString("latin1", 0, 4) !== "fLaC") throw new Error("Not FLAC")
  const blocks: { type: number; body: Buffer }[] = []
  let offset = 4
  let last = false
  while (!last) {
    last = (file[offset] & 0x80) !== 0
    const type = file[offset] & 0x7f
    const length = file.readUIntBE(offset + 1, 3)
    blocks.push({ type, body: file.subarray(offset + 4, offset + 4 + length) })
    offset += 4 + length
  }
  const vendor = Buffer.from("liquidslunk fixtures")
  const parts = [u32le(vendor.length), vendor, u32le(comments.length)]
  for (const [key, value] of comments) {
    const comment = Buffer.from(`${key}=${value}`)
    parts.push(u32le(comment.length), comment)
  }
  const kept = blocks.filter((block) => block.type !== 4)
  // After STREAMINFO, which comes first
  kept.splice(1, 0, { type: 4, body: Buffer.concat(parts) })
  const out: Buffer[] = [Buffer.from("fLaC")]
  kept.forEach((block, i) => {
    const header = Buffer.alloc(4)
    header[0] = (i === kept.length - 1 ? 0x80 : 0) | block.type
    header.writeUIntBE(block.body.length, 1, 3)
    out.push(header, block.body)
  })
  return Buffer.concat([...out, file.subarray(offset)])
}

/**
 * An ID3v2.3 or ID3v2.4 tag of text frames. A TXXX frame is "TXXX:name".
 * ID3v2.4 is UTF-8, and ID3v2.3 UTF-16 with a BOM, as iTunes writes it.
 */
function id3v2(version: 3 | 4, frames: [string, string][]): Buffer {
  const encode = (text: string) =>
    version === 4
      ? Buffer.from(text, "utf8")
      : Buffer.concat([Buffer.from([0xff, 0xfe]), Buffer.from(text, "utf16le")])
  const terminator = Buffer.alloc(version === 4 ? 1 : 2)
  const bodies = frames.map(([id, value]) => {
    const [frameId, description] = id.split(":")
    const encoding = Buffer.from([version === 4 ? 3 : 1])
    const body =
      frameId === "TXXX"
        ? Buffer.concat([
            encoding,
            encode(description),
            terminator,
            encode(value),
          ])
        : Buffer.concat([encoding, encode(value)])
    const header = Buffer.alloc(10)
    header.write(frameId, 0, "latin1")
    if (version === 4) header.writeUInt32BE(syncsafe(body.length), 4)
    else header.writeUInt32BE(body.length, 4)
    return Buffer.concat([header, body])
  })
  const content = Buffer.concat(bodies)
  const header = Buffer.from([0x49, 0x44, 0x33, version, 0, 0, 0, 0, 0, 0])
  header.writeUInt32BE(syncsafe(content.length), 6)
  return Buffer.concat([header, content])
}

/** An ID3v1.1 tag, with a track number. */
function id3v1(tag: {
  title: string
  artist: string
  year: string
  track: number
}): Buffer {
  const out = Buffer.alloc(128)
  out.write("TAG", 0, "latin1")
  out.write(tag.title, 3, 30, "latin1")
  out.write(tag.artist, 33, 30, "latin1")
  out.write(tag.year, 93, 4, "latin1")
  out[126] = tag.track
  out[127] = 255 // No genre
  return out
}

/**
 * Silent MPEG-1 Layer III frames, mono at 44.1 kHz and 32 kbps, after an
 * Info frame with their count, as LAME writes for a constant bitrate.
 * `variable` alternates 32 and 64 kbps.
 */
function mpeg(count: number, { info = true, variable = false } = {}): Buffer {
  // 144 * bitrate / 44100, without padding
  const frame = (fast: boolean) => {
    const bytes = Buffer.alloc(fast ? 208 : 104)
    bytes.set([0xff, 0xfb, fast ? 0x50 : 0x10, 0xc4]) // 32 or 64 kbps, mono
    return bytes
  }
  const frames = Array.from({ length: count }, (_, i) =>
    frame(variable && i % 2 === 1)
  )
  if (!info) return Buffer.concat(frames)
  const header = frame(false)
  // After the header and mono's 17 bytes of side information
  header.write("Info", 4 + 17, "latin1")
  header.writeUInt32BE(0x3, 25) // Frames and bytes
  header.writeUInt32BE(count + 1, 29)
  header.writeUInt32BE((count + 1) * header.length, 33)
  return Buffer.concat([header, ...frames])
}

/** An AIFF file with text chunks and an ID3v2 chunk added. */
function aiff(file: Buffer, texts: [string, string][], id3: Buffer): Buffer {
  if (file.toString("latin1", 0, 4) !== "FORM") throw new Error("Not AIFF")
  const chunks = [
    ...texts.map(([id, value]) => aiffChunk(id, Buffer.from(value, "latin1"))),
    aiffChunk("ID3 ", id3),
  ]
  const body = Buffer.concat([file.subarray(8), ...chunks])
  const header = Buffer.alloc(8)
  header.write("FORM", 0, "latin1")
  header.writeUInt32BE(body.length, 4)
  return Buffer.concat([header, body])
}

function aiffChunk(id: string, body: Buffer): Buffer {
  const header = Buffer.alloc(8)
  header.write(id, 0, "latin1")
  header.writeUInt32BE(body.length, 4)
  return Buffer.concat([header, body, pad(body)])
}

/**
 * An M4A file with atoms added to moov/udta/meta/ilst. afconvert leaves a
 * `free` atom between moov and mdat, which shrinks as moov grows, so the
 * audio data stays where its chunk offsets say it is.
 */
function mp4(file: Buffer, items: Buffer[]): Buffer {
  const added = Buffer.concat(items)
  const path = ["moov", "udta", "meta", "ilst"]
  const found: { offset: number; size: number }[] = []
  let start = 0
  let end = file.length
  for (const name of path) {
    const atom = findAtom(file, start, end, name)
    if (!atom) throw new Error(`No ${name} atom`)
    found.push(atom)
    // meta has 4 bytes of version and flags before its children
    start = atom.offset + 8 + (name === "meta" ? 4 : 0)
    end = atom.offset + atom.size
  }
  const moov = found[0]
  const free = findAtom(file, moov.offset + moov.size, file.length, "free")
  if (!free || free.offset !== moov.offset + moov.size) {
    throw new Error("No free atom after moov")
  }
  if (free.size - 8 < added.length) throw new Error("Not enough free space")
  const out = Buffer.concat([
    file.subarray(0, end),
    added,
    file.subarray(end, free.offset),
    atom("free", Buffer.alloc(free.size - 8 - added.length)),
    file.subarray(free.offset + free.size),
  ])
  for (const { offset, size } of found)
    out.writeUInt32BE(size + added.length, offset)
  return out
}

function findAtom(file: Buffer, start: number, end: number, name: string) {
  for (let offset = start; offset + 8 <= end;) {
    const size = file.readUInt32BE(offset)
    if (file.toString("latin1", offset + 4, offset + 8) === name) {
      return { offset, size }
    }
    if (size < 8) break
    offset += size
  }
  return null
}

function atom(name: string, body: Buffer): Buffer {
  const header = Buffer.alloc(8)
  header.writeUInt32BE(8 + body.length, 0)
  header.write(name, 4, "latin1")
  return Buffer.concat([header, body])
}

/** A `data` atom: its type, a locale of 0, and the value. */
function data(type: number, value: Buffer): Buffer {
  return atom("data", Buffer.concat([u32be(type), u32be(0), value]))
}

function text(name: string, value: string): Buffer {
  return atom(name, data(1, Buffer.from(value, "utf8")))
}

/** trkn and disk: a number and a total, as 16-bit numbers. */
function pair(name: string, number: number, total: number): Buffer {
  const value = Buffer.alloc(name === "trkn" ? 8 : 6)
  value.writeUInt16BE(number, 2)
  value.writeUInt16BE(total, 4)
  return atom(name, data(0, value))
}

/** A `----:com.apple.iTunes:<name>` atom, one data atom per value. */
function freeform(name: string, values: string[]): Buffer {
  const header = (kind: string, value: string) =>
    atom(kind, Buffer.concat([u32be(0), Buffer.from(value, "utf8")]))
  return atom(
    "----",
    Buffer.concat([
      header("mean", "com.apple.iTunes"),
      header("name", name),
      ...values.map((value) => data(1, Buffer.from(value, "utf8"))),
    ])
  )
}

function syncsafe(n: number): number {
  return (
    ((n & 0x0fe00000) << 3) |
    ((n & 0x001fc000) << 2) |
    ((n & 0x00003f80) << 1) |
    (n & 0x7f)
  )
}

function u32le(n: number): Buffer {
  const out = Buffer.alloc(4)
  out.writeUInt32LE(n)
  return out
}

function u32be(n: number): Buffer {
  const out = Buffer.alloc(4)
  out.writeUInt32BE(n)
  return out
}

/** A byte of padding after an odd-sized chunk. */
function pad(body: Buffer): Buffer {
  return Buffer.alloc(body.length % 2)
}
