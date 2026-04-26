use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "imgctl",
    version,
    about = "Agent-first image processing CLI",
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Output JSON instead of TSV
    #[arg(long, global = true)]
    pub json: bool,

    /// Suppress all output (exit code only)
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Verbose logging to stderr
    #[arg(long, global = true)]
    pub verbose: bool,

    /// Operation timeout in milliseconds
    #[arg(long, global = true, default_value_t = 30_000)]
    pub timeout: u64,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Convert image format
    Convert(imgctl_image::ConvertArgs),
    /// Resize image
    Resize(imgctl_image::ResizeArgs),
    /// Crop image
    Crop(imgctl_image::CropArgs),
    /// Draw text on image
    Text(imgctl_image::TextArgs),
    /// Draw arrow on image
    Arrow(imgctl_image::ArrowArgs),
    /// Blur region of image
    Blur(imgctl_image::BlurArgs),
    /// Draw rectangle on image
    Rect(imgctl_image::RectArgs),
    /// Concatenate multiple images
    Concat(imgctl_image::ConcatArgs),
    /// Apply batch operations from a JSON config
    Annotate(imgctl_image::AnnotateArgs),
    /// Extract image metadata
    Info(imgctl_vision::InfoArgs),
    /// Compute visual diff between two images
    Diff(imgctl_vision::DiffArgs),
    /// Compute perceptual hash
    Hash(imgctl_vision::HashArgs),
    /// Slice image into tiles
    Slice(imgctl_vision::SliceArgs),
    /// Map coordinates between dimensions
    MapCoords(imgctl_vision::MapCoordsArgs),
    /// Detect and repair format issues
    Fix(imgctl_vision::FixArgs),
    /// Render Mermaid diagram
    #[cfg(feature = "mermaid")]
    Mermaid(imgctl_mermaid::MermaidArgs),
}

impl Command {
    /// Returns the `--output` argument for commands that have one, used by main.rs
    /// to decide whether metadata should go to stderr (when output is `-` / stdout).
    pub fn output_arg(&self) -> Option<&str> {
        match self {
            Command::Convert(a) => Some(&a.output),
            Command::Resize(a) => Some(&a.output),
            Command::Crop(a) => Some(&a.output),
            Command::Rect(a) => Some(&a.output),
            Command::Text(a) => Some(&a.output),
            Command::Arrow(a) => Some(&a.output),
            Command::Blur(a) => Some(&a.output),
            Command::Concat(a) => Some(&a.output),
            Command::Annotate(a) => a.output.as_deref(),
            #[cfg(feature = "mermaid")]
            Command::Mermaid(a) => Some(&a.output),
            _ => None,
        }
    }
}
