use clap::{Args, ValueEnum};
use serde::Serialize;

use imgctl_core::{Error, InputSource, OutputSink, Result};

use crate::render::{MermaidTheme, render_svg};
use crate::svg_to_png;

#[derive(ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[clap(rename_all = "lowercase")]
pub enum MermaidFormat {
    #[default]
    Png,
    Svg,
}

#[derive(Args, Debug, Clone)]
pub struct MermaidArgs {
    /// Input .mmd file path, or `-` for stdin
    #[arg(short, long)]
    pub input: String,

    /// Output file path, or `-` for stdout
    #[arg(short, long)]
    pub output: String,

    /// Output format
    #[arg(long, value_enum, default_value_t = MermaidFormat::Png)]
    pub format: MermaidFormat,

    /// CDP WebSocket URL for an already-running Chrome (e.g. ws://localhost:9222);
    /// omit to launch a managed Chromium
    #[arg(long)]
    pub chrome: Option<String>,

    /// Mermaid theme
    #[arg(long, value_enum, default_value_t = MermaidTheme::Default)]
    pub theme: MermaidTheme,

    /// Target PNG width in pixels (ignored for --format svg)
    #[arg(long, default_value_t = 1200)]
    pub width: u32,
}

#[derive(Debug, Serialize)]
pub struct MermaidOutput {
    pub output: String,
    pub format: &'static str,
    pub size_bytes: u64,
}

pub fn run(args: MermaidArgs) -> Result<MermaidOutput> {
    let src_bytes = InputSource::from_arg(&args.input).read_all()?;
    let src = std::str::from_utf8(&src_bytes)
        .map_err(|e| Error::InvalidArgument(format!("mermaid source not UTF-8: {e}")))?
        .to_string();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| Error::Internal(format!("tokio runtime: {e}")))?;

    let svg = runtime.block_on(render_svg(&src, args.theme, args.chrome.as_deref()))?;

    let (bytes, format_str): (Vec<u8>, &'static str) = match args.format {
        MermaidFormat::Svg => (svg.into_bytes(), "svg"),
        MermaidFormat::Png => (svg_to_png::render(&svg, Some(args.width))?, "png"),
    };

    let size_bytes = bytes.len() as u64;
    OutputSink::from_arg(&args.output).write_all(&bytes)?;

    Ok(MermaidOutput {
        output: args.output,
        format: format_str,
        size_bytes,
    })
}
