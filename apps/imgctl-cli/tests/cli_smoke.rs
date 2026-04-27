use std::path::PathBuf;
use std::time::SystemTime;

use assert_cmd::Command;

const SUBCOMMANDS: &[&str] = &[
    "convert",
    "resize",
    "crop",
    "text",
    "arrow",
    "blur",
    "rect",
    "concat",
    "annotate",
    "info",
    "diff",
    "hash",
    "slice",
    "map-coords",
    "fix",
    "mermaid",
];

fn imgctl() -> Command {
    Command::cargo_bin("imgctl").expect("binary built")
}

fn unique_temp(suffix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "imgctl-cli-{}-{nanos}-{suffix}",
        std::process::id()
    ))
}

fn write_fixture_png(path: &PathBuf) {
    write_fixture_png_sized(path, 64, 64);
}

fn write_fixture_png_sized(path: &PathBuf, w: u32, h: u32) {
    use image::{DynamicImage, Rgba, RgbaImage};
    let img = RgbaImage::from_fn(w, h, |x, y| {
        let v = ((x * 7 + y * 11) % 255) as u8;
        Rgba([v, v.wrapping_mul(2), v.wrapping_mul(3), 255])
    });
    let dyn_img = DynamicImage::ImageRgba8(img);
    let mut buf = Vec::new();
    dyn_img
        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    std::fs::write(path, &buf).unwrap();
}

#[test]
fn help_lists_all_subcommands() {
    let output = imgctl().arg("--help").assert().success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout).to_string();
    for sub in SUBCOMMANDS {
        assert!(
            stdout.contains(sub),
            "help missing subcommand `{sub}`:\n{stdout}"
        );
    }
}

#[test]
fn version_includes_package_version() {
    let output = imgctl().arg("--version").assert().success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout).to_string();
    assert!(stdout.contains("0.1.0"), "version output: {stdout}");
}

#[test]
fn error_path_emits_tsv() {
    // Use convert against a non-existent file to trigger an IO error response.
    let output = imgctl()
        .args([
            "convert",
            "-i",
            "/tmp/__imgctl_does_not_exist__.png",
            "-o",
            "/tmp/out.png",
        ])
        .assert()
        .failure()
        .code(2);
    let stdout = String::from_utf8_lossy(&output.get_output().stdout).to_string();
    assert!(stdout.contains("success\tfalse"), "stdout: {stdout}");
    assert!(stdout.contains("error.code\tIO_ERROR"), "stdout: {stdout}");
}

#[test]
fn error_path_emits_json() {
    let output = imgctl()
        .args([
            "convert",
            "-i",
            "/tmp/__imgctl_does_not_exist__.png",
            "-o",
            "/tmp/out.png",
            "--json",
        ])
        .assert()
        .failure()
        .code(2);
    let stdout = String::from_utf8_lossy(&output.get_output().stdout).to_string();
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("invalid JSON: {e}\nstdout was:\n{stdout}"));
    assert_eq!(v["success"], false);
    assert_eq!(v["error"]["code"], "IO_ERROR");
}

#[test]
fn quiet_flag_suppresses_output() {
    let output = imgctl()
        .args([
            "convert",
            "-i",
            "/tmp/__imgctl_does_not_exist__.png",
            "-o",
            "/tmp/out.png",
            "--quiet",
        ])
        .assert()
        .failure()
        .code(2);
    let stdout = output.get_output().stdout.clone();
    assert!(stdout.is_empty(), "expected empty stdout, got: {stdout:?}");
}

#[test]
fn convert_e2e_png_to_jpeg() {
    let input = unique_temp("in.png");
    let output = unique_temp("out.jpg");
    write_fixture_png(&input);

    let result = imgctl()
        .arg("convert")
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--quality")
        .arg("80")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();

    assert!(stdout.contains("success\ttrue"), "stdout: {stdout}");
    assert!(stdout.contains("width\t64"), "stdout: {stdout}");
    assert!(stdout.contains("height\t64"), "stdout: {stdout}");
    assert!(stdout.contains("format\tjpeg"), "stdout: {stdout}");
    assert!(stdout.contains(&format!("output\t{}", output.to_string_lossy())));

    let on_disk = std::fs::metadata(&output).unwrap().len();
    assert!(on_disk > 0);

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output);
}

