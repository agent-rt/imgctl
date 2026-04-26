use std::sync::Arc;

use resvg::usvg;

use imgctl_core::{Error, Result};

/// Rasterize an SVG string into PNG bytes.
///
/// `target_width`:
///   - `Some(w)` — scale width to `w`, keep aspect ratio
///   - `None`    — use the SVG's intrinsic width
pub fn render(svg: &str, target_width: Option<u32>) -> Result<Vec<u8>> {
    let mut fontdb = fontdb::Database::new();
    fontdb.load_system_fonts();

    let opt = usvg::Options {
        fontdb: Arc::new(fontdb),
        ..Default::default()
    };

    let tree = usvg::Tree::from_str(svg, &opt)
        .map_err(|e| Error::Image(format!("svg parse: {e}")))?;

    let size = tree.size();
    let intrinsic_w = size.width().max(1.0);
    let intrinsic_h = size.height().max(1.0);

    let scale = match target_width {
        Some(w) if w > 0 => w as f32 / intrinsic_w,
        _ => 1.0,
    };

    let pixmap_w = (intrinsic_w * scale).round().max(1.0) as u32;
    let pixmap_h = (intrinsic_h * scale).round().max(1.0) as u32;

    let mut pixmap = tiny_skia::Pixmap::new(pixmap_w, pixmap_h)
        .ok_or_else(|| Error::Image(format!("pixmap alloc: {pixmap_w}x{pixmap_h}")))?;

    let transform = tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    pixmap
        .encode_png()
        .map_err(|e| Error::Image(format!("png encode: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_minimal_svg_to_png() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20">
            <rect x="0" y="0" width="20" height="20" fill="red"/>
        </svg>"#;
        let bytes = render(svg, None).unwrap();
        // PNG magic
        assert_eq!(
            &bytes[0..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
        // round-trip via image crate
        let img = image::load_from_memory(&bytes).unwrap();
        assert_eq!(img.width(), 20);
        assert_eq!(img.height(), 20);
    }

    #[test]
    fn target_width_scales_aspect() {
        // 100x50 SVG, target 200 width → 200x100 PNG
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50">
            <rect x="0" y="0" width="100" height="50" fill="blue"/>
        </svg>"#;
        let bytes = render(svg, Some(200)).unwrap();
        let img = image::load_from_memory(&bytes).unwrap();
        assert_eq!(img.width(), 200);
        assert_eq!(img.height(), 100);
    }

    #[test]
    fn invalid_svg_errs() {
        let err = render("not svg", None).unwrap_err();
        assert_eq!(err.code(), "IMAGE_ERROR");
    }
}
