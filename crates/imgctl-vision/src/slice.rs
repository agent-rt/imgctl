use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

use imgctl_core::{Error, InputSource, Result};

#[derive(Args, Debug, Clone)]
pub struct SliceArgs {
    /// Input file path, or `-` for stdin
    #[arg(short, long)]
    pub input: String,

    /// Number of rows
    #[arg(long)]
    pub rows: u32,

    /// Number of columns
    #[arg(long)]
    pub cols: u32,

    /// Overlap pixels between adjacent tiles (avoids cutting content)
    #[arg(long, default_value_t = 0)]
    pub overlap: u32,

    /// Output directory (created if missing); tiles named `tile_{r}_{c}.png`
    #[arg(long)]
    pub output_dir: String,
}

#[derive(Debug, Serialize)]
pub struct TileInfo {
    pub file: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Serialize)]
pub struct SliceOutput {
    pub tiles: Vec<TileInfo>,
}

pub fn run(args: SliceArgs) -> Result<SliceOutput> {
    if args.rows == 0 || args.cols == 0 {
        return Err(Error::InvalidArgument(
            "--rows and --cols must both be > 0".into(),
        ));
    }

    let input = InputSource::from_arg(&args.input);
    let bytes = input.read_all()?;
    let img = image::load_from_memory(&bytes).map_err(|e| Error::Image(e.to_string()))?;
    let (w, h) = (img.width(), img.height());

    let cols = args.cols;
    let rows = args.rows;
    let overlap = args.overlap;

    if overlap > 0 && (overlap >= w / cols || overlap >= h / rows) {
        return Err(Error::InvalidArgument(
            "--overlap must be smaller than tile size implied by rows/cols".into(),
        ));
    }

    // Ceil-div so the last tile reaches the image edge.
    let tile_w = (w + (cols - 1) * overlap + cols - 1) / cols;
    let tile_h = (h + (rows - 1) * overlap + rows - 1) / rows;

    let dir = PathBuf::from(&args.output_dir);
    std::fs::create_dir_all(&dir)?;

    let mut tiles = Vec::with_capacity((rows * cols) as usize);
    for r in 0..rows {
        for c in 0..cols {
            let x = c * tile_w.saturating_sub(overlap);
            let y = r * tile_h.saturating_sub(overlap);
            let actual_w = (x + tile_w).min(w).saturating_sub(x);
            let actual_h = (y + tile_h).min(h).saturating_sub(y);
            if actual_w == 0 || actual_h == 0 {
                continue;
            }
            let cropped = img.crop_imm(x, y, actual_w, actual_h);
            let filename = format!("tile_{r}_{c}.png");
            let path = dir.join(&filename);
            let mut buf = Vec::new();
            cropped
                .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
                .map_err(|e| Error::Image(e.to_string()))?;
            std::fs::write(&path, &buf)?;
            tiles.push(TileInfo {
                file: path.to_string_lossy().into_owned(),
                x,
                y,
                w: actual_w,
                h: actual_h,
            });
        }
    }

    Ok(SliceOutput { tiles })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn unique_dir(suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "imgctl-slice-{}-{nanos}-{suffix}",
            std::process::id()
        ))
    }

    fn write_solid_png(path: &PathBuf, w: u32, h: u32) {
        let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            w,
            h,
            image::Rgba([10, 20, 30, 255]),
        ));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(path, &buf).unwrap();
    }

    fn args(input: &PathBuf, dir: &PathBuf, rows: u32, cols: u32, overlap: u32) -> SliceArgs {
        SliceArgs {
            input: input.to_string_lossy().into_owned(),
            rows,
            cols,
            overlap,
            output_dir: dir.to_string_lossy().into_owned(),
        }
    }

    #[test]
    fn slice_2x2_no_overlap() {
        let dir = unique_dir("2x2");
        let input = dir.join("in.png");
        std::fs::create_dir_all(&dir).unwrap();
        write_solid_png(&input, 200, 200);

        let out = run(args(&input, &dir, 2, 2, 0)).unwrap();
        assert_eq!(out.tiles.len(), 4);
        for tile in &out.tiles {
            assert_eq!(tile.w, 100);
            assert_eq!(tile.h, 100);
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn slice_with_overlap() {
        let dir = unique_dir("overlap");
        let input = dir.join("in.png");
        std::fs::create_dir_all(&dir).unwrap();
        write_solid_png(&input, 200, 200);

        let out = run(args(&input, &dir, 2, 2, 10)).unwrap();
        assert_eq!(out.tiles.len(), 4);
        // Tiles are 105x105, starting at 0 and 95 → both 105 wide.
        assert_eq!(out.tiles[0].w, 105);
        assert_eq!(out.tiles[0].h, 105);
        assert_eq!(out.tiles[1].x, 95);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn slice_zero_rows_errs() {
        let dir = unique_dir("zero");
        let input = dir.join("in.png");
        std::fs::create_dir_all(&dir).unwrap();
        write_solid_png(&input, 100, 100);
        let err = run(args(&input, &dir, 0, 1, 0)).unwrap_err();
        assert_eq!(err.code(), "INVALID_ARGUMENT");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
