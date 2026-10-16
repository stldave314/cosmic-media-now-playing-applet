// SPDX-License-Identifier: GPL-3.0-only

//! The album art as the media popup draws it: a square crop for the art card,
//! and a blurred copy for the backdrop behind everything.
//!
//! Album art arrives from MPRIS metadata, which any process on the session bus
//! can publish, so everything here treats the bytes as hostile: dimensions are
//! read from the header and checked before any pixels are decoded, and the
//! decoder runs under allocation limits as well.

use crate::constants::{
    ART_CARD_PIXELS, BACKDROP_BLUR_SIGMA, BACKDROP_HEIGHT, BACKDROP_WIDTH, MAX_ART_DIMENSION,
    POPUP_ART_SIZE, POPUP_WIDTH,
};
use image::imageops::FilterType;
use std::io::Cursor;

/// Tightly packed RGBA pixels, ready for `image::Handle::from_rgba`.
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl From<image::RgbaImage> for Pixels {
    fn from(img: image::RgbaImage) -> Self {
        Self { width: img.width(), height: img.height(), pixels: img.into_raw() }
    }
}

/// Both images the popup needs from one cover.
pub struct RenderedArt {
    /// Centered square crop for the art card, with transparent rounded corners.
    pub card: Pixels,
    /// Small, blurred, popup-shaped backdrop with transparent rounded corners.
    pub backdrop: Pixels,
    /// Average color of the backdrop, as RGB. Drawn under it to fill the
    /// slivers its rounded corners leave.
    pub average: [u8; 3],
}

fn reader(bytes: &[u8]) -> Option<image::ImageReader<Cursor<&[u8]>>> {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()
}

/// Width and height from the image header, without decoding any pixels.
/// `None` for formats this crate can't read.
fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    reader(bytes)?.into_dimensions().ok()
}

/// Whether the art is small enough to display. Art in a format we can't parse
/// passes, matching how it was handled before this check existed: the image
/// widget simply fails to show it.
pub fn within_limits(bytes: &[u8]) -> bool {
    dimensions(bytes).is_none_or(|(w, h)| w <= MAX_ART_DIMENSION && h <= MAX_ART_DIMENSION)
}

/// Decode the art, crop it to a centered square for the card, and shrink and
/// blur a copy for the backdrop. Corners are cut into both, to `card_radius`
/// and `backdrop_radius` logical pixels: the software renderer rounds a scaled
/// image's corners in physical pixels, so on a scaled display its rounding
/// comes out smaller than the theme's. Slow enough (tens of milliseconds for a
/// large cover) that it must run off the UI thread.
pub fn render(bytes: &[u8], card_radius: f32, backdrop_radius: f32) -> Option<RenderedArt> {
    let (w, h) = dimensions(bytes)?;
    if w == 0 || h == 0 || w > MAX_ART_DIMENSION || h > MAX_ART_DIMENSION {
        return None;
    }

    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_ART_DIMENSION);
    limits.max_image_height = Some(MAX_ART_DIMENSION);
    let mut reader = reader(bytes)?;
    reader.limits(limits);
    let art = reader.decode().ok()?;

    // Catmull-Rom keeps edges reasonably crisp both when shrinking a large
    // cover and when enlarging a tiny one.
    let card = art.resize_to_fill(ART_CARD_PIXELS, ART_CARD_PIXELS, FilterType::CatmullRom);

    // Built from the card, which is already square and small, so the backdrop
    // costs almost nothing extra.
    let mut backdrop = card
        .resize_to_fill(BACKDROP_WIDTH, BACKDROP_HEIGHT, FilterType::Triangle)
        .fast_blur(BACKDROP_BLUR_SIGMA)
        .into_rgba8();
    let average = average_rgb(&backdrop);
    round_corners(&mut backdrop, backdrop_radius * BACKDROP_WIDTH as f32 / POPUP_WIDTH);

    let mut card = card.into_rgba8();
    round_corners(&mut card, card_radius * ART_CARD_PIXELS as f32 / POPUP_ART_SIZE);

    Some(RenderedArt {
        card: card.into(),
        backdrop: backdrop.into(),
        average,
    })
}

fn average_rgb(img: &image::RgbaImage) -> [u8; 3] {
    let mut sum = [0u64; 3];
    for p in img.pixels() {
        for (total, &channel) in sum.iter_mut().zip(&p.0[..3]) {
            *total += u64::from(channel);
        }
    }
    let n = u64::from(img.width() * img.height()).max(1);
    sum.map(|total| (total / n) as u8)
}

