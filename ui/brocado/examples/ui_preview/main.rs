//! Offline multi-theme GUI preview CLI for rendering screenshots, geometry trees, and lint reports.

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context as _, bail};
use brocado::prelude::*;
use brocado_preview::harness::{PreviewConfig, PreviewStep};
use brocado_preview::inspect::{IssueSeverity, format_tree_text};
use brocado_preview::report::{PreviewReportEntry, write_html_report};
use clap::Parser;
use image::ImageFormat;

#[allow(dead_code)]
#[path = "../../tests/preview_support/mod.rs"]
mod preview_support;

use preview_support::scenes::{ALL_SCENES, SceneOutput, build_scene_harness, run_scene_preview};
use preview_support::{lint_spec_contrast, lint_ui_accessibility, lint_ui_layout};

#[derive(Debug, Parser)]
#[command(
    name = "ui_preview",
    about = "Render Brocado scenes offline to PNG + Layout JSON/Tree + Lint diagnostics"
)]
struct Cli {
    /// List available preview scene IDs and exit.
    #[arg(long)]
    list: bool,

    /// Comma-separated scene IDs or "all" (default: workspace).
    #[arg(long, default_value = "workspace")]
    scene: String,

    /// Comma-separated themes: md3, fluent, hig, adwaita, or "all".
    #[arg(long, default_value = "all")]
    themes: String,

    /// Comma-separated brightness modes: light, dark, or "all".
    #[arg(long, default_value = "light")]
    brightness: String,

    /// Enable high-contrast mode.
    #[arg(long)]
    high_contrast: bool,

    /// Viewport preset (compact, medium, desktop) or WxH pixels (e.g. 1180x800).
    #[arg(long, default_value = "1180x800")]
    size: String,

    /// Display scale factor (e.g. 1.0 or 2.0).
    #[arg(long, default_value_t = 1.0)]
    scale: f64,

    /// Repeatable interaction steps: click:<label>, focus:<Type>, type:<text>, animate:<ms>, key:escape.
    #[arg(long = "step")]
    steps: Vec<String>,

    /// Emit `<name>.tree.txt` and `<name>.layout.json` alongside each PNG.
    #[arg(long)]
    dump_tree: bool,

    /// Print layout and WCAG contrast diagnostics for every rendered frame.
    #[arg(long)]
    lint: bool,

    /// Exit with non-zero status if any layout or contrast issue is detected.
    #[arg(long)]
    fail_on_lint: bool,

    /// Skip settling entrance animations (e.g. SlideTransition) before capturing the frame.
    #[arg(long)]
    no_settle: bool,

    /// Skip generating `<out>/index.html` review report.
    #[arg(long)]
    no_html: bool,

    /// Output directory for preview artifacts.
    #[arg(long, default_value = "target/previews")]
    out: PathBuf,
}

fn parse_size(spec: &str) -> anyhow::Result<(u32, u32)> {
    match spec.to_ascii_lowercase().as_str() {
        "compact" => return Ok((480, 800)),
        "medium" => return Ok((760, 720)),
        "desktop" => return Ok((1180, 800)),
        _ => {}
    }
    let (w_str, h_str) = spec
        .split_once('x')
        .or_else(|| spec.split_once('X'))
        .with_context(|| {
            format!("invalid size '{spec}'; expected WxH, compact, medium, or desktop")
        })?;
    let w = w_str.trim().parse::<u32>()?;
    let h = h_str.trim().parse::<u32>()?;
    anyhow::ensure!(w > 0 && h > 0, "viewport dimensions must be positive");
    Ok((w, h))
}

fn parse_scenes(spec: &str) -> anyhow::Result<Vec<&'static str>> {
    if spec.eq_ignore_ascii_case("all") {
        return Ok(ALL_SCENES.iter().map(|s| s.id).collect());
    }
    let mut out = Vec::new();
    for raw in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let found = ALL_SCENES
            .iter()
            .find(|s| s.id == raw)
            .with_context(|| format!("unknown scene '{raw}' (use --list)"))?;
        out.push(found.id);
    }
    anyhow::ensure!(!out.is_empty(), "at least one scene is required");
    Ok(out)
}

fn parse_themes(spec: &str) -> anyhow::Result<Vec<(DesignLanguage, &'static str)>> {
    if spec.eq_ignore_ascii_case("all") {
        return Ok(vec![
            (DesignLanguage::Md3, "md3"),
            (DesignLanguage::Fluent, "fluent"),
            (DesignLanguage::Hig, "hig"),
            (DesignLanguage::Adwaita, "adwaita"),
        ]);
    }
    let mut out = Vec::new();
    for raw in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let pair = match raw.to_ascii_lowercase().as_str() {
            "md3" => (DesignLanguage::Md3, "md3"),
            "fluent" => (DesignLanguage::Fluent, "fluent"),
            "hig" => (DesignLanguage::Hig, "hig"),
            "adwaita" => (DesignLanguage::Adwaita, "adwaita"),
            other => bail!("unknown theme '{other}' (expected md3, fluent, hig, adwaita, or all)"),
        };
        out.push(pair);
    }
    anyhow::ensure!(!out.is_empty(), "at least one theme is required");
    Ok(out)
}