#[test]
fn convert_e2e_unsupported_extension_errs() {
    let input = unique_temp("in.png");
    let output = unique_temp("out.bmp2");
    write_fixture_png(&input);

    let result = imgctl()
        .arg("convert")
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .assert()
        .failure()
        .code(2);
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(
        stdout.contains("error.code\tUNSUPPORTED_FORMAT"),
        "stdout: {stdout}"
    );

    let _ = std::fs::remove_file(&input);
}

#[test]
fn resize_e2e_contain() {
    let input = unique_temp("in.png");
    let output = unique_temp("out.png");
    write_fixture_png_sized(&input, 400, 200);

    let result = imgctl()
        .arg("resize")
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--width")
        .arg("100")
        .arg("--height")
        .arg("100")
        .arg("--fit")
        .arg("contain")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("success\ttrue"), "stdout: {stdout}");
    assert!(stdout.contains("width\t100"), "stdout: {stdout}");
    assert!(stdout.contains("height\t50"), "stdout: {stdout}");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output);
}

#[test]
fn resize_e2e_missing_dimensions_errs() {
    let input = unique_temp("in.png");
    let output = unique_temp("out.png");
    write_fixture_png(&input);

    let result = imgctl()
        .arg("resize")
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .assert()
        .failure()
        .code(2);
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(
        stdout.contains("error.code\tINVALID_ARGUMENT"),
        "stdout: {stdout}"
    );

    let _ = std::fs::remove_file(&input);
}

#[test]
fn crop_e2e_normal_region() {
    let input = unique_temp("in.png");
    let output = unique_temp("out.png");
    write_fixture_png_sized(&input, 200, 200);

    let result = imgctl()
        .arg("crop")
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--x")
        .arg("10")
        .arg("--y")
        .arg("10")
        .arg("--w")
        .arg("100")
        .arg("--h")
        .arg("100")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("success\ttrue"), "stdout: {stdout}");
    assert!(stdout.contains("width\t100"), "stdout: {stdout}");
    assert!(stdout.contains("height\t100"), "stdout: {stdout}");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output);
}

#[test]
fn crop_e2e_negative_coords() {
    let input = unique_temp("in.png");
    let output = unique_temp("out.png");
    write_fixture_png_sized(&input, 200, 200);

    let result = imgctl()
        .arg("crop")
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--x")
        .arg("-50")
        .arg("--y")
        .arg("-50")
        .arg("--w")
        .arg("30")
        .arg("--h")
        .arg("30")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("width\t30"), "stdout: {stdout}");
    assert!(stdout.contains("height\t30"), "stdout: {stdout}");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output);
}

#[test]
fn rect_e2e_stroke_only() {
    let input = unique_temp("in.png");
    let output = unique_temp("out.png");
    write_fixture_png_sized(&input, 200, 200);

    let result = imgctl()
        .arg("rect")
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--x")
        .arg("10")
        .arg("--y")
        .arg("10")
        .arg("--w")
        .arg("100")
        .arg("--h")
        .arg("100")
        .arg("--color")
        .arg("#FF0000")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("success\ttrue"), "stdout: {stdout}");
    assert!(stdout.contains("width\t200"), "stdout: {stdout}");
    assert!(stdout.contains("height\t200"), "stdout: {stdout}");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output);
}

#[test]
fn rect_e2e_with_translucent_fill() {
    let input = unique_temp("in.png");
    let output = unique_temp("out.png");
    write_fixture_png_sized(&input, 200, 200);

    let result = imgctl()
        .arg("rect")
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--x")
        .arg("10")
        .arg("--y")
        .arg("10")
        .arg("--w")
        .arg("100")
        .arg("--h")
        .arg("100")
        .arg("--color")
        .arg("#00FF00")
        .arg("--fill")
        .arg("#00FF0040")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("success\ttrue"), "stdout: {stdout}");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output);
}

#[test]
fn text_e2e_default_font() {
    let input = unique_temp("in.png");
    let output = unique_temp("out.png");
    write_fixture_png_sized(&input, 200, 100);

    let result = imgctl()
        .arg("text")
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--text")
        .arg("Hello")
        .arg("--x")
        .arg("20")
        .arg("--y")
        .arg("20")
        .arg("--size")
        .arg("32")
        .arg("--color")
        .arg("#FF0000")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("success\ttrue"), "stdout: {stdout}");
    assert!(stdout.contains("width\t200"), "stdout: {stdout}");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output);
}

