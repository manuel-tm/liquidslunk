//! Covers as external libraries write them (backend.md, "Syncing a folder
//! library"): a release's cover, read with the `image` crate, at most the
//! profile's size, never enlarged, as a JPEG.

use std::io::Cursor;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, ImageReader, Rgb, RgbImage};

/// A JPEG's quality when liquidslunk encodes one
const QUALITY: u8 = 90;

/// What covers are read from. AVIF, HEIC and SVG need decoders outside Rust.
const SUPPORTED: [ImageFormat; 6] = [
    ImageFormat::Jpeg,
    ImageFormat::Png,
    ImageFormat::Gif,
    ImageFormat::Bmp,
    ImageFormat::Tiff,
    ImageFormat::WebP,
];

/// Whether a cover can be exported, from its headers alone, as a plan
/// checks it before anything is decoded. Why not, when it can't.
pub fn check(path: &Path) -> Result<(), String> {
    let reader = ImageReader::open(path)
        .map_err(|error| format!("Its cover can't be read: {error}"))?
        .with_guessed_format()
        .map_err(|error| format!("Its cover can't be read: {error}"))?;
    match reader.format() {
        Some(format) if SUPPORTED.contains(&format) => reader
            .into_dimensions()
            .map(|_| ())
            .map_err(|_| "Its cover can't be read".to_string()),
        _ => Err(unsupported(path)),
    }
}

/// A release's cover as a JPEG at most `max_px` on its longest side, or why
/// it can't be exported. A JPEG already within the size is written as it
/// is. Transparent pixels are made white, since JPEG has no transparency.
pub fn jpeg(path: &Path, max_px: u32) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("Its cover can't be read: {error}"))?;
    let format = image::guess_format(&bytes).map_err(|_| unsupported(path))?;
    if !SUPPORTED.contains(&format) {
        return Err(unsupported(path));
    }
    let reader = || ImageReader::with_format(Cursor::new(&bytes), format);
    if format == ImageFormat::Jpeg {
        let (width, height) = reader()
            .into_dimensions()
            .map_err(|_| "Its cover can't be read".to_string())?;
        if width.max(height) <= max_px {
            return Ok(bytes);
        }
    }
    let image = reader()
        .decode()
        .map_err(|_| "Its cover can't be read".to_string())?;
    let image = if image.width().max(image.height()) > max_px {
        image.resize(max_px, max_px, FilterType::Lanczos3)
    } else {
        image
    };
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, QUALITY)
        .encode_image(&on_white(&image))
        .map_err(|error| format!("Its cover can't be encoded: {error}"))?;
    Ok(jpeg)
}

/// Why a cover in another format isn't exported.
fn unsupported(path: &Path) -> String {
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_uppercase())
        .unwrap_or_default();
    if extension.is_empty() {
        "Its cover isn't a JPEG, PNG, GIF, BMP, TIFF or WebP".to_string()
    } else {
        format!(
            "Its cover is in {extension}, which isn't exported: only JPEG, PNG, GIF, BMP, TIFF and WebP are"
        )
    }
}

/// An image's pixels as RGB, laid on white.
fn on_white(image: &DynamicImage) -> RgbImage {
    let rgba = image.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let over = |c: u8| {
            let (c, a) = (u32::from(c), u32::from(a));
            ((c * a + 255 * (255 - a) + 127) / 255) as u8
        };
        Rgb([over(r), over(g), over(b)])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageEncoder, Rgba, RgbaImage};

    fn write(
        dir: &Path,
        name: &str,
        width: u32,
        height: u32,
        format: ImageFormat,
    ) -> std::path::PathBuf {
        let path = dir.join(name);
        let image = RgbaImage::from_pixel(width, height, Rgba([200, 10, 10, 0]));
        let mut bytes = Vec::new();
        match format {
            ImageFormat::Png => image::codecs::png::PngEncoder::new(&mut bytes)
                .write_image(
                    image.as_raw(),
                    width,
                    height,
                    image::ExtendedColorType::Rgba8,
                )
                .unwrap(),
            _ => JpegEncoder::new(&mut bytes)
                .encode_image(&DynamicImage::ImageRgba8(image).to_rgb8())
                .unwrap(),
        }
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn size(jpeg: &[u8]) -> (u32, u32) {
        ImageReader::with_format(Cursor::new(jpeg), ImageFormat::Jpeg)
            .into_dimensions()
            .unwrap()
    }

    #[test]
    fn covers_fit_the_size_and_are_never_enlarged() {
        let dir = tempfile::tempdir().unwrap();
        let big = write(dir.path(), "big.png", 1200, 800, ImageFormat::Png);
        let out = jpeg(&big, 600).unwrap();
        assert_eq!(size(&out), (600, 400));
        // Transparent pixels are white
        let image = image::load_from_memory(&out).unwrap().to_rgb8();
        assert!(image.get_pixel(10, 10).0.iter().all(|c| *c > 240));
        let small = write(dir.path(), "small.png", 300, 300, ImageFormat::Png);
        assert_eq!(size(&jpeg(&small, 600).unwrap()), (300, 300));
        // A JPEG within the size is written as it is
        let within = write(dir.path(), "within.jpg", 500, 500, ImageFormat::Jpeg);
        assert_eq!(jpeg(&within, 600).unwrap(), std::fs::read(&within).unwrap());
        let over = write(dir.path(), "over.jpg", 900, 900, ImageFormat::Jpeg);
        assert_eq!(size(&jpeg(&over, 600).unwrap()), (600, 600));
    }

    #[test]
    fn other_formats_are_left_out_with_their_reason() {
        let dir = tempfile::tempdir().unwrap();
        let svg = dir.path().join("cover.svg");
        std::fs::write(&svg, "<svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap();
        assert_eq!(
            jpeg(&svg, 600),
            Err("Its cover is in SVG, which isn't exported: only JPEG, PNG, GIF, BMP, TIFF and WebP are".into())
        );
        assert_eq!(jpeg(&svg, 600).map(|_| ()), check(&svg));
        assert!(jpeg(&dir.path().join("missing.jpg"), 600).is_err());
        assert!(check(&dir.path().join("missing.jpg")).is_err());
        let png = write(dir.path(), "cover.png", 10, 10, ImageFormat::Png);
        assert_eq!(check(&png), Ok(()));
    }
}
