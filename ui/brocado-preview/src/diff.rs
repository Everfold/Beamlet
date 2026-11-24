//! Tolerance-aware RGBA pixel comparison and Golden snapshot verification helpers.

use std::fs;
use std::path::Path;

use anyhow::{Context as _, bail};
use image::{ImageFormat, Rgba, RgbaImage};

/// Configurable pixel-level tolerance bounds for image comparison and snapshot verification.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageDiffTolerance {
    /// Maximum per-channel (`R`/`G`/`B`/`A`) absolute delta tolerated before a pixel counts as differing.
    pub max_channel_delta: u8,
    /// Maximum fraction (`0.0..=1.0`) of differing pixels tolerated while still reporting a match.
    pub max_diff_ratio: f64,
}

impl ImageDiffTolerance {
    /// Zero-tolerance exact pixel-for-pixel comparison (`delta = 0`, `ratio = 0.0`).
    #[must_use]
    pub const fn exact() -> Self {
        Self {
            max_channel_delta: 0,
            max_diff_ratio: 0.0,
        }
    }

    /// Mild subpixel anti-aliasing tolerance (`delta <= 2`, `diff_ratio <= 0.1%`).
    #[must_use]
    pub const fn antialiased() -> Self {
        Self {
            max_channel_delta: 2,
            max_diff_ratio: 0.001,
        }
    }
}

impl Default for ImageDiffTolerance {
    fn default() -> Self {
        Self::antialiased()
    }
}

/// Summary and optional highlighted diff artifact produced by comparing two RGBA images.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageDiffReport {
    /// Whether `actual` satisfies the configured [`ImageDiffTolerance`] against `reference`.
    pub matched: bool,
    /// Total pixel area across the compared union rectangle.
    pub total_pixels: u64,
    /// Number of pixels whose max per-channel delta exceeded [`ImageDiffTolerance::max_channel_delta`].
    pub diff_pixels: u64,
    /// Fraction of differing pixels (`diff_pixels / total_pixels`).
    pub diff_ratio: f64,
    /// Largest single-channel absolute delta observed across all pixels.
    pub max_channel_delta: u8,
    /// `Some((reference_dims, actual_dims))` if image dimensions differed.
    pub size_mismatch: Option<((u32, u32), (u32, u32))>,
    /// Dimmed grayscale reference overlaid with bright magenta `[255, 0, 128, 255]` diff pixels.
    pub diff_image: Option<RgbaImage>,
}

fn channel_delta(a: Rgba<u8>, b: Rgba<u8>) -> u8 {
    a.0.into_iter()
        .zip(b.0)
        .map(|(x, y)| x.abs_diff(y))
        .max()
        .unwrap_or(0)
}

fn dimmed_ref_pixel(px: Rgba<u8>) -> Rgba<u8> {
    let luma = ((u16::from(px.0[0]) + u16::from(px.0[1]) + u16::from(px.0[2])) / 3) as u8;
    let dim = luma / 3;
    Rgba([dim, dim, dim, 255])
}

