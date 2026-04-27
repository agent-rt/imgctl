use std::path::Path;

use clap::ValueEnum;

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
#[clap(rename_all = "lowercase")]
pub enum ImageFormat {
    Png,
    Jpeg,
    Webp,
    Bmp,
    Gif,
    Tiff,
    Ico,
}

impl ImageFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Webp => "webp",
            Self::Bmp => "bmp",
            Self::Gif => "gif",
            Self::Tiff => "tiff",
            Self::Ico => "ico",
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "png" => Self::Png,
            "jpg" | "jpeg" => Self::Jpeg,
            "webp" => Self::Webp,
            "bmp" => Self::Bmp,
            "gif" => Self::Gif,
            "tif" | "tiff" => Self::Tiff,
            "ico" => Self::Ico,
            _ => return None,
        })
    }

    pub fn to_image(self) -> image::ImageFormat {
        match self {
            Self::Png => image::ImageFormat::Png,
            Self::Jpeg => image::ImageFormat::Jpeg,
            Self::Webp => image::ImageFormat::WebP,
            Self::Bmp => image::ImageFormat::Bmp,
            Self::Gif => image::ImageFormat::Gif,
            Self::Tiff => image::ImageFormat::Tiff,
            Self::Ico => image::ImageFormat::Ico,
        }
    }

    pub fn from_image(f: image::ImageFormat) -> Option<Self> {
        Some(match f {
            image::ImageFormat::Png => Self::Png,
            image::ImageFormat::Jpeg => Self::Jpeg,
            image::ImageFormat::WebP => Self::Webp,
            image::ImageFormat::Bmp => Self::Bmp,
            image::ImageFormat::Gif => Self::Gif,
            image::ImageFormat::Tiff => Self::Tiff,
            image::ImageFormat::Ico => Self::Ico,
            _ => return None,
        })
    }

    pub fn supports_quality(self) -> bool {
        matches!(self, Self::Jpeg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn from_path_handles_extensions() {
        assert_eq!(
            ImageFormat::from_path(&PathBuf::from("foo.png")),
            Some(ImageFormat::Png)
        );
        assert_eq!(
            ImageFormat::from_path(&PathBuf::from("foo.jpg")),
            Some(ImageFormat::Jpeg)
        );
        assert_eq!(
            ImageFormat::from_path(&PathBuf::from("foo.JPEG")),
            Some(ImageFormat::Jpeg)
        );
        assert_eq!(
            ImageFormat::from_path(&PathBuf::from("foo.tiff")),
            Some(ImageFormat::Tiff)
        );
        assert_eq!(
            ImageFormat::from_path(&PathBuf::from("foo.tif")),
            Some(ImageFormat::Tiff)
        );
        assert_eq!(ImageFormat::from_path(&PathBuf::from("foo.bin")), None);
        assert_eq!(ImageFormat::from_path(&PathBuf::from("noext")), None);
    }

    #[test]
    fn image_format_roundtrip() {
        for fmt in [
            ImageFormat::Png,
            ImageFormat::Jpeg,
            ImageFormat::Webp,
            ImageFormat::Bmp,
            ImageFormat::Gif,
            ImageFormat::Tiff,
            ImageFormat::Ico,
        ] {
            assert_eq!(ImageFormat::from_image(fmt.to_image()), Some(fmt));
        }
    }

    #[test]
    fn as_str_lowercase() {
        assert_eq!(ImageFormat::Png.as_str(), "png");
        assert_eq!(ImageFormat::Jpeg.as_str(), "jpeg");
        assert_eq!(ImageFormat::Webp.as_str(), "webp");
    }

    #[test]
    fn supports_quality_only_jpeg() {
        assert!(ImageFormat::Jpeg.supports_quality());
        assert!(!ImageFormat::Png.supports_quality());
        assert!(!ImageFormat::Webp.supports_quality());
    }
}