#[test]
fn text_e2e_unknown_font_errs() {
    let input = unique_temp("in.png");
    let output = unique_temp("out.png");
    write_fixture_png_sized(&input, 100, 100);

    let result = imgctl()
        .arg("text")
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--text")
        .arg("x")
        .arg("--x")
        .arg("0")
        .arg("--y")
        .arg("0")
        .arg("--font")
        .arg("__not_a_real_font_anywhere__")
        .assert()
        .failure()
        .code(2);
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("error.code\tNOT_FOUND"), "stdout: {stdout}");

    let _ = std::fs::remove_file(&input);
}

#[test]
fn arrow_e2e_solid() {
    let input = unique_temp("in.png");
    let output = unique_temp("out.png");
    write_fixture_png_sized(&input, 200, 100);

    let result = imgctl()
        .arg("arrow")
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--from")
        .arg("10,50")
        .arg("--to")
        .arg("180,50")
        .arg("--color")
        .arg("#FF0000")
        .arg("--width")
        .arg("2")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("success\ttrue"), "stdout: {stdout}");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output);
}

#[test]
fn blur_e2e_two_regions() {
    let input = unique_temp("in.png");
    let output = unique_temp("out.png");
    write_fixture_png_sized(&input, 200, 200);

    let result = imgctl()
        .arg("blur")
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--region")
        .arg("10,10,50,50")
        .arg("--region")
        .arg("100,100,50,50")
        .arg("--sigma")
        .arg("4")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("success\ttrue"), "stdout: {stdout}");
    assert!(stdout.contains("regions_processed\t2"), "stdout: {stdout}");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output);
}

#[test]
fn blur_e2e_no_region_errs() {
    let input = unique_temp("in.png");
    let output = unique_temp("out.png");
    write_fixture_png_sized(&input, 100, 100);

    let result = imgctl()
        .arg("blur")
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--sigma")
        .arg("4")
        .assert()
        .failure()
        .code(2);
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(
        stdout.contains("error.code\tINVALID_ARGUMENT"),
        "stdout: {stdout}"
    );

    let _ = std::fs::remove_file(&input);
}