/// Compares `reference` and `actual` RGBA images under `tolerance`.
///
/// Generates `diff_image` whenever any pixel differs (or image sizes mismatch), highlighting
/// differing pixels in bright magenta over a dimmed grayscale reference background.
#[must_use]
pub fn compare_images(
    reference: &RgbaImage,
    actual: &RgbaImage,
    tolerance: ImageDiffTolerance,
) -> ImageDiffReport {
    let ref_dims = reference.dimensions();
    let act_dims = actual.dimensions();
    let size_mismatch = (ref_dims != act_dims).then_some((ref_dims, act_dims));

    let width = ref_dims.0.max(act_dims.0);
    let height = ref_dims.1.max(act_dims.1);
    let total_pixels = u64::from(width) * u64::from(height);

    let mut diff_pixels = 0u64;
    let mut max_delta_seen = 0u8;
    let mut any_nonzero_delta = size_mismatch.is_some();

    for y in 0..height {
        for x in 0..width {
            let delta = match (
                (x < ref_dims.0 && y < ref_dims.1).then(|| *reference.get_pixel(x, y)),
                (x < act_dims.0 && y < act_dims.1).then(|| *actual.get_pixel(x, y)),
            ) {
                (Some(r), Some(a)) => channel_delta(r, a),
                (Some(r), None) => channel_delta(r, Rgba([0, 0, 0, 0])).max(1),
                (None, Some(a)) => channel_delta(Rgba([0, 0, 0, 0]), a).max(1),
                (None, None) => 0,
            };
            if delta > 0 {
                any_nonzero_delta = true;
            }
            if delta > max_delta_seen {
                max_delta_seen = delta;
            }
            if delta > tolerance.max_channel_delta {
                diff_pixels += 1;
            }
        }
    }

    let diff_ratio = if total_pixels == 0 {
        0.0
    } else {
        diff_pixels as f64 / total_pixels as f64
    };
    let matched = size_mismatch.is_none() && diff_ratio <= tolerance.max_diff_ratio.max(0.0);

    let diff_image = any_nonzero_delta.then(|| {
        let mut out = RgbaImage::new(width.max(1), height.max(1));
        for y in 0..height {
            for x in 0..width {
                let in_ref = x < ref_dims.0 && y < ref_dims.1;
                let in_act = x < act_dims.0 && y < act_dims.1;
                let px = match (in_ref, in_act) {
                    (true, true) => {
                        let r = *reference.get_pixel(x, y);
                        let a = *actual.get_pixel(x, y);
                        if channel_delta(r, a) > tolerance.max_channel_delta {
                            Rgba([255, 0, 128, 255])
                        } else {
                            dimmed_ref_pixel(r)
                        }
                    }
                    _ => Rgba([255, 0, 128, 255]),
                };
                out.put_pixel(x, y, px);
            }
        }
        out
    });

    ImageDiffReport {
        matched,
        total_pixels,
        diff_pixels,
        diff_ratio,
        max_channel_delta: max_delta_seen,
        size_mismatch,
        diff_image,
    }
}

fn bless_enabled() -> bool {
    ["BROCADO_TEST_BLESS", "MASONRY_TEST_BLESS"]
        .into_iter()
        .any(|key| std::env::var_os(key).is_some_and(|val| !val.is_empty()))
}

fn write_png(img: &RgbaImage, path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed creating snapshot directory {}", parent.display()))?;
    }
    img.save_with_format(path, ImageFormat::Png)
        .with_context(|| format!("failed writing PNG to {}", path.display()))
}

fn remove_if_exists(path: &Path) {
    if path.exists() {
        let _ = fs::remove_file(path);
    }
}

