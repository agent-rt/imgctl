use image::Rgba;

/// Source-over alpha compositing for RGBA pixels.
///
/// Fast paths: src.alpha == 0 (no-op), src.alpha == 255 (overwrite).
/// Otherwise: classic Porter-Duff "source over destination" with integer math.
pub fn blend_pixel_over(dst: &mut Rgba<u8>, src: Rgba<u8>) {
    let sa = src.0[3] as u32;
    if sa == 0 {
        return;
    }
    if sa == 255 {
        *dst = src;
        return;
    }
    let inv = 255 - sa;
    for i in 0..3 {
        let s = src.0[i] as u32;
        let d = dst.0[i] as u32;
        dst.0[i] = ((s * sa + d * inv) / 255) as u8;
    }
    let da = dst.0[3] as u32;
    dst.0[3] = (sa + da * inv / 255).min(255) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fully_transparent_src_no_change() {
        let mut dst = Rgba([10, 20, 30, 200]);
        blend_pixel_over(&mut dst, Rgba([255, 255, 255, 0]));
        assert_eq!(dst.0, [10, 20, 30, 200]);
    }

    #[test]
    fn fully_opaque_src_overwrites() {
        let mut dst = Rgba([10, 20, 30, 200]);
        blend_pixel_over(&mut dst, Rgba([255, 0, 0, 255]));
        assert_eq!(dst.0, [255, 0, 0, 255]);
    }

    #[test]
    fn half_alpha_src_blends_halfway() {
        // 50% red over fully opaque blue
        let mut dst = Rgba([0, 0, 255, 255]);
        blend_pixel_over(&mut dst, Rgba([255, 0, 0, 128]));
        // expected ≈ (255*128/255 + 0*127/255, 0, 255*127/255, ...)
        // R ≈ 128, G = 0, B ≈ 127
        assert!((dst.0[0] as i32 - 128).abs() <= 1);
        assert_eq!(dst.0[1], 0);
        assert!((dst.0[2] as i32 - 127).abs() <= 1);
        assert_eq!(dst.0[3], 255);
    }
}
