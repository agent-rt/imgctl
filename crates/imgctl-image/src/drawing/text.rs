use std::path::Path;

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_text_mut, text_size};

use imgctl_core::{ColorRgba, Error, Result};

use crate::drawing::alpha::blend_pixel_over;

/// Embedded default font (NotoSans Regular, OFL — Latin/Greek/Cyrillic only).
/// For CJK or other scripts, pass `--font` with a path or system family name.
const DEFAULT_FONT: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Resolve a `--font` value to a usable `FontVec`.
///
/// Resolution order:
/// 1. `None` → embedded NotoSans Regular
/// 2. `Some(s)` and `s` is an existing file → load that file
/// 3. `Some(s)` otherwise → fontdb system query by family name
pub fn load_font(font_arg: Option<&str>) -> Result<FontVec> {
    match font_arg {
        None => FontVec::try_from_vec(DEFAULT_FONT.to_vec())
            .map_err(|e| Error::Internal(format!("embedded font invalid: {e}"))),
        Some(s) => {
            let path = Path::new(s);
            if path.is_file() {
                let bytes = std::fs::read(path)?;
                return load_from_bytes(bytes, 0);
            }
            // System font lookup.
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            let query = fontdb::Query {
                families: &[fontdb::Family::Name(s)],
                ..Default::default()
            };
            let id = db.query(&query).ok_or_else(|| {
                Error::NotFound(format!(
                    "font: not a file path and not a system family: {s}"
                ))
            })?;
            let result = db.with_face_data(id, |bytes, face_index| {
                let owned = bytes.to_vec();
                if face_index == 0 {
                    FontVec::try_from_vec(owned)
                } else {
                    FontVec::try_from_vec_and_index(owned, face_index)
                }
            });
            match result {
                Some(Ok(font)) => Ok(font),
                Some(Err(e)) => Err(Error::Image(format!("font parse: {e}"))),
                None => Err(Error::Internal("fontdb face data missing".into())),
            }
        }
    }
}

fn load_from_bytes(bytes: Vec<u8>, face_index: u32) -> Result<FontVec> {
    let result = if face_index == 0 {
        FontVec::try_from_vec(bytes)
    } else {
        FontVec::try_from_vec_and_index(bytes, face_index)
    };
    result.map_err(|e| Error::Image(format!("font parse: {e}")))
}

/// Top-left origin computed from `align` + caller-provided `x`.
fn align_origin_x(align: TextAlign, anchor_x: i32, width: i32) -> i32 {
    match align {
        TextAlign::Left => anchor_x,
        TextAlign::Center => anchor_x - width / 2,
        TextAlign::Right => anchor_x - width,
    }
}

/// Render `text` onto `img` at (x,y) with `size` px and `color`.
///
/// `align` shifts the rendering origin around `x` (left/center/right anchor).
/// `bg` (if Some) fills the text bounding box with that color before drawing.
pub fn render_text(
    img: &mut RgbaImage,
    text: &str,
    x: i32,
    y: i32,
    size: u32,
    color: ColorRgba,
    align: TextAlign,
    font: &FontVec,
    bg: Option<ColorRgba>,
) -> Result<()> {
    if size == 0 {
        return Err(Error::InvalidArgument("text --size must be > 0".into()));
    }
    let scale = PxScale::from(size as f32);
    let (text_w, text_h) = text_size(scale, font, text);
    let text_w_i = text_w as i32;
    let text_h_i = text_h as i32;
    let origin_x = align_origin_x(align, x, text_w_i);

    if let Some(bg_color) = bg {
        // Background bbox: tighter to glyph ascent; use scaled font ascent for top.
        let scaled = font.as_scaled(scale);
        let ascent = scaled.ascent().ceil() as i32;
        let bg_top = y;
        let bg_left = origin_x;
        let bg_w = text_w_i;
        let bg_h = ascent.max(text_h_i);
        fill_alpha_rect(img, bg_left, bg_top, bg_w, bg_h, bg_color);
    }

    draw_text_mut(img, Rgba(color.0), origin_x, y, scale, font, text);
    Ok(())
}

