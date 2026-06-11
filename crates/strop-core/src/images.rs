//! Image import pipeline (docs/document-model.md §5b) — deterministic:
//!
//! - PNG/WebP within caps: stored byte-identical.
//! - JPEG within caps: byte-identical ONLY when it carries no EXIF —
//!   privacy (GPS) beats byte-fidelity, so EXIF-bearing JPEGs re-encode
//!   (orientation baked first). Decided 2026-06-11.
//! - GIF/BMP/TIFF: converted — alpha → PNG, else JPEG q88 (deterministic
//!   alpha test, not "looks photographic"). Animated GIF: first frame.
//! - Long edge > 2400px → downscale (CatmullRom; visually ≈ Lanczos3 for
//!   downscale, ~30% faster).
//! - Refused, never silently degraded: >12000px either edge (checked from
//!   the header before any pixel allocation), >8MB after the pipeline,
//!   undecodable input.
//!
//! Takes 0.3–0.8s for a 12MP photo in release — callers MUST run this on
//! a background thread.

use std::io::Cursor;

use image::metadata::Orientation;
use image::{
    DynamicImage, ImageDecoder, ImageEncoder, ImageFormat, ImageReader, Limits,
    codecs::jpeg::JpegEncoder, codecs::png::PngEncoder, imageops::FilterType,
};

pub const MAX_DIM_PRE_DECODE: u32 = 12_000;
pub const MAX_BYTES_POST: usize = 8 * 1024 * 1024;
pub const MAX_LONG_EDGE: u32 = 2_400;
const JPEG_QUALITY: u8 = 88;

#[derive(Debug, PartialEq, Eq)]
pub enum ImportError {
    UnknownFormat,
    /// Pre-decode dimensions or post-pipeline size over the caps.
    TooLarge(String),
    Decode(String),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownFormat => write!(f, "not a supported image format"),
            Self::TooLarge(why) => write!(f, "image refused: {why}"),
            Self::Decode(e) => write!(f, "image could not be decoded: {e}"),
        }
    }
}

#[derive(Debug)]
pub struct Imported {
    pub bytes: Vec<u8>,
    /// "png" | "jpg" | "webp" — for the asset id and alt-text defaults.
    pub ext: &'static str,
    pub width: u32,
    pub height: u32,
}

