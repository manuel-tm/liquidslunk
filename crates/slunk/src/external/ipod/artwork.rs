//! The iPod's artwork (ipod.md, "The ArtworkDB and thumbnails"):
//! `Artwork/ArtworkDB`, which says where each track's thumbnails are, and the
//! `.ithmb` files that hold their pixels. The iPod with video shows covers at
//! two sizes, both RGB565 little-endian: 100 × 100 (format 1028) and 200 × 200
//! (1029). Each image names its track by the track's `dbid`, and each track
//! names its image by the image's id.
//!
//! The ArtworkDB is chunks like the iTunesDB's: a `mhfd`, then three
//! sections: the images (`mhli`, an `mhii` each, with an `mhni` per size),
//! photo albums (`mhla`, empty for covers), and the thumbnail files
//! (`mhlf`, an `mhif` per format).

use image::DynamicImage;
use image::imageops::FilterType;

use super::bytes::{Reader, Writer, from_utf16, utf16};
use crate::Result;

/// The formats the iPod with video shows covers in: their id, and their
/// side in pixels
pub const FORMATS: [(u32, u32); 2] = [(1028, 100), (1029, 200)];

/// The first image id, as iTunes numbers them
pub const FIRST_IMAGE_ID: u32 = 0x64;

/// How large an `.ithmb` file grows before the next one starts, as libgpod
/// keeps them
pub const ITHMB_MAX_BYTES: u64 = 256_000_000;

// Header lengths, as written
const MHFD: usize = 0x84;
const MHSD: usize = 0x60;
const MHL: usize = 0x5c;
const MHII: usize = 0x98;
const MHOD: usize = 0x18;
const MHNI: usize = 0x4c;
const MHIF: usize = 0x7c;

/// A thumbnail's place in an `.ithmb` file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Thumb {
    pub format: u32,
    /// The file it's in, as the ArtworkDB names it: `:F1028_1.ithmb`
    pub file: String,
    pub offset: u32,
    pub size: u32,
    /// Its size with the padding before it, as libgpod and iTunes write it
    pub width: u16,
    pub height: u16,
    pub horizontal_padding: u16,
    pub vertical_padding: u16,
}

/// A track's image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub id: u32,
    /// Its track's `dbid`
    pub track: u64,
    /// The cover's size in bytes, before it was made thumbnails
    pub source_size: u32,
    pub thumbs: Vec<Thumb>,
}

/// Reads an ArtworkDB's images. The other sections aren't needed.
pub fn read(bytes: &[u8]) -> Result<Vec<Image>> {
    let r = Reader::new(bytes, "ArtworkDB");
    r.expect(0, b"mhfd")?;
    let end = r.total_length(0)?;
    let mut offset = r.header_length(0)?;
    let mut images = Vec::new();
    while offset < end {
        r.expect(offset, b"mhsd")?;
        let total = r.total_length(offset)?;
        if r.u16(offset + 12)? == 1 {
            let mhli = offset + r.header_length(offset)?;
            r.expect(mhli, b"mhli")?;
            let count = r.u32(mhli + 8)?;
            let mut at = mhli + r.header_length(mhli)?;
            for _ in 0..count {
                r.expect(at, b"mhii")?;
                images.push(read_image(&r, at)?);
                at += r.total_length(at)?;
            }
        }
        offset += total;
    }
    Ok(images)
}

fn read_image(r: &Reader, at: usize) -> Result<Image> {
    let mut image = Image {
        id: r.u32(at + 16)?,
        track: r.u64(at + 20)?,
        source_size: r.u32(at + 48)?,
        thumbs: Vec::new(),
    };
    let count = r.u32(at + 12)?;
    let mut offset = at + r.header_length(at)?;
    for _ in 0..count {
        r.expect(offset, b"mhod")?;
        let total = r.total_length(offset)?;
        // A container: its thumbnail's mhni
        if r.u16(offset + 12)? == 2 {
            let mhni = offset + r.header_length(offset)?;
            r.expect(mhni, b"mhni")?;
            let mut thumb = Thumb {
                format: r.u32(mhni + 16)?,
                offset: r.u32(mhni + 20)?,
                size: r.u32(mhni + 24)?,
                vertical_padding: r.u16(mhni + 28)?,
                horizontal_padding: r.u16(mhni + 30)?,
                height: r.u16(mhni + 32)?,
                width: r.u16(mhni + 34)?,
                file: String::new(),
            };
            let name = mhni + r.header_length(mhni)?;
            if r.u32(mhni + 12)? > 0 && r.is(name, b"mhod") && r.u16(name + 12)? == 3 {
                let length = r.u32(name + 24)? as usize;
                let text = r.bytes(name + 36, length)?;
                thumb.file = if r.u8(name + 28)? == 2 {
                    from_utf16(text)
                } else {
                    String::from_utf8_lossy(text).into_owned()
                };
            }
            image.thumbs.push(thumb);
        }
        offset += total;
    }
    Ok(image)
}

