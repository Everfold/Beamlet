//! Self-contained offline HTML review report generator for batch preview runs.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use anyhow::Context as _;
use serde::Serialize;

use crate::inspect::{A11yIssue, ContrastIssue, IssueSeverity, LayoutIssue};

/// Single rendered frame and its diagnostics included in an offline HTML preview report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PreviewReportEntry {
    /// Human-readable entry label / artifact base name.
    pub name: String,
    /// Preview scene identifier.
    pub scene: String,
    /// Theme identifier (e.g. `"md3"`, `"fluent"`, `"hig"`, `"adwaita"`).
    pub theme: String,
    /// Brightness mode (e.g. `"light"`, `"dark"`, `"light_hc"`).
    pub brightness: String,
    /// Rendered pixel viewport `(width, height)`.
    pub viewport: (u32, u32),
    /// Configured scale factor (`1.0` = 96 DPI).
    pub scale_factor: f64,
    /// Frame build/render latency in milliseconds.
    pub elapsed_ms: u128,
    /// Relative path from the HTML file to the rendered PNG image.
    pub image_path: String,
    /// Optional relative path from the HTML file to a diff PNG image.
    pub diff_image_path: Option<String>,
    /// Layout diagnostics reported for this frame.
    pub layout_issues: Vec<LayoutIssue>,
    /// WCAG 2.1 contrast diagnostics reported for this frame.
    pub contrast_issues: Vec<ContrastIssue>,
    /// AccessKit accessibility diagnostics reported for this frame.
    pub a11y_issues: Vec<A11yIssue>,
    /// Optional indented widget layout tree dump.
    pub tree_text: Option<String>,
}

fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Renders a zero-external-dependency HTML review grid summarizing screenshots and lint diagnostics.
#[must_use]
pub fn generate_html_report(title: &str, entries: &[PreviewReportEntry]) -> String {
    let total_frames = entries.len();
    let mut layout_errors = 0usize;
    let mut layout_warnings = 0usize;
    let mut contrast_count = 0usize;
    let mut a11y_errors = 0usize;
    let mut themes = BTreeSet::new();
    let mut brightnesses = BTreeSet::new();

    for entry in entries {
        themes.insert(entry.theme.as_str());
        brightnesses.insert(entry.brightness.as_str());
        for issue in &entry.layout_issues {
            match issue.severity {
                IssueSeverity::Error => layout_errors += 1,
                IssueSeverity::Warning => layout_warnings += 1,
            }
        }
        contrast_count += entry.contrast_issues.len();
        a11y_errors += entry.a11y_issues.len();
    }

    let mut html = String::with_capacity(8192);
    let escaped_title = escape_html(title);

    let _ = write!(
        html,
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{escaped_title}</title>
<style>
  :root {{
    color-scheme: light dark;
    --bg: #f4f5f7;
    --surface: #ffffff;
    --surface-alt: #f0f2f5;
    --border: #d7dce3;
    --fg: #18202c;
    --muted: #5a6577;
    --accent: #2563eb;
    --ok: #15803d;
    --warn: #b45309;
    --err: #b91c1c;
  }}
  @media (prefers-color-scheme: dark) {{
    :root {{
      --bg: #0f141c;
      --surface: #18202c;
      --surface-alt: #1f2937;
      --border: #2c384a;
      --fg: #e5e9f0;
      --muted: #9aa5b5;
      --accent: #60a5fa;
      --ok: #4ade80;
      --warn: #fbbf24;
      --err: #f87171;
    }}
  }}
  * {{ box-sizing: border-box; }}
  body {{
    margin: 0;
    font: 14px/1.45 system-ui, -apple-system, sans-serif;
    background: var(--bg);
    color: var(--fg);
  }}
  header {{
    position: sticky;
    top: 0;
    z-index: 10;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    padding: 14px 20px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }}
  .top-row {{
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }}
  h1 {{ margin: 0; font-size: 18px; }}
  .stats {{
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }}
  .badge {{
    display: inline-flex;
    align-items: center;
    padding: 2px 8px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 600;
    border: 1px solid var(--border);
    background: var(--surface-alt);
  }}
  .badge.ok {{ color: var(--ok); }}
  .badge.warn {{ color: var(--warn); }}
  .badge.err {{ color: var(--err); }}
  .controls {{
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: center;
  }}
  input[type="search"], select {{
    padding: 6px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--surface-alt);
    color: var(--fg);
    font: inherit;
  }}
  label.toggle {{
    display: inline-flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    user-select: none;
  }}
  main {{
    padding: 20px;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
    gap: 16px;
  }}
  .card {{
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }}
  .card.hidden {{ display: none; }}
  .preview-frame {{
    position: relative;
    background: var(--surface-alt);
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 200px;
    max-height: 320px;
    overflow: hidden;
    cursor: zoom-in;
  }}
  .preview-frame img {{
    max-width: 100%;
    max-height: 320px;
    object-fit: contain;
    display: block;
  }}
  .card-body {{
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }}
  .card-title {{
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
  }}
  .card-title strong {{ font-size: 14px; word-break: break-word; }}
  .meta {{ font-size: 12px; color: var(--muted); }}
  .diff-btn {{
    align-self: flex-start;
    padding: 3px 8px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--surface-alt);
    color: var(--fg);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }}
  ul.issues {{
    margin: 4px 0 0;
    padding-left: 18px;
    font-size: 12px;
  }}
  ul.issues li.err {{ color: var(--err); }}
  ul.issues li.warn {{ color: var(--warn); }}
  details pre {{
    margin: 6px 0 0;
    padding: 8px;
    border-radius: 6px;
    background: var(--surface-alt);
    overflow: auto;
    max-height: 240px;
    font-size: 11px;
    line-height: 1.35;
  }}
  dialog#lightbox {{
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    color: var(--fg);
    max-width: 94vw;
    max-height: 92vh;
    padding: 12px;
  }}
  dialog#lightbox::backdrop {{
    background: rgba(0, 0, 0, 0.72);
  }}
  .lightbox-bar {{
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    margin-bottom: 8px;
  }}
  dialog#lightbox img {{
    max-width: 90vw;
    max-height: 82vh;
    display: block;
  }}
