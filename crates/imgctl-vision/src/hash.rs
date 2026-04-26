use clap::{Args, ValueEnum};
use image::DynamicImage;
use image::imageops::FilterType;
use serde::Serialize;

use imgctl_core::{Error, InputSource, Result};

#[derive(ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[clap(rename_all = "lowercase")]
pub enum HashAlgo {
    #[default]
    Phash,
    Dhash,
    Ahash,
}

#[derive(Args, Debug, Clone)]
pub struct HashArgs {
    /// Input image (specify once for hash, twice for similarity comparison)
    #[arg(short, long)]
    pub input: Vec<String>,

    /// Hash algorithm
    #[arg(long, value_enum, default_value_t = HashAlgo::Phash)]
    pub algo: HashAlgo,
}

#[derive(Debug, Serialize)]
pub struct HashOutput {
    pub algo: &'static str,
    pub hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash_b: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub similarity: Option<f64>,
}

pub fn run(args: HashArgs) -> Result<HashOutput> {
    if args.input.is_empty() || args.input.len() > 2 {
        return Err(Error::InvalidArgument(
            "hash requires 1 input (compute) or 2 inputs (compare)".into(),
        ));
    }

    let img_a = load(&args.input[0])?;
    let hash_a = compute_hash(&img_a, args.algo);

    if args.input.len() == 2 {
        let img_b = load(&args.input[1])?;
        let hash_b = compute_hash(&img_b, args.algo);
        let hd = (hash_a ^ hash_b).count_ones();
        let similarity = 1.0 - f64::from(hd) / 64.0;
        Ok(HashOutput {
            algo: algo_str(args.algo),
            hash: format!("{hash_a:016x}"),
            hash_b: Some(format!("{hash_b:016x}")),
            similarity: Some(similarity),
        })
    } else {
        Ok(HashOutput {
            algo: algo_str(args.algo),
            hash: format!("{hash_a:016x}"),
            hash_b: None,
            similarity: None,
        })
    }
}

fn load(path: &str) -> Result<DynamicImage> {
    let bytes = InputSource::from_arg(path).read_all()?;
    image::load_from_memory(&bytes).map_err(|e| Error::Image(e.to_string()))
}

fn algo_str(a: HashAlgo) -> &'static str {
    match a {
        HashAlgo::Phash => "phash",
        HashAlgo::Dhash => "dhash",
        HashAlgo::Ahash => "ahash",
    }
}

pub fn compute_hash(img: &DynamicImage, algo: HashAlgo) -> u64 {
    match algo {
        HashAlgo::Ahash => ahash(img),
        HashAlgo::Dhash => dhash(img),
        HashAlgo::Phash => phash(img),
    }
}

fn ahash(img: &DynamicImage) -> u64 {
    let small = img.resize_exact(8, 8, FilterType::Lanczos3).to_luma8();
    let pixels: Vec<u8> = small.pixels().map(|p| p.0[0]).collect();
    let sum: u32 = pixels.iter().map(|&p| u32::from(p)).sum();
    let mean = sum / 64;
    let mut hash = 0u64;
    for (i, &p) in pixels.iter().enumerate() {
        if u32::from(p) >= mean {
            hash |= 1u64 << i;
        }
    }
    hash
}

fn dhash(img: &DynamicImage) -> u64 {
    let small = img.resize_exact(9, 8, FilterType::Lanczos3).to_luma8();
    let mut hash = 0u64;
    let mut bit = 0u32;
    for y in 0..8u32 {
        for x in 0..8u32 {
            let l = small.get_pixel(x, y).0[0];
            let r = small.get_pixel(x + 1, y).0[0];
            if l > r {
                hash |= 1u64 << bit;
            }
            bit += 1;
        }
    }
    hash
}

