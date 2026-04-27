use clap::Args;
use image::{DynamicImage, Rgba, RgbaImage};
use imageproc::drawing::draw_hollow_rect_mut;
use imageproc::rect::Rect;
use serde::Serialize;

use imgctl_core::{ColorRgba, Error, InputSource, OutputSink, Region, Result, Size};

use crate::drawing::alpha::blend_pixel_over;
use crate::format::ImageFormat;
use crate::{decode, encode};

#[derive(Args, Debug, Clone)]
pub struct RectArgs {
    /// Input file path, or `-` for stdin
    #[arg(short, long)]
    pub input: String,

    /// Output file path, or `-` for stdout
    #[arg(short, long)]
    pub output: String,

    /// Origin X (negative values count from the right edge)
    #[arg(long, allow_hyphen_values = true)]
    pub x: i32,

    /// Origin Y (negative values count from the bottom edge)
    #[arg(long, allow_hyphen_values = true)]
    pub y: i32,

    /// Rectangle width
    #[arg(long)]
    pub w: u32,

    /// Rectangle height
    #[arg(long)]
    pub h: u32,

    /// Stroke color (e.g. #FF0000 or #FF0000FF)
    #[arg(long)]
    pub color: String,

    /// Stroke width in pixels (rendered as concentric outlines)
    #[arg(long, default_value_t = 1)]
    pub width: u32,

    /// Optional fill color (e.g. #00FF0040 for translucent green)
    #[arg(long)]
    pub fill: Option<String>,

    #[arg(long, default_value_t = 85)]
    pub quality: u8,

    #[arg(long, value_enum)]
    pub format: Option<ImageFormat>,
}

#[derive(Debug, Serialize)]
pub struct RectOutput {
    pub output: String,
    pub width: u32,
    pub height: u32,
    pub format: &'static str,
    pub size_bytes: u64,
}

pub fn run(args: RectArgs) -> Result<RectOutput> {
    let input = InputSource::from_arg(&args.input);
    let sink = OutputSink::from_arg(&args.output);
    let decoded = decode::load(&input)?;
    let mut img = decoded.image.to_rgba8();

    let region = Region {
        x: args.x,
        y: args.y,
        w: args.w,
        h: args.h,
    };
    let resolved = region.resolve(Size {
        w: img.width(),
        h: img.height(),
    })?;

    if let Some(fill_str) = &args.fill {
        let fill_color = ColorRgba::parse(fill_str)?;
        draw_filled_alpha(&mut img, resolved, fill_color);
    }

    let stroke_color = ColorRgba::parse(&args.color)?;
    draw_stroke(&mut img, resolved, args.width.max(1), stroke_color);

    let dyn_img = DynamicImage::ImageRgba8(img);

    let target_fmt = if let Some(f) = args.format {
        f
    } else {
        match &sink {
            OutputSink::File(p) => ImageFormat::from_path(p).ok_or_else(|| {
                Error::UnsupportedFormat(format!(
                    "cannot infer format from output path: {}",
                    p.display()
                ))
            })?,
            OutputSink::Stdio => return Err(Error::FormatRequired),
        }
    };

    let info = encode::write(&dyn_img, target_fmt, args.quality, &sink)?;

    Ok(RectOutput {
        output: args.output,
        width: info.width,
        height: info.height,
        format: info.format,
        size_bytes: info.size_bytes,
    })
}

/// Draw a rectangle (optional fill + stroke) for batch / annotate use.
pub fn draw(
    img: &mut RgbaImage,
    region: Region,
    stroke_color: ColorRgba,
    stroke_width: u32,
    fill: Option<ColorRgba>,
) {
    if let Some(fc) = fill {
        draw_filled_alpha(img, region, fc);
    }
    draw_stroke(img, region, stroke_width.max(1), stroke_color);
}

/// Fill a rectangle with alpha-aware blending.
fn draw_filled_alpha(img: &mut RgbaImage, r: Region, color: ColorRgba) {
    let src = Rgba(color.0);
    let x0 = r.x as u32;
    let y0 = r.y as u32;
    let x1 = (x0.saturating_add(r.w)).min(img.width());
    let y1 = (y0.saturating_add(r.h)).min(img.height());
    for y in y0..y1 {
        for x in x0..x1 {
            blend_pixel_over(img.get_pixel_mut(x, y), src);
        }
    }
}

/// Stroke a rectangle outline with given thickness, alpha-aware.
fn draw_stroke(img: &mut RgbaImage, r: Region, stroke_width: u32, color: ColorRgba) {
    let src = Rgba(color.0);
    if color.a() == 255 {
        for i in 0..stroke_width {
            let new_w = r.w.saturating_sub(2 * i);
            let new_h = r.h.saturating_sub(2 * i);
            if new_w == 0 || new_h == 0 {
                break;
            }
            let rect = Rect::at(r.x + i as i32, r.y + i as i32).of_size(new_w, new_h);
            draw_hollow_rect_mut(img, rect, src);
        }
    } else {
        for i in 0..stroke_width {
            let new_w = r.w.saturating_sub(2 * i);
            let new_h = r.h.saturating_sub(2 * i);
            if new_w == 0 || new_h == 0 {
                break;
            }
            draw_outline_alpha(
                img,
                Region {
                    x: r.x + i as i32,
                    y: r.y + i as i32,
                    w: new_w,
                    h: new_h,
                },
                src,
            );
        }
    }
}

