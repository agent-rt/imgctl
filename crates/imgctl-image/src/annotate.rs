use clap::Args;
use image::{DynamicImage, RgbaImage};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use imgctl_core::{ColorRgba, Error, InputSource, OutputSink, Region, Result, Size};

use crate::format::ImageFormat;
use crate::{blur, decode, drawing, encode, rect};

#[derive(Args, Debug, Clone)]
pub struct AnnotateArgs {
    #[arg(short, long)]
    pub input: Option<String>,

    #[arg(short, long)]
    pub output: Option<String>,

    /// JSON config file with batch operations (or `-` for stdin)
    #[arg(long)]
    pub config: Option<String>,

    /// Print the JSON schema for the config file and exit
    #[arg(long)]
    pub print_schema: bool,

    #[arg(long, default_value_t = 85)]
    pub quality: u8,

    #[arg(long, value_enum)]
    pub format: Option<ImageFormat>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OperationsFile {
    pub operations: Vec<Operation>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Operation {
    Text {
        text: String,
        x: i32,
        y: i32,
        #[serde(default = "default_text_size")]
        size: u32,
        #[serde(default = "default_black")]
        color: String,
        #[serde(default)]
        align: TextAlignOp,
        #[serde(default)]
        bg: Option<String>,
        #[serde(default)]
        font: Option<String>,
    },
    Arrow {
        from: [i32; 2],
        to: [i32; 2],
        #[serde(default = "default_red")]
        color: String,
        #[serde(default = "default_arrow_width")]
        width: u32,
        #[serde(default = "default_arrow_head")]
        head_size: u32,
        #[serde(default)]
        style: ArrowStyleOp,
    },
    Blur {
        region: [i32; 4],
        #[serde(default = "default_sigma")]
        sigma: f32,
        #[serde(default)]
        kind: BlurKindOp,
    },
    Rect {
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        #[serde(default = "default_red")]
        color: String,
        #[serde(default = "default_one")]
        width: u32,
        #[serde(default)]
        fill: Option<String>,
    },
}

#[derive(Debug, Deserialize, Default, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TextAlignOp {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Deserialize, Default, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ArrowStyleOp {
    #[default]
    Solid,
    Dashed,
}

#[derive(Debug, Deserialize, Default, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BlurKindOp {
    #[default]
    Gaussian,
    Pixelate,
}

fn default_text_size() -> u32 { 24 }
fn default_black() -> String { "#000000".into() }
fn default_red() -> String { "#FF0000".into() }
fn default_arrow_width() -> u32 { 2 }
fn default_arrow_head() -> u32 { 12 }
fn default_one() -> u32 { 1 }
fn default_sigma() -> f32 { 8.0 }

#[derive(Debug, Serialize)]
pub struct AnnotateOutput {
    pub output: String,
    pub width: u32,
    pub height: u32,
    pub format: &'static str,
    pub size_bytes: u64,
    pub operations: usize,
}

/// Print the OperationsFile JSON schema to stdout. Bypasses the regular
/// Response/format pipeline — meant for `--print-schema`.
pub fn print_schema() -> Result<()> {
    let schema = schemars::schema_for!(OperationsFile);
    let json = serde_json::to_string_pretty(&schema)
        .map_err(|e| Error::Serialization(e.to_string()))?;
    println!("{json}");
    Ok(())
}

pub fn run(args: AnnotateArgs) -> Result<AnnotateOutput> {
    let input_path = args.input.ok_or_else(|| Error::InvalidArgument(
        "annotate --input is required (use --print-schema to dump schema)".into(),
    ))?;
    let output_path = args.output.ok_or_else(|| Error::InvalidArgument(
        "annotate --output is required".into(),
    ))?;
    let config_path = args.config.ok_or_else(|| Error::InvalidArgument(
        "annotate --config is required".into(),
    ))?;

    let config_bytes = if config_path == "-" {
        InputSource::Stdio.read_all()?
    } else {
        std::fs::read(&config_path)?
    };
    let parsed: OperationsFile = serde_json::from_slice(&config_bytes)
        .map_err(|e| Error::InvalidArgument(format!("config parse: {e}")))?;

    let input = InputSource::from_arg(&input_path);
    let sink = OutputSink::from_arg(&output_path);
    let decoded = decode::load(&input)?;
    let mut img = decoded.image.to_rgba8();

    let ops_count = parsed.operations.len();
    for (idx, op) in parsed.operations.iter().enumerate() {
        apply_operation(&mut img, op).map_err(|e| {
            Error::InvalidArgument(format!("op[{idx}]: {e}"))
        })?;
    }

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

    Ok(AnnotateOutput {
        output: output_path,
        width: info.width,
        height: info.height,
        format: info.format,
        size_bytes: info.size_bytes,
        operations: ops_count,
    })
}