fn parse_brightness(spec: &str) -> anyhow::Result<Vec<(Brightness, &'static str)>> {
    if spec.eq_ignore_ascii_case("all") {
        return Ok(vec![
            (Brightness::Light, "light"),
            (Brightness::Dark, "dark"),
        ]);
    }
    let mut out = Vec::new();
    for raw in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let pair = match raw.to_ascii_lowercase().as_str() {
            "light" => (Brightness::Light, "light"),
            "dark" => (Brightness::Dark, "dark"),
            other => bail!("unknown brightness '{other}' (expected light, dark, or all)"),
        };
        out.push(pair);
    }
    anyhow::ensure!(!out.is_empty(), "at least one brightness mode is required");
    Ok(out)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.list {
        println!("Available preview scenes:");
        for info in ALL_SCENES {
            println!("  {:<18} {}", info.id, info.description);
        }
        return Ok(());
    }

    let (width, height) = parse_size(&cli.size)?;
    let scenes = parse_scenes(&cli.scene)?;
    let themes = parse_themes(&cli.themes)?;
    let modes = parse_brightness(&cli.brightness)?;
    let steps: Vec<PreviewStep> = cli
        .steps
        .iter()
        .map(|s| PreviewStep::parse(s))
        .collect::<anyhow::Result<_>>()?;

    let config = PreviewConfig {
        width,
        height,
        scale_factor: cli.scale,
        ..PreviewConfig::default()
    };

    fs::create_dir_all(&cli.out)
        .with_context(|| format!("failed creating output directory {}", cli.out.display()))?;

    let mut total_lint_failures = 0usize;
    let mut report_entries = Vec::new();

    for scene_id in &scenes {
        for (lang, lang_slug) in &themes {
            for (brightness, bright_slug) in &modes {
                let mut theme = UiTheme::new(*lang, *brightness);
                theme.high_contrast = cli.high_contrast;
                let hc_suffix = if cli.high_contrast { "_hc" } else { "" };
                let bright_label = format!("{bright_slug}{hc_suffix}");
                let base_name = format!("{scene_id}_{lang_slug}_{bright_label}");

                let t0 = Instant::now();
                let output = if cli.no_settle {
                    let mut h = build_scene_harness(scene_id, theme, config)?;
                    h.run_steps(&steps)?;
                    let image = h.render_image();
                    let tree = brocado_preview::inspect::dump_layout_tree(&mut h);
                    let layout_issues = lint_ui_layout(&tree, h.viewport());
                    let contrast_issues = lint_spec_contrast(&h.state().theme.to_spec());
                    let a11y_issues = lint_ui_accessibility(&mut h);
                    SceneOutput {
                        image,
                        tree,
                        layout_issues,
                        contrast_issues,
                        a11y_issues,
                    }
                } else {
                    run_scene_preview(scene_id, theme, config, &steps)?
                };
                let elapsed_ms = t0.elapsed().as_millis();

                let image_filename = format!("{base_name}.png");
                let png_path = cli.out.join(&image_filename);
                output
                    .image
                    .save_with_format(&png_path, ImageFormat::Png)
                    .with_context(|| format!("failed saving {}", png_path.display()))?;

                let tree_text = if cli.dump_tree {
                    let text = format_tree_text(&output.tree);
                    let tree_path = cli.out.join(format!("{base_name}.tree.txt"));
                    let json_path = cli.out.join(format!("{base_name}.layout.json"));
                    fs::write(&tree_path, &text)?;
                    fs::write(&json_path, serde_json::to_string_pretty(&output.tree)?)?;
                    Some(text)
                } else {
                    None
                };

                let err_count = output
                    .layout_issues
                    .iter()
                    .filter(|i| i.severity == IssueSeverity::Error)
                    .count();
                let warn_count = output.layout_issues.len() - err_count;
                let contrast_count = output.contrast_issues.len();
                let a11y_count = output.a11y_issues.len();

                println!(
                    "[preview] {} ({}ms) -> layout: {} errors / {} warnings, contrast: {} issues, a11y: {} issues",
                    png_path.display(),
                    elapsed_ms,
                    err_count,
                    warn_count,
                    contrast_count,
                    a11y_count,
                );

                if cli.lint || cli.fail_on_lint {
                    for issue in &output.layout_issues {
                        println!(
                            "  [layout:{:?}:{}] {}",
                            issue.severity, issue.kind, issue.message
                        );
                    }
                    for issue in &output.contrast_issues {
                        println!(
                            "  [contrast] {} = {:.2}:1 (< {:.2}:1)",
                            issue.pair, issue.ratio, issue.minimum
                        );
                    }
                    for issue in &output.a11y_issues {
                        println!(
                            "  [a11y:{:?}:{}] {}",
                            issue.severity, issue.kind, issue.message
                        );
                    }
                }

                if cli.fail_on_lint {
                    total_lint_failures += err_count + contrast_count + a11y_count;
                }

                report_entries.push(PreviewReportEntry {
                    name: base_name,
                    scene: (*scene_id).to_string(),
                    theme: lang_slug.to_string(),
                    brightness: bright_label,
                    viewport: (width, height),
                    scale_factor: cli.scale,
                    elapsed_ms,
                    image_path: image_filename,
                    diff_image_path: None,
                    layout_issues: output.layout_issues,
                    contrast_issues: output.contrast_issues,
                    a11y_issues: output.a11y_issues,
                    tree_text,
                });
            }
        }
    }

    if !cli.no_html {
        let report_path = cli.out.join("index.html");
        write_html_report(
            &report_path,
            "Brocado Offline Preview Report",
            &report_entries,
        )?;
        println!("[preview] report -> {}", report_path.display());
    }

    if total_lint_failures > 0 {
        bail!("preview lint check detected {total_lint_failures} error/contrast/a11y failures");
    }

    Ok(())
}