fn draw_outline_alpha(img: &mut RgbaImage, r: Region, src: Rgba<u8>) {
    let img_w = img.width();
    let img_h = img.height();
    let x0 = r.x as u32;
    let y0 = r.y as u32;
    if r.w == 0 || r.h == 0 || x0 >= img_w || y0 >= img_h {
        return;
    }
    let x1 = (x0 + r.w - 1).min(img_w - 1);
    let y1 = (y0 + r.h - 1).min(img_h - 1);

    for x in x0..=x1 {
        blend_pixel_over(img.get_pixel_mut(x, y0), src);
        if y1 != y0 {
            blend_pixel_over(img.get_pixel_mut(x, y1), src);
        }
    }
    if y1 > y0 + 1 {
        for y in (y0 + 1)..y1 {
            blend_pixel_over(img.get_pixel_mut(x0, y), src);
            if x1 != x0 {
                blend_pixel_over(img.get_pixel_mut(x1, y), src);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::SystemTime;

    use image::{DynamicImage, Rgba, RgbaImage};

    fn unique_temp(suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "imgctl-rect-{}-{nanos}-{suffix}",
            std::process::id()
        ))
    }

    fn write_solid_png(path: &PathBuf, w: u32, h: u32, fill: [u8; 4]) {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, Rgba(fill)));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(path, &buf).unwrap();
    }

    fn pixel_at(path: &PathBuf, x: u32, y: u32) -> [u8; 4] {
        let img = image::open(path).unwrap().to_rgba8();
        img.get_pixel(x, y).0
    }

    fn args(input: &PathBuf, output: &PathBuf, color: &str, fill: Option<&str>) -> RectArgs {
        RectArgs {
            input: input.to_string_lossy().into_owned(),
            output: output.to_string_lossy().into_owned(),
            x: 10,
            y: 10,
            w: 50,
            h: 50,
            color: color.into(),
            width: 1,
            fill: fill.map(String::from),
            quality: 85,
            format: None,
        }
    }

    #[test]
    fn rect_stroke_opaque_marks_corner() {
        let input = unique_temp("in.png");
        let output = unique_temp("out.png");
        write_solid_png(&input, 100, 100, [255, 255, 255, 255]); // white bg

        let _ = run(args(&input, &output, "#FF0000", None)).unwrap();
        // Top-left corner of the 1-px stroke should now be red.
        assert_eq!(pixel_at(&output, 10, 10), [255, 0, 0, 255]);
        // A point inside the rectangle but not on the stroke should remain white.
        assert_eq!(pixel_at(&output, 30, 30), [255, 255, 255, 255]);

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
    }

    #[test]
    fn rect_fill_translucent_blends() {
        let input = unique_temp("in.png");
        let output = unique_temp("out.png");
        write_solid_png(&input, 100, 100, [255, 255, 255, 255]); // white bg

        // Red stroke + 50% red fill — interior should be ≈ pinkish red.
        let _ = run(args(&input, &output, "#FF0000", Some("#FF000080"))).unwrap();
        let p = pixel_at(&output, 30, 30);
        // R ≈ 255 (full red mixed onto 255 → 255), G ≈ 127 (half), B ≈ 127 (half)
        assert_eq!(p[0], 255);
        assert!(
            p[1] < 200,
            "green component should be reduced from 255: {p:?}"
        );
        assert!(p[2] < 200);

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
    }

    #[test]
    fn rect_invalid_color_errs() {
        let input = unique_temp("in.png");
        let output = unique_temp("out.png");
        write_solid_png(&input, 100, 100, [255, 255, 255, 255]);

        let mut a = args(&input, &output, "not-a-color", None);
        a.color = "not-a-color".into();
        let err = run(a).unwrap_err();
        assert_eq!(err.code(), "INVALID_ARGUMENT");

        let _ = std::fs::remove_file(&input);
    }

    #[test]
    fn rect_zero_area_errs() {
        let input = unique_temp("in.png");
        let output = unique_temp("out.png");
        write_solid_png(&input, 100, 100, [255, 255, 255, 255]);

        let mut a = args(&input, &output, "#FF0000", None);
        a.x = 200;
        a.y = 200;
        let err = run(a).unwrap_err();
        assert_eq!(err.code(), "INVALID_ARGUMENT");

        let _ = std::fs::remove_file(&input);
    }

    #[test]
    fn rect_stroke_width_3_renders_thick_outline() {
        let input = unique_temp("in.png");
        let output = unique_temp("out.png");
        write_solid_png(&input, 100, 100, [255, 255, 255, 255]);

        let mut a = args(&input, &output, "#FF0000", None);
        a.width = 3;
        let _ = run(a).unwrap();
        // Corner pixel and inset by 2 should both be red.
        assert_eq!(pixel_at(&output, 10, 10), [255, 0, 0, 255]);
        assert_eq!(pixel_at(&output, 12, 12), [255, 0, 0, 255]);
        // Inset by 3 should be white again.
        assert_eq!(pixel_at(&output, 13, 13), [255, 255, 255, 255]);

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
    }
}