fn apply_operation(img: &mut RgbaImage, op: &Operation) -> Result<()> {
    match op {
        Operation::Text { text, x, y, size, color, align, bg, font } => {
            let color = ColorRgba::parse(color)?;
            let bg = bg.as_deref().map(ColorRgba::parse).transpose()?;
            let font_obj = drawing::text::load_font(font.as_deref())?;
            let internal_align = match align {
                TextAlignOp::Left => drawing::text::TextAlign::Left,
                TextAlignOp::Center => drawing::text::TextAlign::Center,
                TextAlignOp::Right => drawing::text::TextAlign::Right,
            };
            drawing::text::render_text(img, text, *x, *y, *size, color, internal_align, &font_obj, bg)
        }
        Operation::Arrow { from, to, color, width, head_size, style } => {
            let color = ColorRgba::parse(color)?;
            let internal_style = match style {
                ArrowStyleOp::Solid => drawing::arrow::ArrowStyle::Solid,
                ArrowStyleOp::Dashed => drawing::arrow::ArrowStyle::Dashed,
            };
            drawing::arrow::draw_arrow(
                img,
                (from[0], from[1]),
                (to[0], to[1]),
                color,
                *width,
                *head_size,
                internal_style,
            );
            Ok(())
        }
        Operation::Blur { region, sigma, kind } => {
            if region[2] < 0 || region[3] < 0 {
                return Err(Error::InvalidArgument(
                    format!("blur region w/h must be non-negative: {region:?}"),
                ));
            }
            let r = Region {
                x: region[0],
                y: region[1],
                w: region[2] as u32,
                h: region[3] as u32,
            };
            let resolved = r.resolve(Size { w: img.width(), h: img.height() })?;
            let internal_kind = match kind {
                BlurKindOp::Gaussian => blur::BlurType::Gaussian,
                BlurKindOp::Pixelate => blur::BlurType::Pixelate,
            };
            blur::process_region(img, resolved, *sigma, internal_kind);
            Ok(())
        }
        Operation::Rect { x, y, w, h, color, width, fill } => {
            let stroke_color = ColorRgba::parse(color)?;
            let fill_color = fill.as_deref().map(ColorRgba::parse).transpose()?;
            let r = Region { x: *x, y: *y, w: *w, h: *h };
            let resolved = r.resolve(Size { w: img.width(), h: img.height() })?;
            rect::draw(img, resolved, stroke_color, *width, fill_color);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ops_text_arrow_blur_rect() {
        let json = r#"{
            "operations": [
                { "type": "text", "text": "hi", "x": 10, "y": 10 },
                { "type": "arrow", "from": [0, 0], "to": [50, 50] },
                { "type": "blur", "region": [10, 10, 30, 30] },
                { "type": "rect", "x": 5, "y": 5, "w": 20, "h": 20 }
            ]
        }"#;
        let parsed: OperationsFile = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.operations.len(), 4);
    }

    #[test]
    fn parse_ops_unknown_type_errs() {
        let json = r#"{ "operations": [{ "type": "foo", "x": 0 }] }"#;
        let err = serde_json::from_str::<OperationsFile>(json).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("foo") || msg.contains("variant"), "{msg}");
    }

    #[test]
    fn schema_includes_operations_field() {
        let schema = schemars::schema_for!(OperationsFile);
        let json = serde_json::to_string(&schema).unwrap();
        assert!(json.contains("operations"), "schema: {json}");
        assert!(json.contains("text") || json.contains("Text"));
        assert!(json.contains("arrow") || json.contains("Arrow"));
    }

    #[test]
    fn apply_text_op_marks_red_pixels() {
        let mut img = RgbaImage::from_pixel(100, 100, image::Rgba([255, 255, 255, 255]));
        let op = Operation::Text {
            text: "X".into(),
            x: 10,
            y: 10,
            size: 32,
            color: "#FF0000".into(),
            align: TextAlignOp::Left,
            bg: None,
            font: None,
        };
        apply_operation(&mut img, &op).unwrap();
        let red_count = img
            .pixels()
            .filter(|p| p.0[0] > 100 && p.0[1] < 200 && p.0[2] < 200)
            .count();
        assert!(red_count > 5, "expected red text pixels");
    }

    #[test]
    fn apply_rect_op_with_fill() {
        let mut img = RgbaImage::from_pixel(100, 100, image::Rgba([255, 255, 255, 255]));
        let op = Operation::Rect {
            x: 10,
            y: 10,
            w: 50,
            h: 50,
            color: "#00FF00".into(),
            width: 1,
            fill: Some("#FF000080".into()),
        };
        apply_operation(&mut img, &op).unwrap();
        // Interior pixel should have R high (red tint over white).
        let p = img.get_pixel(30, 30).0;
        assert_eq!(p[0], 255);
        assert!(p[1] < 200);
    }

    #[test]
    fn invalid_color_in_op_errs() {
        let mut img = RgbaImage::from_pixel(50, 50, image::Rgba([255; 4]));
        let op = Operation::Rect {
            x: 0, y: 0, w: 10, h: 10,
            color: "not-a-color".into(),
            width: 1,
            fill: None,
        };
        let err = apply_operation(&mut img, &op).unwrap_err();
        assert_eq!(err.code(), "INVALID_ARGUMENT");
    }
}
