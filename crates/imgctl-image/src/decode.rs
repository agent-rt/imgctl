use image::DynamicImage;

use imgctl_core::{Error, InputSource, Result};

use crate::format::ImageFormat;

#[derive(Debug)]
pub struct Decoded {
    pub image: DynamicImage,
    pub format: ImageFormat,
}

/// Load an image from a file path or stdin, sniffing the actual format from
/// the magic bytes (extension is ignored).
pub fn load(src: &InputSource) -> Result<Decoded> {
    let bytes = src.read_all()?;
    let raw_fmt = image::guess_format(&bytes)
        .map_err(|e| Error::UnsupportedFormat(e.to_string()))?;
    let image = image::load_from_memory_with_format(&bytes, raw_fmt)
        .map_err(|e| Error::Image(e.to_string()))?;
    let format = ImageFormat::from_image(raw_fmt)
        .ok_or_else(|| Error::UnsupportedFormat(format!("{raw_fmt:?}")))?;
    Ok(Decoded { image, format })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::time::SystemTime;

    use image::{DynamicImage, Rgba, RgbaImage};

    fn unique_temp(suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "imgctl-decode-{}-{nanos}-{suffix}",
            std::process::id()
        ))
    }

    fn fixture_png_bytes() -> Vec<u8> {
        let img = RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 255]));
        let dyn_img = DynamicImage::ImageRgba8(img);
        let mut buf = Vec::new();
        dyn_img
            .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn load_png_from_file() {
        let bytes = fixture_png_bytes();
        let path = unique_temp("png");
        std::fs::write(&path, &bytes).unwrap();

        let d = load(&InputSource::File(path.clone())).unwrap();
        assert_eq!(d.format, ImageFormat::Png);
        assert_eq!(d.image.width(), 2);
        assert_eq!(d.image.height(), 2);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_unsupported_bytes_errs() {
        let path = unique_temp("junk");
        std::fs::write(&path, b"not an image at all").unwrap();
        let err = load(&InputSource::File(path.clone())).unwrap_err();
        assert_eq!(err.code(), "UNSUPPORTED_FORMAT");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_ignores_extension() {
        // PNG bytes saved with .jpg extension — should still detect as PNG.
        let bytes = fixture_png_bytes();
        let path = unique_temp("misnamed.jpg");
        std::fs::write(&path, &bytes).unwrap();
        let d = load(&InputSource::File(path.clone())).unwrap();
        assert_eq!(d.format, ImageFormat::Png);
        let _ = std::fs::remove_file(&path);
    }
}
