use std::collections::VecDeque;

use clap::Args;
use image::{DynamicImage, Rgba, RgbaImage};
use serde::Serialize;

use imgctl_core::{Error, InputSource, OutputSink, Result};

#[derive(Args, Debug, Clone)]
pub struct DiffArgs {
    /// First image
    #[arg(short = 'a', long)]
    pub a: String,

    /// Second image
    #[arg(short = 'b', long)]
    pub b: String,

    /// Diff visualization output (PNG); omit to skip writing
    #[arg(short, long)]
    pub output: Option<String>,

    /// Change ratio threshold below which `changed` is false
    #[arg(long, default_value_t = 0.02)]
    pub threshold: f64,

    /// Per-pixel RGB euclidean distance threshold for "different"
    #[arg(long, default_value_t = 10.0)]
    pub epsilon: f64,
}

#[derive(Debug, Serialize)]
pub struct RegionOut {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Serialize)]
pub struct DiffOutput {
    pub changed: bool,
    pub change_ratio: f64,
    pub changed_regions: Vec<RegionOut>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

pub fn run(args: DiffArgs) -> Result<DiffOutput> {
    let a_bytes = InputSource::from_arg(&args.a).read_all()?;
    let b_bytes = InputSource::from_arg(&args.b).read_all()?;

    let a_img = image::load_from_memory(&a_bytes).map_err(|e| Error::Image(e.to_string()))?;
    let b_img = image::load_from_memory(&b_bytes).map_err(|e| Error::Image(e.to_string()))?;

    if a_img.width() != b_img.width() || a_img.height() != b_img.height() {
        return Err(Error::InvalidArgument(format!(
            "diff inputs must have matching dimensions: a={}x{}, b={}x{}",
            a_img.width(),
            a_img.height(),
            b_img.width(),
            b_img.height(),
        )));
    }

    let a_rgba = a_img.to_rgba8();
    let b_rgba = b_img.to_rgba8();
    let (w, h) = (a_rgba.width(), a_rgba.height());

    let (mask, changed_count) = pixel_diff_mask(&a_rgba, &b_rgba, args.epsilon);
    let total = u64::from(w) * u64::from(h);
    let change_ratio = if total == 0 {
        0.0
    } else {
        changed_count as f64 / total as f64
    };

    if change_ratio < args.threshold {
        return Ok(DiffOutput {
            changed: false,
            change_ratio,
            changed_regions: Vec::new(),
            output: None,
        });
    }

    let regions = find_components(&mask, w, h);

    let mut written = None;
    if let Some(out_path) = args.output {
        let mut canvas = a_rgba.clone();
        let red = Rgba([255, 0, 0, 255]);
        for r in &regions {
            draw_stroke(&mut canvas, r, red);
        }
        let dyn_img = DynamicImage::ImageRgba8(canvas);
        let mut buf = Vec::new();
        dyn_img
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .map_err(|e| Error::Image(e.to_string()))?;
        OutputSink::from_arg(&out_path).write_all(&buf)?;
        written = Some(out_path);
    }

    Ok(DiffOutput {
        changed: true,
        change_ratio,
        changed_regions: regions
            .iter()
            .map(|r| RegionOut {
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
            })
            .collect(),
        output: written,
    })
}

#[derive(Debug)]
struct InternalRegion {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

fn pixel_diff_mask(a: &RgbaImage, b: &RgbaImage, epsilon: f64) -> (Vec<bool>, u64) {
    let total = (a.width() * a.height()) as usize;
    let mut mask = vec![false; total];
    let mut count = 0u64;
    let eps2 = epsilon * epsilon;
    for (i, (pa, pb)) in a.pixels().zip(b.pixels()).enumerate() {
        let dr = pa.0[0] as f64 - pb.0[0] as f64;
        let dg = pa.0[1] as f64 - pb.0[1] as f64;
        let db = pa.0[2] as f64 - pb.0[2] as f64;
        if dr * dr + dg * dg + db * db > eps2 {
            mask[i] = true;
            count += 1;
        }
    }
    (mask, count)
}

fn find_components(mask: &[bool], w: u32, h: u32) -> Vec<InternalRegion> {
    let mut visited = vec![false; mask.len()];
    let mut regions = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            if !mask[idx] || visited[idx] {
                continue;
            }
            let mut queue: VecDeque<(u32, u32)> = VecDeque::new();
            queue.push_back((x, y));
            visited[idx] = true;
            let (mut min_x, mut min_y, mut max_x, mut max_y) = (x, y, x, y);
            while let Some((cx, cy)) = queue.pop_front() {
                if cx < min_x {
                    min_x = cx;
                }
                if cy < min_y {
                    min_y = cy;
                }
                if cx > max_x {
                    max_x = cx;
                }
                if cy > max_y {
                    max_y = cy;
                }
                for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let nidx = (ny as u32 * w + nx as u32) as usize;
                    if !visited[nidx] && mask[nidx] {
                        visited[nidx] = true;
                        queue.push_back((nx as u32, ny as u32));
                    }
                }
            }
            regions.push(InternalRegion {
                x: min_x as i32,
                y: min_y as i32,
                w: max_x - min_x + 1,
                h: max_y - min_y + 1,
            });
        }
    }
    regions
}

