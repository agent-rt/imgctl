# imgctl

Agent-first image processing CLI built in Rust. Single static binary, JSON/TSV-structured output, 16 commands for editing, visual analysis, and diagram generation.

## Why

Existing image tools (ImageMagick, ffmpeg, …) weren't built for AI agents — complex flags, inconsistent output, no structured responses. `imgctl` is opinionated for programmatic consumers:

- **Structured output by default** — every command emits TSV, or JSON with `--json`.
- **Stable error codes** — agents can pattern-match on `error.code` (`UNSUPPORTED_FORMAT`, `IO_ERROR`, `CHROME_TIMEOUT`, …) rather than parsing prose.
- **Single static binary** — no Node/Python/ImageMagick installs.
- **Dual-channel I/O** — when output is `-` (stdout), binary data goes to stdout and TSV/JSON metadata goes to stderr automatically.
- **Fast cold start** — < 10ms for `--help`.

## Quick start

```bash
cargo build --release
./target/release/imgctl --help
```

```bash
# Convert + resize + crop
imgctl convert -i in.png -o out.jpg --quality 80
imgctl resize  -i in.png -o out.png --width 400 --fit contain
imgctl crop    -i in.png -o out.png --x -50 --y -50 --w 30 --h 30   # negative = from right/bottom

# Annotate (text/arrow/blur/rect)
imgctl rect  -i in.png -o out.png --x 10 --y 10 --w 100 --h 100 --color "#FF0000" --fill "#00FF0040"
imgctl text  -i in.png -o out.png --text "你好" --font "PingFang SC" --x 50 --y 50 --size 32
imgctl arrow -i in.png -o out.png --from 10,10 --to 200,200 --color "#FF0000" --style dashed
imgctl blur  -i in.png -o out.png --region 10,10,100,100 --sigma 8 --type pixelate

# Compose
imgctl concat -i a.png -i b.png -i c.png -o out.png --direction horizontal --gap 10

# Batch via JSON config (with JSON-Schema introspection)
imgctl annotate --print-schema             # → JSON Schema for ops.json
imgctl annotate -i in.png -o out.png --config ops.json

# Visual analysis
imgctl info  -i photo.jpg                   # dimensions + EXIF + dominant colors
imgctl diff  -a a.png -b b.png -o diff.png  # pixel diff + bounding boxes
imgctl hash  -i a.png -i b.png --algo phash # perceptual hashes + similarity
imgctl slice -i long.png --rows 3 --cols 1 --output-dir tiles/
imgctl map-coords --from-size 1280x720 --to-size 2560x1440 --point 640,360
imgctl fix   -i broken.jpg                  # detect format mismatch + JPEG truncation repair

# Diagrams (requires Chrome/Chromium installed locally)
imgctl mermaid -i flow.mmd -o flow.png --theme dark --width 1200
echo 'flowchart LR; A-->B-->C' | imgctl mermaid -i - -o out.svg --format svg
imgctl mermaid -i flow.mmd -o flow.png --chrome ws://localhost:9222   # reuse Chrome
```

## Output format

Default is TSV — each line is `key<TAB>value`. Nested objects use `parent.child` paths, arrays use `[i]` indices.

```
$ imgctl convert -i in.png -o out.jpg --quality 80
success	true
elapsed_ms	8
output	out.jpg
width	64
height	64
format	jpeg
size_bytes	1234
```

```
$ imgctl convert -i in.png -o out.bmp2
success	false
elapsed_ms	1
error.code	UNSUPPORTED_FORMAT
error.message	unsupported format: ...
```

Use `--json` for `serde_json`-friendly output, `--quiet` for exit-code-only.

## Build flags

```bash
# Default: all features
cargo build --release

# Minimal (only edit + vision, no Chrome/Tokio)
cargo build --release --no-default-features --features vision
# → ~4.2 MB binary, no chromiumoxide / tokio / reqwest in tree

# Full (default)
cargo build --release
# → ~9.0 MB binary
```

| Feature flag | What it adds |
|---|---|
| `vision` (default) | `info`, `diff`, `hash`, `slice`, `map-coords`, `fix` (kamadak-exif) |
| `mermaid` (default) | `mermaid` command + chromiumoxide + tokio + resvg |

The base binary always includes the 9 edit commands (convert/resize/crop/text/arrow/blur/rect/concat/annotate); they require only `image` + `imageproc` + `ab_glyph`.

## Architecture

Cargo workspace, four library crates plus the CLI binary:

```
imgctl/
├── Cargo.toml                          # [workspace] resolver = "3"
├── apps/
│   └── imgctl-cli/                     # clap entry, subcommand dispatch, dual-channel writer
└── crates/
    ├── imgctl-core/                    # protocol layer: Response<T>, Error+code(), TSV/JSON,
    │                                   #   IoSource, ColorRgba, Region/Point/Size newtypes
    ├── imgctl-image/                   # 9 edit commands, drawing primitives, image deps
    ├── imgctl-vision/                  # 6 visual-analysis commands
    └── imgctl-mermaid/                 # Chrome-based Mermaid renderer, SVG→PNG (resvg)
```

Engineering rules:
- **Zero panic in library code** — every fallible call returns `Result<T, imgctl_core::Error>`. `unwrap`/`expect` are confined to test modules.
- **Newtype everywhere** — colors, points, sizes, regions are not bare tuples or `(u32, u32, u32, u32)`.
- **Strong-typed CLI args** — all multi-choice flags (`--fit`, `--style`, `--align`, `--algo`, `--theme`, …) use clap `ValueEnum`; no stringly-typed states.

## Testing

```bash
cargo test --workspace                  # 168 tests
cargo test -- --ignored                 # +1 mermaid E2E (requires Chrome)
```

Coverage:

| Crate | Tests |
|---|---|
| imgctl-core | 31 |
| imgctl-image | 69 |
| imgctl-vision | 30 |
| imgctl-mermaid | 7 |
| cli_smoke (E2E via `assert_cmd`) | 31 + 1 ignored |
| **Total** | **168** + 1 ignored |

## Fonts

The default font is **NotoSans-Regular.ttf** (OFL, embedded, ~826KB) — Latin/Greek/Cyrillic only. For CJK text, pass a system family name to `--font`:

```bash
imgctl text --font "Hiragino Sans"  ...   # macOS, Japanese
imgctl text --font "PingFang SC"    ...   # macOS, 简体中文
imgctl text --font "Source Han Sans" ...  # cross-platform CJK if installed
```

Resolution order: existing file path → `fontdb` system query by family name → embedded NotoSans (no `--font` argument).

## Status

v0.1.0 — all 16 commands shipped end-to-end. Tested on macOS Apple Silicon (rust 1.94+).

Out of scope today: Windows builds, video, OCR, AVIF, WASM target (REQ §7 Phase 4).

## License

MIT OR Apache-2.0 (your choice). Embedded NotoSans-Regular is OFL — see `crates/imgctl-image/assets/fonts/LICENSE-OFL.txt`.