/// Fade each corner outside a circle of `radius` pixels to transparent, over a
/// one-pixel edge that lies just *inside* the circle, so nothing of the image
/// is left beyond it.
fn round_corners(img: &mut image::RgbaImage, radius: f32) {
    let (w, h) = (img.width() as f32, img.height() as f32);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
        // Distance past the corner circle's center, per axis; zero outside
        // the corner squares.
        let dx = (radius - cx).max(cx - (w - radius)).max(0.0);
        let dy = (radius - cy).max(cy - (h - radius)).max(0.0);
        if dx > 0.0 && dy > 0.0 {
            let coverage = (radius - dx.hypot(dy)).clamp(0.0, 1.0);
            pixel.0[3] = (f32::from(pixel.0[3]) * coverage).round() as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(width, height, image::Rgb([200, 40, 90]));
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    /// A valid PNG header claiming the given size, with no pixel data behind
    /// it: what a hostile peer would publish to make the decoder allocate.
    fn png_header_only(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = png(1, 1);
        // IHDR: type at 12..16, data at 16..29 (width then height first,
        // big-endian), CRC over type + data at 29..33.
        bytes[16..20].copy_from_slice(&width.to_be_bytes());
        bytes[20..24].copy_from_slice(&height.to_be_bytes());
        let crc = crc32(&bytes[12..29]);
        bytes[29..33].copy_from_slice(&crc.to_be_bytes());
        bytes
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }

    #[test]
    fn wide_art_becomes_a_square_card_and_backdrop() {
        let art = render(&png(1280, 720), 8.0, 12.0).expect("rendered");
        let card = &art.card;
        assert_eq!((card.width, card.height), (ART_CARD_PIXELS, ART_CARD_PIXELS));
        assert_eq!(card.pixels.len(), (ART_CARD_PIXELS * ART_CARD_PIXELS * 4) as usize);

        let b = &art.backdrop;
        assert_eq!((b.width, b.height), (BACKDROP_WIDTH, BACKDROP_HEIGHT));
        assert_eq!(b.pixels.len(), (BACKDROP_WIDTH * BACKDROP_HEIGHT * 4) as usize);
        // A flat color stays that color through the crop and blur.
        let center = ((BACKDROP_HEIGHT / 2 * BACKDROP_WIDTH + BACKDROP_WIDTH / 2) * 4) as usize;
        assert_eq!(&b.pixels[center..center + 4], &[200, 40, 90, 255]);
        let card_center = ((card.height / 2 * card.width + card.width / 2) * 4) as usize;
        assert_eq!(&card.pixels[card_center..card_center + 4], &[200, 40, 90, 255]);
        assert_eq!(card.pixels[3], 0, "card corner should be cut away");
        assert_eq!(art.average, [200, 40, 90]);
    }

    #[test]
    fn backdrop_corners_are_transparent_and_edges_opaque() {
        let art = render(&png(100, 100), 8.0, 12.0).expect("rendered");
        let b = &art.backdrop;
        let alpha = |x: u32, y: u32| b.pixels[((y * b.width + x) * 4 + 3) as usize];
        let (r, bt) = (b.width - 1, b.height - 1);
        for (x, y) in [(0, 0), (r, 0), (0, bt), (r, bt)] {
            assert_eq!(alpha(x, y), 0, "corner ({x}, {y}) should be cut away");
        }
        // Mid-edges are untouched.
        assert_eq!(alpha(b.width / 2, 0), 255);
        assert_eq!(alpha(0, b.height / 2), 255);
        assert_eq!(alpha(b.width / 2, b.height / 2), 255);
    }

    #[test]
    fn small_art_is_scaled_up_to_the_card_size() {
        assert!(within_limits(&png(60, 60)));
        let art = render(&png(60, 60), 8.0, 12.0).expect("rendered");
        assert_eq!((art.card.width, art.card.height), (ART_CARD_PIXELS, ART_CARD_PIXELS));
    }

    #[test]
    fn oversized_dimensions_are_refused_before_decoding() {
        let hostile = png_header_only(MAX_ART_DIMENSION + 1, 10);
        // The header must actually parse, or this test proves nothing.
        assert_eq!(dimensions(&hostile), Some((MAX_ART_DIMENSION + 1, 10)));
        assert!(!within_limits(&hostile));
        assert!(render(&hostile, 8.0, 12.0).is_none());

        let tall = png_header_only(10, 60_000);
        assert_eq!(dimensions(&tall), Some((10, 60_000)));
        assert!(!within_limits(&tall));
        assert!(render(&tall, 8.0, 12.0).is_none());
    }

    #[test]
    fn art_at_the_limit_is_accepted() {
        let edge = png_header_only(MAX_ART_DIMENSION, MAX_ART_DIMENSION);
        assert_eq!(dimensions(&edge), Some((MAX_ART_DIMENSION, MAX_ART_DIMENSION)));
        assert!(within_limits(&edge));
    }

    #[test]
    fn garbage_is_not_rendered() {
        let junk = b"definitely not an image".to_vec();
        assert!(render(&junk, 8.0, 12.0).is_none());
        assert!(render(&[], 8.0, 12.0).is_none());
        // Truncated real data: valid header, missing body.
        let mut cut = png(64, 64);
        cut.truncate(40);
        assert!(render(&cut, 8.0, 12.0).is_none());
    }
}
