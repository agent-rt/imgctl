use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Region {
    /// Resolve region against an image:
    /// - negative `x`/`y` are interpreted as offsets from the right/bottom edge
    /// - origin and extent are clamped to image bounds
    /// - returns `Err(InvalidArgument)` if the resolved region has zero area
    pub fn resolve(&self, image: Size) -> Result<Region, Error> {
        let img_w = i64::from(image.w);
        let img_h = i64::from(image.h);

        let raw_x = if self.x < 0 { img_w + i64::from(self.x) } else { i64::from(self.x) };
        let raw_y = if self.y < 0 { img_h + i64::from(self.y) } else { i64::from(self.y) };

        let x = raw_x.max(0).min(img_w);
        let y = raw_y.max(0).min(img_h);

        let right = (raw_x + i64::from(self.w)).max(0).min(img_w);
        let bottom = (raw_y + i64::from(self.h)).max(0).min(img_h);

        let w = right - x;
        let h = bottom - y;

        if w <= 0 || h <= 0 {
            return Err(Error::InvalidArgument(format!(
                "region resolves to empty area: {self:?} on {image:?}"
            )));
        }

        Ok(Region {
            x: x as i32,
            y: y as i32,
            w: w as u32,
            h: h as u32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: u32, h: u32) -> Size {
        Size { w, h }
    }

    #[test]
    fn resolve_within_bounds() {
        let r = Region { x: 10, y: 20, w: 100, h: 50 };
        assert_eq!(r.resolve(img(200, 200)).unwrap(), Region { x: 10, y: 20, w: 100, h: 50 });
    }

    #[test]
    fn resolve_clamps_overflow() {
        let r = Region { x: 50, y: 50, w: 200, h: 200 };
        assert_eq!(r.resolve(img(100, 100)).unwrap(), Region { x: 50, y: 50, w: 50, h: 50 });
    }

    #[test]
    fn resolve_negative_origin_from_right_bottom() {
        let r = Region { x: -50, y: -30, w: 30, h: 20 };
        assert_eq!(
            r.resolve(img(200, 200)).unwrap(),
            Region { x: 150, y: 170, w: 30, h: 20 }
        );
    }

    #[test]
    fn resolve_zero_width_errs() {
        let r = Region { x: 10, y: 10, w: 0, h: 50 };
        assert!(r.resolve(img(200, 200)).is_err());
    }

    #[test]
    fn resolve_outside_image_errs() {
        let r = Region { x: 500, y: 500, w: 100, h: 100 };
        assert!(r.resolve(img(200, 200)).is_err());
    }

    #[test]
    fn region_serde_roundtrip() {
        let r = Region { x: 1, y: 2, w: 3, h: 4 };
        let json = serde_json::to_string(&r).unwrap();
        let back: Region = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
    }
}