/// Writes an ArtworkDB for these images, whose thumbnails are already in
/// their files.
pub fn write(images: &[Image]) -> Vec<u8> {
    let mut w = Writer::new();
    w.tag(b"mhfd");
    w.u32(MHFD as u32);
    w.u32(0);
    w.u32(0);
    // 2 since iTunes 4.9: iTunes 7 erases an ArtworkDB with 1
    w.u32(2);
    w.u32(3);
    w.u32(0);
    // 0x1c: the highest image id, as libgpod writes it
    w.u32(images.iter().map(|image| image.id).max().unwrap_or(0));
    w.zeros(16);
    w.u8(2);
    w.pad_to(0, MHFD);

    let mhsd = section(&mut w, 1);
    list(&mut w, b"mhli", images.len());
    for image in images {
        let start = w.len();
        w.tag(b"mhii");
        w.u32(MHII as u32);
        w.u32(0);
        w.u32(image.thumbs.len() as u32);
        w.u32(image.id);
        w.u64(image.track);
        // 0x1c: unknown, rating, unknown, and two dates
        w.zeros(20);
        w.u32(image.source_size);
        w.pad_to(start, MHII);
        for thumb in &image.thumbs {
            let mhod = w.len();
            w.tag(b"mhod");
            w.u32(MHOD as u32);
            w.u32(0);
            // A container, for a thumbnail
            w.u16(2);
            w.pad_to(mhod, MHOD);
            let mhni = w.len();
            w.tag(b"mhni");
            w.u32(MHNI as u32);
            w.u32(0);
            w.u32(1);
            w.u32(thumb.format);
            w.u32(thumb.offset);
            w.u32(thumb.size);
            w.u16(thumb.vertical_padding);
            w.u16(thumb.horizontal_padding);
            w.u16(thumb.height);
            w.u16(thumb.width);
            w.pad_to(mhni, MHNI);
            file_name(&mut w, &thumb.file);
            w.close(mhni);
            w.close(mhod);
        }
        w.close(start);
    }
    w.close(mhsd);

    let mhsd = section(&mut w, 2);
    list(&mut w, b"mhla", 0);
    w.close(mhsd);

    let mhsd = section(&mut w, 3);
    list(&mut w, b"mhlf", FORMATS.len());
    for (format, side) in FORMATS {
        let start = w.len();
        w.tag(b"mhif");
        w.u32(MHIF as u32);
        w.u32(MHIF as u32);
        w.u32(0);
        w.u32(format);
        w.u32(side * side * 2);
        w.pad_to(start, MHIF);
    }
    w.close(mhsd);
    w.close(0);
    w.bytes
}

fn section(w: &mut Writer, kind: u16) -> usize {
    let start = w.len();
    w.tag(b"mhsd");
    w.u32(MHSD as u32);
    w.u32(0);
    w.u16(kind);
    w.pad_to(start, MHSD);
    start
}

fn list(w: &mut Writer, tag: &[u8; 4], count: usize) {
    let start = w.len();
    w.tag(tag);
    w.u32(MHL as u32);
    w.u32(count as u32);
    w.pad_to(start, MHL);
}