fn draw_stroke(img: &mut RgbaImage, r: &InternalRegion, color: Rgba<u8>) {
    let img_w = img.width();
    let img_h = img.height();
    if r.w == 0 || r.h == 0 || img_w == 0 || img_h == 0 {
        return;
    }
    let x0 = r.x.max(0) as u32;
    let y0 = r.y.max(0) as u32;
    let x1 = ((r.x + r.w as i32 - 1) as u32).min(img_w - 1);
    let y1 = ((r.y + r.h as i32 - 1) as u32).min(img_h - 1);
    if x0 >= img_w || y0 >= img_h {
        return;
    }
    for x in x0..=x1 {
        img.put_pixel(x, y0, color);
        if y1 != y0 {
            img.put_pixel(x, y1, color);
        }
    }
    for y in y0..=y1 {
        img.put_pixel(x0, y, color);
        if x1 != x0 {
            img.put_pixel(x1, y, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, color: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba(color))
    }

    fn fill_rect(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, color: [u8; 4]) {
        for yy in y..(y + h) {
            for xx in x..(x + w) {
                img.put_pixel(xx, yy, Rgba(color));
            }
        }
    }

    #[test]
    fn identical_images_yield_no_change() {
        let a = solid(50, 50, [10, 20, 30, 255]);
        let b = a.clone();
        let (_mask, count) = pixel_diff_mask(&a, &b, 5.0);
        assert_eq!(count, 0);
    }

    #[test]
    fn pixel_diff_mask_counts_differences() {
        let a = solid(10, 10, [0, 0, 0, 255]);
        let mut b = a.clone();
        fill_rect(&mut b, 1, 1, 3, 3, [255, 255, 255, 255]); // 9 pixels differ
        let (_mask, count) = pixel_diff_mask(&a, &b, 5.0);
        assert_eq!(count, 9);
    }

    #[test]
    fn find_components_one_blob() {
        let w = 10;
        let h = 10;
        let mut mask = vec![false; (w * h) as usize];
        for y in 2..5 {
            for x in 3..7 {
                mask[(y * w + x) as usize] = true;
            }
        }
        let regions = find_components(&mask, w, h);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].x, 3);
        assert_eq!(regions[0].y, 2);
        assert_eq!(regions[0].w, 4);
        assert_eq!(regions[0].h, 3);
    }

    #[test]
    fn find_components_two_disjoint_blobs() {
        let w = 20;
        let h = 20;
        let mut mask = vec![false; (w * h) as usize];
        // blob 1
        for y in 0..3 {
            for x in 0..3 {
                mask[(y * w + x) as usize] = true;
            }
        }
        // blob 2
        for y in 10..13 {
            for x in 10..13 {
                mask[(y * w + x) as usize] = true;
            }
        }
        let regions = find_components(&mask, w, h);
        assert_eq!(regions.len(), 2);
    }

    #[test]
    fn run_below_threshold_no_change() {
        // 1 pixel differs out of 10000, ratio = 0.0001 << 0.02
        let a = solid(100, 100, [0, 0, 0, 255]);
        let mut b = a.clone();
        b.put_pixel(50, 50, Rgba([255, 255, 255, 255]));

        let in_a = std::env::temp_dir().join(format!("diff-a-{}.png", std::process::id()));
        let in_b = std::env::temp_dir().join(format!("diff-b-{}.png", std::process::id()));
        for (img, p) in [(&a, &in_a), (&b, &in_b)] {
            let dyn_img = DynamicImage::ImageRgba8(img.clone());
            let mut buf = Vec::new();
            dyn_img
                .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
                .unwrap();
            std::fs::write(p, &buf).unwrap();
        }

        let out = run(DiffArgs {
            a: in_a.to_string_lossy().into_owned(),
            b: in_b.to_string_lossy().into_owned(),
            output: None,
            threshold: 0.02,
            epsilon: 10.0,
        })
        .unwrap();
        assert!(!out.changed);
        assert!(out.change_ratio < 0.02);

        for p in [&in_a, &in_b] {
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn run_dimension_mismatch_errs() {
        let a = solid(10, 10, [0; 4]);
        let b = solid(20, 20, [0; 4]);
        let in_a = std::env::temp_dir().join(format!("diff-mm-a-{}.png", std::process::id()));
        let in_b = std::env::temp_dir().join(format!("diff-mm-b-{}.png", std::process::id()));
        for (img, p) in [(&a, &in_a), (&b, &in_b)] {
            let dyn_img = DynamicImage::ImageRgba8(img.clone());
            let mut buf = Vec::new();
            dyn_img
                .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
                .unwrap();
            std::fs::write(p, &buf).unwrap();
        }
        let err = run(DiffArgs {
            a: in_a.to_string_lossy().into_owned(),
            b: in_b.to_string_lossy().into_owned(),
            output: None,
            threshold: 0.02,
            epsilon: 10.0,
        })
        .unwrap_err();
        assert_eq!(err.code(), "INVALID_ARGUMENT");
        for p in [&in_a, &in_b] {
            let _ = std::fs::remove_file(p);
        }
    }
}