</style>
</head>
<body>
<header>
  <div class="top-row">
    <h1>{escaped_title}</h1>
    <div class="stats">
      <span class="badge">Frames: {total_frames}</span>
      <span class="badge {layout_err_cls}">Layout Errors: {layout_errors}</span>
      <span class="badge {layout_warn_cls}">Layout Warnings: {layout_warnings}</span>
      <span class="badge {contrast_cls}">Contrast: {contrast_count}</span>
      <span class="badge {a11y_cls}">A11y: {a11y_errors}</span>
    </div>
  </div>
  <div class="controls">
    <input id="filter-search" type="search" placeholder="Filter scene, name, or issue text...">
    <select id="filter-theme">
      <option value="">All themes</option>
"#,
        layout_err_cls = if layout_errors > 0 { "err" } else { "ok" },
        layout_warn_cls = if layout_warnings > 0 { "warn" } else { "ok" },
        contrast_cls = if contrast_count > 0 { "err" } else { "ok" },
        a11y_cls = if a11y_errors > 0 { "err" } else { "ok" },
    );

    for theme in themes {
        let t = escape_html(theme);
        let _ = writeln!(html, "      <option value=\"{t}\">{t}</option>");
    }

    html.push_str(
        r#"    </select>
    <select id="filter-brightness">
      <option value="">All modes</option>
"#,
    );

    for mode in brightnesses {
        let m = escape_html(mode);
        let _ = writeln!(html, "      <option value=\"{m}\">{m}</option>");
    }

    html.push_str(
        r#"    </select>
    <label class="toggle"><input id="filter-issues" type="checkbox"> Issues only</label>
  </div>
</header>
<main id="grid">
"#,
    );

    for entry in entries {
        let err_n = entry
            .layout_issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Error)
            .count();
        let warn_n = entry.layout_issues.len() - err_n;
        let has_issues = !entry.layout_issues.is_empty()
            || !entry.contrast_issues.is_empty()
            || !entry.a11y_issues.is_empty();

        let name = escape_html(&entry.name);
        let scene = escape_html(&entry.scene);
        let theme = escape_html(&entry.theme);
        let brightness = escape_html(&entry.brightness);
        let img = escape_html(&entry.image_path);

        let _ = writeln!(
            html,
            "  <article class=\"card\" data-theme=\"{theme}\" data-brightness=\"{brightness}\" data-issues=\"{}\">",
            usize::from(has_issues)
        );
        let _ = writeln!(
            html,
            "    <div class=\"preview-frame\" data-lightbox-src=\"{img}\" data-lightbox-title=\"{name}\">"
        );
        let _ = writeln!(
            html,
            "      <img src=\"{img}\" alt=\"{name}\" loading=\"lazy\">"
        );
        html.push_str("    </div>\n    <div class=\"card-body\">\n");
        let _ = writeln!(
            html,
            "      <div class=\"card-title\"><strong>{name}</strong><span class=\"meta\">{}ms</span></div>",
            entry.elapsed_ms
        );
        let _ = writeln!(
            html,
            "      <div class=\"meta\">{scene} &middot; {theme} &middot; {brightness} &middot; {}x{} @{:.1}x</div>",
            entry.viewport.0, entry.viewport.1, entry.scale_factor
        );

        let _ = writeln!(html, "      <div class=\"stats\">");
        let _ = writeln!(
            html,
            "        <span class=\"badge {}\">Layout {err_n}E/{warn_n}W</span>",
            if err_n > 0 {
                "err"
            } else if warn_n > 0 {
                "warn"
            } else {
                "ok"
            }
        );
        let _ = writeln!(
            html,
            "        <span class=\"badge {}\">Contrast {}</span>",
            if entry.contrast_issues.is_empty() {
                "ok"
            } else {
                "err"
            },
            entry.contrast_issues.len()
        );
        let _ = writeln!(
            html,
            "        <span class=\"badge {}\">A11y {}</span>",
            if entry.a11y_issues.is_empty() {
                "ok"
            } else {
                "err"
            },
            entry.a11y_issues.len()
        );
        html.push_str("      </div>\n");

        if let Some(diff_path) = &entry.diff_image_path {
            let diff_escaped = escape_html(diff_path);
            let _ = writeln!(
                html,
                "      <button type=\"button\" class=\"diff-btn\" data-base-src=\"{img}\" data-diff-src=\"{diff_escaped}\">Show diff overlay</button>"
            );
        }

        if has_issues {
            html.push_str("      <ul class=\"issues\">\n");
            for issue in &entry.layout_issues {
                let cls = if issue.severity == IssueSeverity::Error {
                    "err"
                } else {
                    "warn"
                };
                let kind = escape_html(issue.kind);
                let msg = escape_html(&issue.message);
                let _ = writeln!(
                    html,
                    "        <li class=\"{cls}\">[layout:{kind}] {msg}</li>"
                );
            }
            for issue in &entry.contrast_issues {
                let pair = escape_html(issue.pair);
                let _ = writeln!(
                    html,
                    "        <li class=\"err\">[contrast] {pair}: {:.2}:1 (&lt; {:.2}:1)</li>",
                    issue.ratio, issue.minimum
                );
            }
            for issue in &entry.a11y_issues {
                let cls = if issue.severity == IssueSeverity::Error {
                    "err"
                } else {
                    "warn"
                };
                let kind = escape_html(issue.kind);
                let msg = escape_html(&issue.message);
                let _ = writeln!(html, "        <li class=\"{cls}\">[a11y:{kind}] {msg}</li>");
            }
            html.push_str("      </ul>\n");
        }

        if let Some(tree) = &entry.tree_text {
            let tree_escaped = escape_html(tree);
            let _ = writeln!(
                html,
                "      <details><summary class=\"meta\">Layout tree</summary><pre>{tree_escaped}</pre></details>"
            );
        }

        html.push_str("    </div>\n  </article>\n");
    }

    html.push_str(
        r#"</main>
<dialog id="lightbox">
  <div class="lightbox-bar">
    <strong id="lightbox-title"></strong>
    <button type="button" id="lightbox-close" class="diff-btn">Close</button>
  </div>
  <img id="lightbox-img" alt="">
</dialog>
<script>
(() => {
  const search = document.getElementById('filter-search');
  const theme = document.getElementById('filter-theme');
  const brightness = document.getElementById('filter-brightness');
  const issuesOnly = document.getElementById('filter-issues');
  const cards = Array.from(document.querySelectorAll('.card'));
  const apply = () => {
    const q = search.value.trim().toLowerCase();
    const t = theme.value;
    const b = brightness.value;
    const only = issuesOnly.checked;
    for (const card of cards) {
      const okTheme = !t || card.dataset.theme === t;
      const okBright = !b || card.dataset.brightness === b;
      const okIssues = !only || card.dataset.issues === '1';
      const okSearch = !q || card.textContent.toLowerCase().includes(q);
      card.classList.toggle('hidden', !(okTheme && okBright && okIssues && okSearch));
    }
  };
  search.addEventListener('input', apply);
  theme.addEventListener('change', apply);
  brightness.addEventListener('change', apply);
  issuesOnly.addEventListener('change', apply);

  const dlg = document.getElementById('lightbox');
  const dlgImg = document.getElementById('lightbox-img');
  const dlgTitle = document.getElementById('lightbox-title');
  document.getElementById('lightbox-close').addEventListener('click', () => dlg.close());
  dlg.addEventListener('click', (e) => { if (e.target === dlg) dlg.close(); });
  for (const frame of document.querySelectorAll('.preview-frame')) {
    frame.addEventListener('click', () => {
      const img = frame.querySelector('img');
      dlgImg.src = img ? img.src : frame.dataset.lightboxSrc;
      dlgTitle.textContent = frame.dataset.lightboxTitle;
      dlg.showModal();
    });
  }
  for (const btn of document.querySelectorAll('.diff-btn[data-diff-src]')) {
    btn.addEventListener('click', () => {
      const img = btn.closest('.card').querySelector('.preview-frame img');
      const showingDiff = img.dataset.mode === 'diff';
      img.src = showingDiff ? btn.dataset.baseSrc : btn.dataset.diffSrc;
      img.dataset.mode = showingDiff ? 'base' : 'diff';
      btn.textContent = showingDiff ? 'Show diff overlay' : 'Show rendered image';
    });
  }
})();
</script>
</body>
</html>
"#,
    );

    html
}

/// Generates an offline HTML preview report and writes it to `path`.
pub fn write_html_report(
    path: impl AsRef<Path>,
    title: &str,
    entries: &[PreviewReportEntry],
) -> anyhow::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed creating report directory {}", parent.display()))?;
    }
    let html = generate_html_report(title, entries);
    fs::write(path, html)
        .with_context(|| format!("failed writing HTML report to {}", path.display()))
}