pub fn import_image(bytes: Vec<u8>) -> Result<Imported, ImportError> {
    // 1. Header-only sniff: format + dimensions, no pixel allocation.
    let reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|e| ImportError::Decode(e.to_string()))?;
    let format = reader.format().ok_or(ImportError::UnknownFormat)?;
    let (w, h) = reader
        .into_dimensions()
        .map_err(|e| ImportError::Decode(e.to_string()))?;
    if w > MAX_DIM_PRE_DECODE || h > MAX_DIM_PRE_DECODE {
        return Err(ImportError::TooLarge(format!(
            "{w}x{h} exceeds the {MAX_DIM_PRE_DECODE}px limit"
        )));
    }

    // 2. PNG/WebP passthrough needs no decoder at all.
    let within_caps = w.max(h) <= MAX_LONG_EDGE && bytes.len() <= MAX_BYTES_POST;
    if within_caps && matches!(format, ImageFormat::Png | ImageFormat::WebP) {
        let ext = if format == ImageFormat::Png { "png" } else { "webp" };
        return Ok(Imported {
            bytes,
            ext,
            width: w,
            height: h,
        });
    }

    // 3. Decode with strict limits (bomb protection at the decoder level).
    let mut reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|e| ImportError::Decode(e.to_string()))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIM_PRE_DECODE);
    limits.max_image_height = Some(MAX_DIM_PRE_DECODE);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| ImportError::Decode(e.to_string()))?;
    let has_exif = decoder
        .exif_metadata()
        .ok()
        .flatten()
        .is_some_and(|m| !m.is_empty());
    let orientation = decoder
        .orientation()
        .unwrap_or(Orientation::NoTransforms);

    // JPEG passthrough only when EXIF-free: stripping GPS beats fidelity.
    if within_caps && format == ImageFormat::Jpeg && !has_exif {
        drop(decoder);
        return Ok(Imported {
            bytes,
            ext: "jpg",
            width: w,
            height: h,
        });
    }

    let mut img = DynamicImage::from_decoder(decoder)
        .map_err(|e| ImportError::Decode(e.to_string()))?;

    // 4. Bake orientation; nothing re-attaches EXIF/ICC → metadata stripped.
    img.apply_orientation(orientation);

    // 5. Downscale only past the threshold (resize() preserves aspect).
    if img.width().max(img.height()) > MAX_LONG_EDGE {
        img = img.resize(MAX_LONG_EDGE, MAX_LONG_EDGE, FilterType::CatmullRom);
    }

    // 6. Encode by the deterministic alpha rule.
    let mut out = Vec::new();
    let (ext, width, height): (&'static str, u32, u32) = if img.color().has_alpha() {
        let rgba = img.into_rgba8();
        let (w, h) = (rgba.width(), rgba.height());
        PngEncoder::new(&mut out)
            .write_image(rgba.as_raw(), w, h, image::ExtendedColorType::Rgba8)
            .map_err(|e| ImportError::Decode(e.to_string()))?;
        ("png", w, h)
    } else {
        let rgb = img.into_rgb8();
        let (w, h) = (rgb.width(), rgb.height());
        JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY)
            .encode_image(&rgb)
            .map_err(|e| ImportError::Decode(e.to_string()))?;
        ("jpg", w, h)
    };

    if out.len() > MAX_BYTES_POST {
        return Err(ImportError::TooLarge(format!(
            "{} bytes after processing exceeds the {MAX_BYTES_POST}-byte limit",
            out.len()
        )));
    }
    Ok(Imported {
        bytes: out,
        ext,
        width,
        height,
    })
}

impl std::error::Error for ImportError {}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, Rgba, RgbImage, RgbaImage};

    fn png_bytes(img: &RgbaImage) -> Vec<u8> {
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn small_png_passes_through_byte_identical() {
        let img = RgbaImage::from_pixel(10, 8, Rgba([10, 20, 30, 255]));
        let bytes = png_bytes(&img);
        let imported = import_image(bytes.clone()).unwrap();
        assert_eq!(imported.bytes, bytes);
        assert_eq!((imported.width, imported.height), (10, 8));
        assert_eq!(imported.ext, "png");
    }

    #[test]
    fn bmp_converts_by_alpha_rule() {
        // Opaque BMP -> JPEG.
        let rgb = RgbImage::from_pixel(12, 12, Rgb([200, 100, 50]));
        let mut bmp = Vec::new();
        rgb.write_to(&mut Cursor::new(&mut bmp), ImageFormat::Bmp)
            .unwrap();
        let imported = import_image(bmp).unwrap();
        assert_eq!(imported.ext, "jpg");
    }

    #[test]
    fn oversized_long_edge_downscales() {
        // 3000x30 white PNG: long edge over 2400 -> scaled to 2400 wide.
        let img = RgbaImage::from_pixel(3000, 30, Rgba([255, 255, 255, 255]));
        let imported = import_image(png_bytes(&img)).unwrap();
        assert_eq!(imported.width, 2400);
        assert!(imported.height >= 23 && imported.height <= 25);
    }

    #[test]
    fn refuses_pre_decode_bombs_and_junk() {
        let img = RgbaImage::from_pixel(12001, 1, Rgba([0, 0, 0, 255]));
        match import_image(png_bytes(&img)) {
            Err(ImportError::TooLarge(_)) => {}
            other => panic!("expected TooLarge, got {other:?}"),
        }
        assert!(matches!(
            import_image(b"not an image at all".to_vec()),
            Err(ImportError::UnknownFormat)
        ));
    }
}
