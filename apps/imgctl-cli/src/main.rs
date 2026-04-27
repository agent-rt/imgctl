mod cli;

use std::io::Write;
use std::process::ExitCode;
use std::time::Instant;

use clap::Parser;
use serde::Serialize;

use cli::{Cli, Command};
use imgctl_core::{Error, NoData, OutputFormat, Response};

fn main() -> ExitCode {
    let cli = Cli::parse();

    let format = match (cli.quiet, cli.json) {
        (true, _) => OutputFormat::Quiet,
        (_, true) => OutputFormat::Json,
        _ => OutputFormat::Tsv,
    };

    let started = Instant::now();
    let meta_to_stderr = cli.command.output_arg() == Some("-");
    let success = dispatch(cli.command, format, started, meta_to_stderr);

    if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

fn dispatch(cmd: Command, format: OutputFormat, started: Instant, meta_to_stderr: bool) -> bool {
    match cmd {
        Command::Annotate(args) if args.print_schema => {
            // Bypass Response wrapping; print raw JSON schema.
            imgctl_image::annotate::print_schema().is_ok()
        }
        Command::Convert(args) => emit(
            format,
            imgctl_image::convert::run(args),
            started,
            meta_to_stderr,
        ),
        Command::Resize(args) => emit(
            format,
            imgctl_image::resize::run(args),
            started,
            meta_to_stderr,
        ),
        Command::Crop(args) => emit(
            format,
            imgctl_image::crop::run(args),
            started,
            meta_to_stderr,
        ),
        Command::Rect(args) => emit(
            format,
            imgctl_image::rect::run(args),
            started,
            meta_to_stderr,
        ),
        Command::Text(args) => emit(
            format,
            imgctl_image::text::run(args),
            started,
            meta_to_stderr,
        ),
        Command::Arrow(args) => emit(
            format,
            imgctl_image::arrow::run(args),
            started,
            meta_to_stderr,
        ),
        Command::Blur(args) => emit(
            format,
            imgctl_image::blur::run(args),
            started,
            meta_to_stderr,
        ),
        Command::Concat(args) => emit(
            format,
            imgctl_image::concat::run(args),
            started,
            meta_to_stderr,
        ),
        Command::Annotate(args) => emit(
            format,
            imgctl_image::annotate::run(args),
            started,
            meta_to_stderr,
        ),
        Command::Slice(args) => emit(
            format,
            imgctl_vision::slice::run(args),
            started,
            meta_to_stderr,
        ),
        Command::MapCoords(args) => emit(
            format,
            imgctl_vision::map_coords::run(args),
            started,
            meta_to_stderr,
        ),
        Command::Info(args) => emit(
            format,
            imgctl_vision::info::run(args),
            started,
            meta_to_stderr,
        ),
        Command::Diff(args) => emit(
            format,
            imgctl_vision::diff::run(args),
            started,
            meta_to_stderr,
        ),
        Command::Hash(args) => emit(
            format,
            imgctl_vision::hash::run(args),
            started,
            meta_to_stderr,
        ),
        Command::Fix(args) => emit(
            format,
            imgctl_vision::fix::run(args),
            started,
            meta_to_stderr,
        ),
        #[cfg(feature = "mermaid")]
        Command::Mermaid(args) => emit(format, imgctl_mermaid::run(args), started, meta_to_stderr),
        #[allow(unreachable_patterns)]
        _ => emit::<NoData>(
            format,
            Err(Error::Internal("command not implemented yet".into())),
            started,
            meta_to_stderr,
        ),
    }
}

fn emit<T: Serialize>(
    format: OutputFormat,
    result: imgctl_core::Result<T>,
    started: Instant,
    meta_to_stderr: bool,
) -> bool {
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let success = result.is_ok();

    let mut writer: Box<dyn Write> = if meta_to_stderr {
        Box::new(std::io::stderr())
    } else {
        Box::new(std::io::stdout())
    };

    let _ = match result {
        Ok(data) => format.write(&mut writer, &Response::ok(data, elapsed_ms)),
        Err(e) => format.write(&mut writer, &Response::<()>::from_error(&e, elapsed_ms)),
    };
    let _ = writer.flush();

    success
}
