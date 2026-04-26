use std::time::Duration;

use chromiumoxide::Browser;
use chromiumoxide::browser::BrowserConfig;
use clap::ValueEnum;
use futures::StreamExt;
use serde::Deserialize;

use imgctl_core::{Error, Result};

#[derive(ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[clap(rename_all = "lowercase")]
pub enum MermaidTheme {
    #[default]
    Default,
    Dark,
    Forest,
    Neutral,
}

impl MermaidTheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Dark => "dark",
            Self::Forest => "forest",
            Self::Neutral => "neutral",
        }
    }
}

const RENDER_TIMEOUT: Duration = Duration::from_secs(30);

const HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"></head>
<body>
<div id="container"></div>
<script src="https://cdn.jsdelivr.net/npm/mermaid/dist/mermaid.min.js"></script>
<script>
  mermaid.initialize({ startOnLoad: false, theme: '__THEME__' });
  const src = '__SRC__';
  mermaid.render('graphDiv', src).then(({svg}) => {
    document.getElementById('container').innerHTML = svg;
    const el = document.querySelector('#container svg');
    if (el) el.id = 'diagram';
  }).catch(err => {
    document.body.setAttribute('data-mermaid-error', String(err && err.message ? err.message : err));
  });
</script>
</body></html>"#;

/// Build the HTML page that Chromium will render to produce the SVG.
///
/// `src` is JavaScript-escaped so backticks, backslashes and template
/// interpolation can't break out of the `'...'` literal.
pub fn build_html(src: &str, theme: MermaidTheme) -> String {
    let escaped = js_escape(src);
    HTML_TEMPLATE
        .replacen("__THEME__", theme.as_str(), 1)
        .replacen("__SRC__", &escaped, 1)
}

fn js_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

/// Render Mermaid `src` into an SVG string by driving a headless Chrome.
///
/// `chrome_endpoint`:
///   - `Some(ws)` — connect to an existing CDP-enabled Chrome (cheap, ~50ms)
///   - `None`     — launch a managed Chromium (slow first time, ~2s)
pub async fn render_svg(
    src: &str,
    theme: MermaidTheme,
    chrome_endpoint: Option<&str>,
) -> Result<String> {
    let (mut browser, mut handler) = launch_browser(chrome_endpoint).await?;

    // Drain Handler events; without this the browser hangs.
    let handler_task = tokio::task::spawn(async move {
        while let Some(event) = handler.next().await {
            if event.is_err() {
                break;
            }
        }
    });

    let result = drive_render(&browser, src, theme).await;

    let _ = browser.close().await;
    handler_task.abort();

    result
}

async fn launch_browser(
    endpoint: Option<&str>,
) -> Result<(Browser, chromiumoxide::Handler)> {
    match endpoint {
        Some(ws) => Browser::connect(ws)
            .await
            .map_err(|e| Error::ChromeConnection(e.to_string())),
        None => {
            let cfg = BrowserConfig::builder()
                .build()
                .map_err(|e| Error::ChromeConnection(e.to_string()))?;
            Browser::launch(cfg)
                .await
                .map_err(|e| Error::ChromeConnection(e.to_string()))
        }
    }
}

async fn drive_render(
    browser: &Browser,
    src: &str,
    theme: MermaidTheme,
) -> Result<String> {
    let html = build_html(src, theme);

    let page = browser
        .new_page("about:blank")
        .await
        .map_err(|e| Error::ChromeConnection(e.to_string()))?;
    page.set_content(html)
        .await
        .map_err(|e| Error::ChromeConnection(e.to_string()))?;

    // Race element-ready against syntax-error attribute against timeout.
    let deadline = tokio::time::Instant::now() + RENDER_TIMEOUT;
    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(Error::ChromeTimeout);
        }

        // Check for mermaid render error attribute.
        if let Ok(err_value) = page
            .evaluate("document.body.getAttribute('data-mermaid-error')")
            .await
        {
            if let Ok(Some(msg)) = err_value.into_value::<Option<String>>() {
                return Err(Error::MermaidSyntax(msg));
            }
        }

        // Check whether SVG is ready.
        if let Ok(ready_value) = page.evaluate("!!document.getElementById('diagram')").await {
            if matches!(ready_value.into_value::<bool>(), Ok(true)) {
                break;
            }
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let svg_value = page
        .evaluate("document.getElementById('diagram').outerHTML")
        .await
        .map_err(|e| Error::ChromeConnection(e.to_string()))?;
    let svg: String = svg_value
        .into_value()
        .map_err(|e| Error::ChromeConnection(e.to_string()))?;

    Ok(svg)
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DummyValue;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_strings() {
        assert_eq!(MermaidTheme::Default.as_str(), "default");
        assert_eq!(MermaidTheme::Dark.as_str(), "dark");
        assert_eq!(MermaidTheme::Forest.as_str(), "forest");
        assert_eq!(MermaidTheme::Neutral.as_str(), "neutral");
    }

    #[test]
    fn js_escape_handles_specials() {
        assert_eq!(js_escape("a'b"), "a\\'b");
        assert_eq!(js_escape("a\\b"), "a\\\\b");
        assert_eq!(js_escape("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn build_html_includes_theme_and_src() {
        let html = build_html("flowchart LR; A-->B", MermaidTheme::Dark);
        assert!(html.contains("'dark'"), "theme not injected: {html}");
        assert!(html.contains("flowchart LR; A--&gt;B") || html.contains("flowchart LR; A-->B"));
        assert!(html.contains("mermaid.min.js"));
    }

    #[test]
    fn build_html_escapes_quotes_in_src() {
        // Mermaid source containing single quotes shouldn't break the JS literal.
        let html = build_html("graph LR; A[\"It's a test\"]", MermaidTheme::Default);
        assert!(html.contains("It\\'s a test"), "quotes not escaped: {html}");
    }
}
