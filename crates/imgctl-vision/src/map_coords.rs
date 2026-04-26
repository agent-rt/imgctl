use clap::Args;
use serde::Serialize;

use imgctl_core::{Error, Result};

fn parse_size(s: &str) -> std::result::Result<(u32, u32), String> {
    let parts: Vec<&str> = s.split(['x', 'X']).collect();
    if parts.len() != 2 {
        return Err(format!("expected WxH, got: {s}"));
    }
    let w = parts[0].trim().parse::<u32>().map_err(|e| format!("w: {e}"))?;
    let h = parts[1].trim().parse::<u32>().map_err(|e| format!("h: {e}"))?;
    Ok((w, h))
}

fn parse_point(s: &str) -> std::result::Result<(i32, i32), String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return Err(format!("expected X,Y, got: {s}"));
    }
    let x = parts[0].trim().parse::<i32>().map_err(|e| format!("x: {e}"))?;
    let y = parts[1].trim().parse::<i32>().map_err(|e| format!("y: {e}"))?;
    Ok((x, y))
}

#[derive(Args, Debug, Clone)]
pub struct MapCoordsArgs {
    /// Source size as "WxH"
    #[arg(long, value_parser = parse_size)]
    pub from_size: (u32, u32),

    /// Target size as "WxH"
    #[arg(long, value_parser = parse_size)]
    pub to_size: (u32, u32),

    /// Point in source coordinates as "X,Y"
    #[arg(long, value_parser = parse_point, allow_hyphen_values = true)]
    pub point: (i32, i32),
}

#[derive(Debug, Serialize)]
pub struct PointPair {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Serialize)]
pub struct MapCoordsOutput {
    pub input: PointPair,
    pub output: PointPair,
    pub scale_x: f64,
    pub scale_y: f64,
}

pub fn run(args: MapCoordsArgs) -> Result<MapCoordsOutput> {
    let (from_w, from_h) = args.from_size;
    let (to_w, to_h) = args.to_size;
    if from_w == 0 || from_h == 0 {
        return Err(Error::InvalidArgument("--from-size must be non-zero".into()));
    }
    let scale_x = f64::from(to_w) / f64::from(from_w);
    let scale_y = f64::from(to_h) / f64::from(from_h);
    let (px, py) = args.point;
    let out_x = (f64::from(px) * scale_x).round() as i32;
    let out_y = (f64::from(py) * scale_y).round() as i32;
    Ok(MapCoordsOutput {
        input: PointPair { x: px, y: py },
        output: PointPair { x: out_x, y: out_y },
        scale_x,
        scale_y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_size_ok() {
        assert_eq!(parse_size("1280x720").unwrap(), (1280, 720));
        assert_eq!(parse_size("100X200").unwrap(), (100, 200));
    }

    #[test]
    fn parse_size_invalid() {
        assert!(parse_size("100").is_err());
        assert!(parse_size("100x200x300").is_err());
        assert!(parse_size("ax2").is_err());
    }

    #[test]
    fn upscale_2x() {
        let out = run(MapCoordsArgs {
            from_size: (1280, 720),
            to_size: (2560, 1440),
            point: (640, 360),
        })
        .unwrap();
        assert_eq!(out.scale_x, 2.0);
        assert_eq!(out.scale_y, 2.0);
        assert_eq!(out.output.x, 1280);
        assert_eq!(out.output.y, 720);
    }

    #[test]
    fn downscale_half() {
        let out = run(MapCoordsArgs {
            from_size: (200, 200),
            to_size: (100, 100),
            point: (40, 80),
        })
        .unwrap();
        assert_eq!(out.scale_x, 0.5);
        assert_eq!(out.output.x, 20);
        assert_eq!(out.output.y, 40);
    }

    #[test]
    fn zero_from_size_errs() {
        let err = run(MapCoordsArgs {
            from_size: (0, 100),
            to_size: (100, 100),
            point: (0, 0),
        })
        .unwrap_err();
        assert_eq!(err.code(), "INVALID_ARGUMENT");
    }
}