#[test]
fn concat_e2e_horizontal_three() {
    let a = unique_temp("a.png");
    let b = unique_temp("b.png");
    let c = unique_temp("c.png");
    let output = unique_temp("out.png");
    write_fixture_png_sized(&a, 50, 50);
    write_fixture_png_sized(&b, 50, 50);
    write_fixture_png_sized(&c, 50, 50);

    let result = imgctl()
        .arg("concat")
        .arg("-i")
        .arg(&a)
        .arg("-i")
        .arg(&b)
        .arg("-i")
        .arg(&c)
        .arg("-o")
        .arg(&output)
        .arg("--direction")
        .arg("horizontal")
        .arg("--gap")
        .arg("10")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("success\ttrue"), "stdout: {stdout}");
    assert!(stdout.contains("width\t170"), "stdout: {stdout}"); // 50+10+50+10+50
    assert!(stdout.contains("height\t50"), "stdout: {stdout}");
    assert!(stdout.contains("inputs\t3"), "stdout: {stdout}");

    for p in [&a, &b, &c, &output] {
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn concat_e2e_single_input_errs() {
    let a = unique_temp("a.png");
    let output = unique_temp("out.png");
    write_fixture_png(&a);

    let result = imgctl()
        .arg("concat")
        .arg("-i")
        .arg(&a)
        .arg("-o")
        .arg(&output)
        .assert()
        .failure()
        .code(2);
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(
        stdout.contains("error.code\tINVALID_ARGUMENT"),
        "stdout: {stdout}"
    );

    let _ = std::fs::remove_file(&a);
}

#[test]
fn annotate_e2e_print_schema() {
    let result = imgctl()
        .args(["annotate", "--print-schema"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    let schema: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("expected JSON schema, got: {e}\n{stdout}"));
    // Schema should describe an OperationsFile with operations field.
    let s = schema.to_string();
    assert!(s.contains("operations"), "schema: {s}");
}

#[test]
fn annotate_e2e_runs_three_ops() {
    let input = unique_temp("in.png");
    let output = unique_temp("out.png");
    let config = unique_temp("ops.json");
    write_fixture_png_sized(&input, 200, 200);
    std::fs::write(
        &config,
        r##"{
            "operations": [
                { "type": "rect", "x": 20, "y": 20, "w": 80, "h": 80, "color": "#FF0000" },
                { "type": "arrow", "from": [10, 10], "to": [180, 180], "color": "#00FF00", "width": 3 },
                { "type": "text", "text": "Hi", "x": 50, "y": 60, "size": 24, "color": "#0000FF" }
            ]
        }"##,
    )
    .unwrap();

    let result = imgctl()
        .arg("annotate")
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--config")
        .arg(&config)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("success\ttrue"), "stdout: {stdout}");
    assert!(stdout.contains("operations\t3"), "stdout: {stdout}");

    for p in [&input, &output, &config] {
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn annotate_e2e_unknown_op_type_errs() {
    let input = unique_temp("in.png");
    let output = unique_temp("out.png");
    let config = unique_temp("ops.json");
    write_fixture_png(&input);
    std::fs::write(&config, r#"{"operations":[{"type":"foo"}]}"#).unwrap();

    let result = imgctl()
        .arg("annotate")
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--config")
        .arg(&config)
        .assert()
        .failure()
        .code(2);
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(
        stdout.contains("error.code\tINVALID_ARGUMENT"),
        "stdout: {stdout}"
    );

    for p in [&input, &config] {
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn map_coords_e2e_2x_upscale() {
    let result = imgctl()
        .arg("map-coords")
        .arg("--from-size")
        .arg("1280x720")
        .arg("--to-size")
        .arg("2560x1440")
        .arg("--point")
        .arg("640,360")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("scale_x\t2"), "stdout: {stdout}");
    assert!(stdout.contains("output.x\t1280"), "stdout: {stdout}");
    assert!(stdout.contains("output.y\t720"), "stdout: {stdout}");
}

#[test]
fn slice_e2e_2x2() {
    let dir = unique_temp("slice");
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("in.png");
    write_fixture_png_sized(&input, 200, 200);

    let result = imgctl()
        .arg("slice")
        .arg("-i")
        .arg(&input)
        .arg("--rows")
        .arg("2")
        .arg("--cols")
        .arg("2")
        .arg("--output-dir")
        .arg(&dir)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("success\ttrue"), "stdout: {stdout}");
    assert!(stdout.contains("tiles[0].w\t100"), "stdout: {stdout}");
    assert!(stdout.contains("tiles[3].w\t100"), "stdout: {stdout}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn info_e2e_solid_png() {
    let input = unique_temp("solid.png");
    write_fixture_png_sized(&input, 100, 80);

    let result = imgctl()
        .arg("info")
        .arg("-i")
        .arg(&input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("width\t100"), "stdout: {stdout}");
    assert!(stdout.contains("height\t80"), "stdout: {stdout}");
    assert!(stdout.contains("format\tpng"), "stdout: {stdout}");
    assert!(stdout.contains("channels\t4"), "stdout: {stdout}");
    assert!(stdout.contains("has_alpha\ttrue"), "stdout: {stdout}");
    assert!(stdout.contains("dominant_colors[0]"), "stdout: {stdout}");

    let _ = std::fs::remove_file(&input);
}

#[test]
fn diff_e2e_detects_changed_region() {
    use image::{DynamicImage, Rgba, RgbaImage};
    let a_path = unique_temp("a.png");
    let b_path = unique_temp("b.png");
    let diff_path = unique_temp("diff.png");

    let mut a = RgbaImage::from_pixel(100, 100, Rgba([0, 0, 0, 255]));
    let mut b = a.clone();
    // Change a 30x30 patch in b
    for y in 30..60 {
        for x in 30..60 {
            b.put_pixel(x, y, Rgba([255, 255, 255, 255]));
        }
    }
    for (img, p) in [(&a, &a_path), (&b, &b_path)] {
        let dyn_img = DynamicImage::ImageRgba8(img.clone());
        let mut buf = Vec::new();
        dyn_img
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(p, &buf).unwrap();
    }
    let _ = (&mut a,); // suppress unused-mut warning

    let result = imgctl()
        .arg("diff")
        .arg("-a")
        .arg(&a_path)
        .arg("-b")
        .arg(&b_path)
        .arg("-o")
        .arg(&diff_path)
        .arg("--threshold")
        .arg("0.01")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("changed\ttrue"), "stdout: {stdout}");
    assert!(stdout.contains("changed_regions[0]"), "stdout: {stdout}");
    assert!(std::fs::metadata(&diff_path).is_ok());

    for p in [&a_path, &b_path, &diff_path] {
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn hash_e2e_single_input() {
    let input = unique_temp("hash.png");
    write_fixture_png_sized(&input, 64, 64);

    let result = imgctl()
        .arg("hash")
        .arg("-i")
        .arg(&input)
        .arg("--algo")
        .arg("phash")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("algo\tphash"), "stdout: {stdout}");
    // hash field should be 16 hex chars
    let line = stdout
        .lines()
        .find(|l| l.starts_with("hash\t"))
        .expect("hash field");
    let hex = line.trim_start_matches("hash\t");
    assert_eq!(hex.len(), 16, "hash hex length: {hex}");
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()), "non-hex: {hex}");

    let _ = std::fs::remove_file(&input);
}

#[test]
fn hash_e2e_compare_identical_images() {
    let input = unique_temp("hash-cmp.png");
    write_fixture_png_sized(&input, 64, 64);

    let result = imgctl()
        .arg("hash")
        .arg("-i")
        .arg(&input)
        .arg("-i")
        .arg(&input)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("similarity\t1"), "stdout: {stdout}");

    let _ = std::fs::remove_file(&input);
}

/// E2E test for `imgctl mermaid` requires Chrome/Chromium installed locally.
/// Run with `cargo test -- --ignored` to execute.
#[test]
#[ignore]
fn mermaid_e2e_requires_chrome() {
    let mmd = unique_temp("flow.mmd");
    let output = unique_temp("flow.svg");
    std::fs::write(&mmd, "flowchart LR\n  A --> B --> C\n").unwrap();

    let result = imgctl()
        .arg("mermaid")
        .arg("-i")
        .arg(&mmd)
        .arg("-o")
        .arg(&output)
        .arg("--format")
        .arg("svg")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("success\ttrue"), "stdout: {stdout}");
    assert!(stdout.contains("format\tsvg"));
    let svg = std::fs::read_to_string(&output).unwrap();
    assert!(svg.contains("<svg"), "expected SVG content: {svg}");

    let _ = std::fs::remove_file(&mmd);
    let _ = std::fs::remove_file(&output);
}