/// Compares `actual` against `<manifest_dir>/screenshots/<test_name>.png` using `tolerance`.
///
/// * When `BROCADO_TEST_BLESS` or `MASONRY_TEST_BLESS` is non-empty, writes the reference PNG and
///   cleans up any stale `.new.png` / `.diff.png` artifacts.
/// * On mismatch or missing baseline, writes `<test_name>.new.png` (and `<test_name>.diff.png` on
///   mismatch) and returns a descriptive error containing observed pixel delta statistics.
pub fn check_snapshot_image(
    actual: &RgbaImage,
    manifest_dir: impl AsRef<Path>,
    test_name: &str,
    tolerance: ImageDiffTolerance,
) -> anyhow::Result<ImageDiffReport> {
    let screenshots_dir = manifest_dir.as_ref().join("screenshots");
    let reference_path = screenshots_dir.join(format!("{test_name}.png"));
    let new_path = screenshots_dir.join(format!("{test_name}.new.png"));
    let diff_path = screenshots_dir.join(format!("{test_name}.diff.png"));

    if bless_enabled() {
        write_png(actual, &reference_path)?;
        remove_if_exists(&new_path);
        remove_if_exists(&diff_path);
        return Ok(compare_images(actual, actual, tolerance));
    }

    if !reference_path.exists() {
        write_png(actual, &new_path)?;
        bail!(
            "missing baseline screenshot {} (wrote {}; re-run with BROCADO_TEST_BLESS=1 to accept)",
            reference_path.display(),
            new_path.display()
        );
    }

    let reference = image::open(&reference_path)
        .with_context(|| format!("failed reading baseline {}", reference_path.display()))?
        .to_rgba8();
    let report = compare_images(&reference, actual, tolerance);

    if !report.matched {
        write_png(actual, &new_path)?;
        if let Some(diff_img) = &report.diff_image {
            write_png(diff_img, &diff_path)?;
        }
        if let Some((ref_dims, act_dims)) = report.size_mismatch {
            bail!(
                "snapshot '{test_name}' size mismatch: reference {ref_dims:?} != actual {act_dims:?} (wrote {} and {})",
                new_path.display(),
                diff_path.display()
            );
        }
        bail!(
            "snapshot '{test_name}' mismatch: {}/{} differing pixels ({:.4}%, max_channel_delta={}, tolerance delta={}, ratio={:.4}%) — wrote {} and {}",
            report.diff_pixels,
            report.total_pixels,
            report.diff_ratio * 100.0,
            report.max_channel_delta,
            tolerance.max_channel_delta,
            tolerance.max_diff_ratio * 100.0,
            new_path.display(),
            diff_path.display()
        );
    }

    remove_if_exists(&new_path);
    remove_if_exists(&diff_path);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "brocado_preview_{label}_{}_{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn compare_images_handles_exact_tolerance_ratio_and_size_mismatch() {
        let base = RgbaImage::from_pixel(10, 10, Rgba([100, 100, 100, 255]));
        let exact = compare_images(&base, &base, ImageDiffTolerance::exact());
        assert!(exact.matched);
        assert_eq!(exact.diff_pixels, 0);
        assert_eq!(exact.max_channel_delta, 0);
        assert!(exact.diff_image.is_none());

        let mut aa = base.clone();
        aa.put_pixel(0, 0, Rgba([102, 99, 100, 255]));
        let strict_aa = compare_images(&base, &aa, ImageDiffTolerance::exact());
        assert!(!strict_aa.matched);
        assert_eq!(strict_aa.diff_pixels, 1);
        assert_eq!(strict_aa.max_channel_delta, 2);
        assert!(strict_aa.diff_image.is_some());

        let tolerant_aa = compare_images(&base, &aa, ImageDiffTolerance::antialiased());
        assert!(tolerant_aa.matched);
        assert_eq!(tolerant_aa.diff_pixels, 0);
        assert_eq!(tolerant_aa.max_channel_delta, 2);

        let mut broken = base.clone();
        broken.put_pixel(1, 1, Rgba([200, 100, 100, 255]));
        let ratio_fail = compare_images(&base, &broken, ImageDiffTolerance::antialiased());
        assert!(!ratio_fail.matched);
        assert_eq!(ratio_fail.diff_pixels, 1);
        assert_eq!(ratio_fail.diff_ratio, 0.01);
        let diff_img = ratio_fail.diff_image.expect("diff image generated");
        assert_eq!(*diff_img.get_pixel(1, 1), Rgba([255, 0, 128, 255]));

        let smaller = RgbaImage::from_pixel(8, 10, Rgba([100, 100, 100, 255]));
        let size_report = compare_images(&base, &smaller, ImageDiffTolerance::antialiased());
        assert!(!size_report.matched);
        assert_eq!(size_report.size_mismatch, Some(((10, 10), (8, 10))));
        assert!(size_report.diff_image.is_some());
    }

    #[test]
    fn check_snapshot_image_verifies_writes_failures_and_cleans_stale_artifacts() {
        let dir = unique_temp_dir("snap");
        let baseline = RgbaImage::from_pixel(4, 4, Rgba([20, 40, 80, 255]));

        // Missing baseline writes `.new.png` and returns descriptive Err
        let err = check_snapshot_image(&baseline, &dir, "sample", ImageDiffTolerance::exact())
            .expect_err("missing baseline must fail");
        assert!(err.to_string().contains("missing baseline screenshot"));
        assert!(dir.join("screenshots/sample.new.png").exists());

        // Seed reference PNG, then assert mismatch produces `.diff.png` + `.new.png`
        write_png(&baseline, &dir.join("screenshots/sample.png")).unwrap();
        let mut altered = baseline.clone();
        altered.put_pixel(2, 2, Rgba([255, 255, 255, 255]));
        let mismatch = check_snapshot_image(&altered, &dir, "sample", ImageDiffTolerance::exact())
            .expect_err("altered image must fail");
        assert!(mismatch.to_string().contains("mismatch"));
        assert!(dir.join("screenshots/sample.new.png").exists());
        assert!(dir.join("screenshots/sample.diff.png").exists());

        // Matching check passes and cleans up stale `.new.png` / `.diff.png`
        let ok = check_snapshot_image(&baseline, &dir, "sample", ImageDiffTolerance::exact())
            .expect("matching image should succeed");
        assert!(ok.matched);
        assert!(!dir.join("screenshots/sample.new.png").exists());
        assert!(!dir.join("screenshots/sample.diff.png").exists());

        let _ = fs::remove_dir_all(&dir);
    }
}
