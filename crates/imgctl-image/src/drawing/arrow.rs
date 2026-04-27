use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_line_segment_mut, draw_polygon_mut};
use imageproc::point::Point as ImgPoint;

use imgctl_core::ColorRgba;

use crate::drawing::alpha::blend_pixel_over;

#[derive(Clone, Copy, Debug)]
pub enum ArrowStyle {
    Solid,
    Dashed,
}

const DASH_LEN: f32 = 8.0;
const GAP_LEN: f32 = 6.0;

/// Draw an arrow from `from` to `to` with `color`, given line width and head size.
///
/// The line is shortened so it doesn't extend past the arrowhead base.
/// Alpha is honored throughout via the blend helper.
pub fn draw_arrow(
    img: &mut RgbaImage,
    from: (i32, i32),
    to: (i32, i32),
    color: ColorRgba,
    line_width: u32,
    head_size: u32,
    style: ArrowStyle,
) {
    let line_width = line_width.max(1);
    let head_size = head_size.max(3);

    let dx = (to.0 - from.0) as f32;
    let dy = (to.1 - from.1) as f32;
    let length = (dx * dx + dy * dy).sqrt();
    if length < 1.0 {
        return;
    }
    let (ux, uy) = (dx / length, dy / length);
    let (perp_x, perp_y) = (-uy, ux);

    // Shorten the line so the arrowhead can sit on top.
    let line_end = (
        (to.0 as f32 - ux * head_size as f32) as i32,
        (to.1 as f32 - uy * head_size as f32) as i32,
    );

    let src = Rgba(color.0);
    let alpha_blend = color.a() < 255;

    match style {
        ArrowStyle::Solid => draw_thick_line(img, from, line_end, line_width, src, alpha_blend),
        ArrowStyle::Dashed => draw_dashed_line(img, from, line_end, line_width, src, alpha_blend),
    }

    // Arrowhead triangle: tip at `to`, base centered at `to - head_size * u`.
    let base_x = to.0 as f32 - ux * head_size as f32;
    let base_y = to.1 as f32 - uy * head_size as f32;
    let half_base = head_size as f32 * 0.5;
    let left = (
        (base_x + perp_x * half_base).round() as i32,
        (base_y + perp_y * half_base).round() as i32,
    );
    let right = (
        (base_x - perp_x * half_base).round() as i32,
        (base_y - perp_y * half_base).round() as i32,
    );

    let tri = [
        ImgPoint::new(to.0, to.1),
        ImgPoint::new(left.0, left.1),
        ImgPoint::new(right.0, right.1),
    ];
    if alpha_blend {
        fill_triangle_alpha(img, tri[0], tri[1], tri[2], src);
    } else {
        // imageproc rejects polygons whose first/last points coincide; ours never do.
        draw_polygon_mut(img, &tri, src);
    }
}

fn draw_thick_line(
    img: &mut RgbaImage,
    from: (i32, i32),
    to: (i32, i32),
    width: u32,
    src: Rgba<u8>,
    alpha_blend: bool,
) {
    let dx = (to.0 - from.0) as f32;
    let dy = (to.1 - from.1) as f32;
    let length = (dx * dx + dy * dy).sqrt();
    if length < 1.0 {
        return;
    }
    let (ux, uy) = (dx / length, dy / length);
    let (perp_x, perp_y) = (-uy, ux);

    let steps = width as i32;
    for i in -(steps / 2)..=((steps - 1) / 2).max(0) {
        let off = i as f32;
        let fx = from.0 as f32 + perp_x * off;
        let fy = from.1 as f32 + perp_y * off;
        let tx = to.0 as f32 + perp_x * off;
        let ty = to.1 as f32 + perp_y * off;
        if alpha_blend {
            draw_line_alpha(img, fx, fy, tx, ty, src);
        } else {
            draw_line_segment_mut(img, (fx, fy), (tx, ty), src);
        }
    }
}

fn draw_dashed_line(
    img: &mut RgbaImage,
    from: (i32, i32),
    to: (i32, i32),
    width: u32,
    src: Rgba<u8>,
    alpha_blend: bool,
) {
    let dx = (to.0 - from.0) as f32;
    let dy = (to.1 - from.1) as f32;
    let length = (dx * dx + dy * dy).sqrt();
    if length < 1.0 {
        return;
    }
    let (ux, uy) = (dx / length, dy / length);

    let mut t = 0.0;
    while t < length {
        let seg_end = (t + DASH_LEN).min(length);
        let seg_from = (
            (from.0 as f32 + ux * t).round() as i32,
            (from.1 as f32 + uy * t).round() as i32,
        );
        let seg_to = (
            (from.0 as f32 + ux * seg_end).round() as i32,
            (from.1 as f32 + uy * seg_end).round() as i32,
        );
        draw_thick_line(img, seg_from, seg_to, width, src, alpha_blend);
        t = seg_end + GAP_LEN;
    }
}