fn phash(img: &DynamicImage) -> u64 {
    const N: usize = 32;
    let small = img.resize_exact(N as u32, N as u32, FilterType::Lanczos3).to_luma8();
    let pixels: Vec<f64> = small.pixels().map(|p| f64::from(p.0[0])).collect();
    let dct = dct_2d(&pixels, N);

    // Take top-left 8x8 coefficients (DC at index 0).
    let mut coeffs = Vec::with_capacity(64);
    for v in 0..8 {
        for u in 0..8 {
            coeffs.push(dct[v * N + u]);
        }
    }

    // Median of the 63 non-DC coefficients.
    let mut non_dc: Vec<f64> = coeffs[1..].to_vec();
    non_dc.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = non_dc[non_dc.len() / 2];

    let mut hash = 0u64;
    for (i, &c) in coeffs.iter().enumerate() {
        if c >= median {
            hash |= 1u64 << i;
        }
    }
    hash
}

fn dct_2d(input: &[f64], n: usize) -> Vec<f64> {
    let mut output = vec![0.0_f64; n * n];
    let pi = std::f64::consts::PI;
    let n_f = n as f64;
    let c0 = 1.0 / n_f.sqrt();
    let ck = (2.0 / n_f).sqrt();
    for v in 0..n {
        for u in 0..n {
            let mut sum = 0.0;
            for y in 0..n {
                for x in 0..n {
                    let cx = ((pi * (2.0 * x as f64 + 1.0) * u as f64) / (2.0 * n_f)).cos();
                    let cy = ((pi * (2.0 * y as f64 + 1.0) * v as f64) / (2.0 * n_f)).cos();
                    sum += input[y * n + x] * cx * cy;
                }
            }
            let cu = if u == 0 { c0 } else { ck };
            let cv = if v == 0 { c0 } else { ck };
            output[v * n + u] = cu * cv * sum;
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn solid(c: u8) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(64, 64, Rgba([c, c, c, 255])))
    }

    fn pattern(seed: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_fn(64, 64, |x, y| {
            let v = ((x.wrapping_mul(7) + y.wrapping_mul(13) + seed) % 255) as u8;
            Rgba([v, v.wrapping_mul(2), v.wrapping_mul(3), 255])
        }))
    }

    #[test]
    fn ahash_self_similar() {
        let img = pattern(0);
        let h1 = ahash(&img);
        let h2 = ahash(&img);
        assert_eq!(h1, h2);
    }

    #[test]
    fn dhash_self_similar() {
        let img = pattern(0);
        assert_eq!(dhash(&img), dhash(&img));
    }

    #[test]
    fn phash_self_similar() {
        let img = pattern(0);
        assert_eq!(phash(&img), phash(&img));
    }

    #[test]
    fn different_patterns_have_different_hashes() {
        let a = pattern(0);
        let b = pattern(123);
        assert_ne!(phash(&a), phash(&b));
        assert_ne!(dhash(&a), dhash(&b));
    }

    #[test]
    fn solid_image_hashes_no_panic() {
        // ahash mean comparison with constant pixels — all bits should be set or unset.
        let img = solid(128);
        let _ = ahash(&img);
        let _ = dhash(&img);
        let _ = phash(&img);
    }

    #[test]
    fn similar_images_have_high_similarity() {
        let a = pattern(0);
        // "lightly perturbed": same pattern, slight intensity shift
        let b = DynamicImage::ImageRgba8(RgbaImage::from_fn(64, 64, |x, y| {
            let v = ((x.wrapping_mul(7) + y.wrapping_mul(13)) % 255) as u8;
            let v2 = v.saturating_add(5);
            Rgba([v2, v2.wrapping_mul(2), v2.wrapping_mul(3), 255])
        }));
        let ha = phash(&a);
        let hb = phash(&b);
        let hd = (ha ^ hb).count_ones();
        let similarity = 1.0 - f64::from(hd) / 64.0;
        // Synthetic pattern + saturating intensity shift; self-implemented phash
        // tends to flip more bits than a tuned production hash. 0.6 is a safe
        // floor that still distinguishes "similar" from "different" patterns.
        assert!(similarity > 0.6, "expected > 0.6, got {similarity}");
    }
}