#[test]
fn fix_e2e_detects_extension_mismatch() {
    let png = unique_temp("misnamed.jpg");
    write_fixture_png(&png);

    let result = imgctl().arg("fix").arg("-i").arg(&png).assert().success();
    let stdout = String::from_utf8_lossy(&result.get_output().stdout).to_string();
    assert!(stdout.contains("detected_format\tpng"), "stdout: {stdout}");
    assert!(
        stdout.contains("extension_format\tjpeg"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("mismatch\ttrue"), "stdout: {stdout}");

    let _ = std::fs::remove_file(&png);
}

#[test]
fn convert_e2e_stdio_routes_meta_to_stderr() {
    let input = unique_temp("in.png");
    write_fixture_png(&input);

    let result = imgctl()
        .arg("convert")
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg("-")
        .arg("--format")
        .arg("jpeg")
        .assert()
        .success();
    let stdout = result.get_output().stdout.clone();
    let stderr = String::from_utf8_lossy(&result.get_output().stderr).to_string();

    // Binary on stdout: should start with JPEG SOI.
    assert!(
        stdout.len() > 100,
        "expected non-trivial JPEG bytes, got {}",
        stdout.len()
    );
    assert_eq!(&stdout[0..2], &[0xFF, 0xD8], "expected JPEG SOI on stdout");

    // Metadata on stderr.
    assert!(stderr.contains("success\ttrue"), "stderr: {stderr}");
    assert!(stderr.contains("format\tjpeg"), "stderr: {stderr}");

    let _ = std::fs::remove_file(&input);
}