/// A thumbnail's file name, a string `mhod` of type 3 in UTF-16LE, padded
/// to four bytes.
fn file_name(w: &mut Writer, name: &str) {
    let text = utf16(name);
    let padding = (4 - (36 + text.len()) % 4) % 4;
    let start = w.len();
    w.tag(b"mhod");
    // The header's length, without its string's fields
    w.u32(MHOD as u32);
    w.u32(0);
    w.u16(3);
    w.u8(0);
    w.u8(padding as u8);
    w.zeros(8);
    w.u32(text.len() as u32);
    // UTF-16LE
    w.u8(2);
    w.zeros(7);
    w.raw(&text);
    w.zeros(padding);
    w.close(start);
}

/// A cover's thumbnails, a format's each: its pixels, RGB565 little-endian,
/// the image scaled to fit the square and centred on white, and its size
/// and padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pixels {
    pub format: u32,
    pub bytes: Vec<u8>,
    pub width: u16,
    pub height: u16,
    pub horizontal_padding: u16,
    pub vertical_padding: u16,
}

/// Makes a cover's thumbnails, in each of `FORMATS`, from an image.
pub fn thumbnails(cover: &DynamicImage) -> Vec<Pixels> {
    FORMATS
        .iter()
        .map(|&(format, side)| {
            let scaled = cover.resize(side, side, FilterType::Lanczos3).to_rgba8();
            let (width, height) = scaled.dimensions();
            let left = (side - width) / 2;
            let top = (side - height) / 2;
            let white = pack(255, 255, 255);
            let mut bytes = Vec::with_capacity((side * side * 2) as usize);
            for y in 0..side {
                for x in 0..side {
                    let inside = x >= left && x < left + width && y >= top && y < top + height;
                    let pixel = if inside {
                        let [r, g, b, a] = scaled.get_pixel(x - left, y - top).0;
                        let over = |c: u8| {
                            let (c, a) = (u32::from(c), u32::from(a));
                            ((c * a + 255 * (255 - a) + 127) / 255) as u8
                        };
                        pack(over(r), over(g), over(b))
                    } else {
                        white
                    };
                    bytes.extend_from_slice(&pixel.to_le_bytes());
                }
            }
            Pixels {
                format,
                bytes,
                width: (left + width) as u16,
                height: (top + height) as u16,
                horizontal_padding: left as u16,
                vertical_padding: top as u16,
            }
        })
        .collect()
}

/// A pixel as RGB565: 5 bits of red, 6 of green and 5 of blue.
fn pack(r: u8, g: u8, b: u8) -> u16 {
    (u16::from(r >> 3) << 11) | (u16::from(g >> 2) << 5) | u16::from(b >> 3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn an_artwork_database_is_read_back() {
        let thumb = |format, side: u16| Thumb {
            format,
            file: format!(":F{format}_1.ithmb"),
            offset: 0,
            size: u32::from(side) * u32::from(side) * 2,
            width: side,
            height: side - 10,
            horizontal_padding: 0,
            vertical_padding: 5,
        };
        let images = vec![
            Image {
                id: FIRST_IMAGE_ID,
                track: 0xabcd,
                source_size: 12_345,
                thumbs: vec![thumb(1028, 100), thumb(1029, 200)],
            },
            // A release's tracks share their pixels
            Image {
                id: FIRST_IMAGE_ID + 1,
                track: 0xabce,
                source_size: 12_345,
                thumbs: vec![thumb(1028, 100), thumb(1029, 200)],
            },
        ];
        let bytes = write(&images);
        assert_eq!(read(&bytes).unwrap(), images);
        assert!(read(&bytes[..50]).is_err());
    }

    #[test]
    fn thumbnails_fit_the_square_on_white() {
        let wide =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(400, 200, Rgba([255, 0, 0, 255])));
        let made = thumbnails(&wide);
        assert_eq!(made.len(), 2);
        let small = &made[0];
        assert_eq!(small.bytes.len(), 100 * 100 * 2);
        assert_eq!(
            (small.width, small.height, small.vertical_padding),
            (100, 75, 25)
        );
        let pixel = |x: usize, y: usize| {
            let at = (y * 100 + x) * 2;
            u16::from_le_bytes([small.bytes[at], small.bytes[at + 1]])
        };
        assert_eq!(pixel(50, 10), 0xffff);
        assert_eq!(pixel(50, 50), 0xf800);
        assert_eq!(made[1].bytes.len(), 200 * 200 * 2);
    }
}