fn fill_alpha_rect(img: &mut RgbaImage, x: i32, y: i32, w: i32, h: i32, color: ColorRgba) {
    if w <= 0 || h <= 0 {
        return;
    }
    let img_w = img.width() as i32;
    let img_h = img.height() as i32;
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w).min(img_w);
    let y1 = (y + h).min(img_h);
    let src = Rgba(color.0);
    for yy in y0..y1 {
        for xx in x0..x1 {
            blend_pixel_over(img.get_pixel_mut(xx as u32, yy as u32), src);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn unique_temp(suffix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "imgctl-font-{}-{nanos}-{suffix}",
            std::process::id()
        ))
    }

    #[test]
    fn default_font_loads() {
        let f = load_font(None).unwrap();
        // Smoke: glyph for 'A' is non-zero.
        let id = f.glyph_id('A');
        assert_ne!(id.0, 0);
    }

    #[test]
    fn font_path_loads() {
        let path = unique_temp("custom.ttf");
        std::fs::write(&path, DEFAULT_FONT).unwrap();
        let f = load_font(Some(&path.to_string_lossy())).unwrap();
        assert_ne!(f.glyph_id('A').0, 0);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn font_unknown_path_or_name_errs() {
        let err = load_font(Some("__definitely_not_a_font_or_path__")).unwrap_err();
        assert_eq!(err.code(), "NOT_FOUND");
    }

    #[test]
    fn align_origin_x_math() {
        assert_eq!(align_origin_x(TextAlign::Left, 100, 50), 100);
        assert_eq!(align_origin_x(TextAlign::Center, 100, 50), 75);
        assert_eq!(align_origin_x(TextAlign::Right, 100, 50), 50);
    }

    #[test]
    fn render_text_marks_glyph_pixels() {
        let mut img = RgbaImage::from_pixel(200, 100, Rgba([255, 255, 255, 255]));
        let font = load_font(None).unwrap();
        render_text(
            &mut img,
            "ABC",
            10,
            10,
            32,
            ColorRgba::rgba(255, 0, 0, 255),
            TextAlign::Left,
            &font,
            None,
        )
        .unwrap();

        // Some pixel inside the text region should now be reddish (R > 0, G/B reduced).
        let mut found_red = false;
        for y in 10..50 {
            for x in 10..100 {
                let p = img.get_pixel(x, y).0;
                if p[0] > 100 && p[1] < 200 && p[2] < 200 {
                    found_red = true;
                    break;
                }
            }
            if found_red {
                break;
            }
        }
        assert!(found_red, "expected at least one red glyph pixel");
    }

    #[test]
    fn render_text_with_translucent_bg_blends() {
        let mut img = RgbaImage::from_pixel(200, 100, Rgba([255, 255, 255, 255]));
        let font = load_font(None).unwrap();
        render_text(
            &mut img,
            "X",
            20,
            20,
            32,
            ColorRgba::rgba(0, 0, 0, 255),
            TextAlign::Left,
            &font,
            Some(ColorRgba::rgba(255, 0, 0, 128)),
        )
        .unwrap();
        // Background area should have R > G ≈ B (red tint).
        let p = img.get_pixel(22, 22).0;
        assert!(p[0] > p[1], "expected red tint: {p:?}");
    }

    #[test]
    fn render_text_zero_size_errs() {
        let mut img = RgbaImage::from_pixel(50, 50, Rgba([255; 4]));
        let font = load_font(None).unwrap();
        let err = render_text(
            &mut img,
            "x",
            0,
            0,
            0,
            ColorRgba::rgba(0, 0, 0, 255),
            TextAlign::Left,
            &font,
            None,
        )
        .unwrap_err();
        assert_eq!(err.code(), "INVALID_ARGUMENT");
    }
}