fn draw_line_alpha(img: &mut RgbaImage, fx: f32, fy: f32, tx: f32, ty: f32, src: Rgba<u8>) {
    let dx = tx - fx;
    let dy = ty - fy;
    let steps = dx.abs().max(dy.abs()).ceil() as i32;
    if steps == 0 {
        plot(img, fx.round() as i32, fy.round() as i32, src);
        return;
    }
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        plot(
            img,
            (fx + dx * t).round() as i32,
            (fy + dy * t).round() as i32,
            src,
        );
    }
}

fn plot(img: &mut RgbaImage, x: i32, y: i32, src: Rgba<u8>) {
    if x < 0 || y < 0 {
        return;
    }
    let (x, y) = (x as u32, y as u32);
    if x >= img.width() || y >= img.height() {
        return;
    }
    blend_pixel_over(img.get_pixel_mut(x, y), src);
}

fn fill_triangle_alpha(
    img: &mut RgbaImage,
    a: ImgPoint<i32>,
    b: ImgPoint<i32>,
    c: ImgPoint<i32>,
    src: Rgba<u8>,
) {
    let xs = [a.x, b.x, c.x];
    let ys = [a.y, b.y, c.y];
    let min_x = *xs.iter().min().unwrap_or(&0);
    let max_x = *xs.iter().max().unwrap_or(&0);
    let min_y = *ys.iter().min().unwrap_or(&0);
    let max_y = *ys.iter().max().unwrap_or(&0);
    let img_w = img.width() as i32;
    let img_h = img.height() as i32;
    for y in min_y.max(0)..=max_y.min(img_h - 1) {
        for x in min_x.max(0)..=max_x.min(img_w - 1) {
            if point_in_triangle((x, y), a, b, c) {
                plot(img, x, y, src);
            }
        }
    }
}

fn point_in_triangle(p: (i32, i32), a: ImgPoint<i32>, b: ImgPoint<i32>, c: ImgPoint<i32>) -> bool {
    let sign = |p: (i32, i32), q: (i32, i32), r: (i32, i32)| {
        (p.0 - r.0) * (q.1 - r.1) - (q.0 - r.0) * (p.1 - r.1)
    };
    let d1 = sign(p, (a.x, a.y), (b.x, b.y));
    let d2 = sign(p, (b.x, b.y), (c.x, c.y));
    let d3 = sign(p, (c.x, c.y), (a.x, a.y));
    let neg = d1 < 0 || d2 < 0 || d3 < 0;
    let pos = d1 > 0 || d2 > 0 || d3 > 0;
    !(neg && pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]))
    }

    fn count_red(img: &RgbaImage) -> usize {
        img.pixels()
            .filter(|p| p.0[0] > 200 && p.0[1] < 100 && p.0[2] < 100)
            .count()
    }

    #[test]
    fn solid_arrow_paints_red_along_line() {
        let mut img = fresh(100, 100);
        draw_arrow(
            &mut img,
            (10, 50),
            (90, 50),
            ColorRgba::rgba(255, 0, 0, 255),
            2,
            12,
            ArrowStyle::Solid,
        );
        assert!(
            count_red(&img) > 50,
            "expected many red pixels along solid line"
        );
    }

    #[test]
    fn dashed_arrow_has_fewer_red_than_solid() {
        let mut s = fresh(100, 100);
        let mut d = fresh(100, 100);
        draw_arrow(
            &mut s,
            (10, 50),
            (90, 50),
            ColorRgba::rgba(255, 0, 0, 255),
            1,
            12,
            ArrowStyle::Solid,
        );
        draw_arrow(
            &mut d,
            (10, 50),
            (90, 50),
            ColorRgba::rgba(255, 0, 0, 255),
            1,
            12,
            ArrowStyle::Dashed,
        );
        let cs = count_red(&s);
        let cd = count_red(&d);
        assert!(
            cd < cs,
            "dashed should paint fewer red pixels: solid={cs}, dashed={cd}"
        );
    }

    #[test]
    fn arrow_head_paints_red_at_tip() {
        let mut img = fresh(100, 100);
        draw_arrow(
            &mut img,
            (10, 50),
            (90, 50),
            ColorRgba::rgba(255, 0, 0, 255),
            1,
            12,
            ArrowStyle::Solid,
        );
        // Pixel at the tip should be red.
        assert_eq!(img.get_pixel(89, 50).0[0], 255);
    }

    #[test]
    fn alpha_arrow_blends_with_background() {
        let mut img = fresh(100, 100);
        draw_arrow(
            &mut img,
            (10, 50),
            (90, 50),
            ColorRgba::rgba(255, 0, 0, 128),
            2,
            12,
            ArrowStyle::Solid,
        );
        // A pixel on the line should have R high, G/B reduced (blend toward red over white).
        let p = img.get_pixel(50, 50).0;
        assert_eq!(p[0], 255);
        assert!(
            p[1] < 200 && p[2] < 200,
            "expected alpha-blended pixel: {p:?}"
        );
    }

    #[test]
    fn zero_length_arrow_no_panic() {
        let mut img = fresh(50, 50);
        draw_arrow(
            &mut img,
            (10, 10),
            (10, 10),
            ColorRgba::rgba(255, 0, 0, 255),
            1,
            12,
            ArrowStyle::Solid,
        );
    }
}
