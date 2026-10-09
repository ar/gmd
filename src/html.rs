use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub fn run(args: &[String]) -> Result<(), String> {
    let mut input = None;
    let mut output = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-o" | "--output" if output.is_none() => {
                output = Some(PathBuf::from(it.next().ok_or("-o needs an output path")?));
            }
            _ if !arg.starts_with('-') && input.is_none() => input = Some(PathBuf::from(arg)),
            _ => return Err("usage: gmd html <file.md> [-o <file.html>]".into()),
        }
    }
    let input = input.ok_or("usage: gmd html <file.md> [-o <file.html>]")?;
    let output = output.unwrap_or_else(|| input.with_extension("html"));
    let canonical = fs::canonicalize(&input).map_err(|e| format!("{}: {e}", input.display()))?;
    if fs::canonicalize(&output).is_ok_and(|p| p == canonical) {
        return Err("output must differ from the Markdown input".into());
    }
    let markdown = fs::read_to_string(&input).map_err(|e| format!("{}: {e}", input.display()))?;
    let body = render(&markdown)?;
    fs::write(&output, document(&body)).map_err(|e| format!("{}: {e}", output.display()))?;
    println!("Wrote {}", output.display());
    Ok(())
}

pub fn render(markdown: &str) -> Result<String, String> {
    convert(markdown, "html5").map(|html| inline(&html))
}

pub fn plain(markdown: &str) -> Result<String, String> {
    convert(markdown, "plain")
}

fn convert(markdown: &str, format: &str) -> Result<String, String> {
    let folder = crate::stage(&[])?;
    let result = (|| {
        let filter = folder.join("email.lua");
        fs::write(&filter, include_str!("email.lua")).map_err(|e| e.to_string())?;
        let mut child = Command::new("pandoc")
            .args([
                "--from",
                "gfm-raw_html",
                "--to",
                format,
                "--highlight-style",
                "pygments",
                "--wrap=none",
                "--lua-filter",
            ])
            .arg(&filter)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("pandoc: {e}; install Pandoc (macOS: brew install pandoc)"))?;
        let mut stdin = child.stdin.take().unwrap();
        let markdown = markdown.to_owned();
        let writer = std::thread::spawn(move || stdin.write_all(markdown.as_bytes()));
        let result = child.wait_with_output().map_err(|e| e.to_string());
        let written = writer.join().map_err(|_| "could not write to Pandoc")?;
        let result = result?;
        if !result.status.success() {
            return Err("Pandoc could not render the Markdown".into());
        }
        written.map_err(|e| e.to_string())?;
        String::from_utf8(result.stdout).map_err(|e| e.to_string())
    })();
    let _ = fs::remove_dir_all(folder);
    result
}

// Pandoc generates these tags and token classes. Raw HTML input is disabled.
// Apply styles directly to elements so copying does not depend on a stylesheet.
fn inline(html: &str) -> String {
    let mut out = String::from("<div style=\"font-family:Arial,Helvetica,sans-serif;font-size:14px;line-height:1.5;color:#202124;\">");
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find('>') else { break };
        let tag = &rest[..=end];
        let name = tag[1..]
            .split(|c: char| c.is_whitespace() || c == '>')
            .next()
            .unwrap_or("");
        let style = match name {
            "h1" => "font-size:26px;line-height:1.2;margin:20px 0 12px;",
            "h2" => "font-size:21px;line-height:1.3;margin:18px 0 10px;",
            "h3" | "h4" | "h5" | "h6" => "font-size:17px;margin:16px 0 8px;",
            "p" => "margin:0 0 12px;",
            "pre" => "white-space:pre-wrap;overflow-wrap:anywhere;background-color:#f5f7fa;padding:12px;border:1px solid #e1e4e8;border-radius:4px;font-family:Menlo,Consolas,monospace;font-size:13px;line-height:1.4;",
            "code" => "font-family:Menlo,Consolas,monospace;font-size:13px;",
            "blockquote" => "border-left:3px solid #cbd5e1;padding-left:12px;margin:12px 0;color:#59636e;",
            "table" => "border-collapse:collapse;margin:12px 0;",
            "th" => "border:1px solid #cbd5e1;padding:6px 10px;text-align:left;background-color:#f5f7fa;",
            "td" => "border:1px solid #cbd5e1;padding:6px 10px;text-align:left;",
            "a" => "color:#2459a6;",
            "span" => {
                let class = tag.split_once("class=\"").and_then(|(_, s)| s.split('"').next()).unwrap_or("");
                match class {
                    "kw" | "cf" => "color:#007020;font-weight:bold;",
                    "dt" => "color:#902000;",
                    "dv" | "bn" | "fl" => "color:#40a070;",
                    "ch" | "st" | "vs" | "ss" => "color:#4070a0;",
                    "co" | "cv" => "color:#607880;font-style:italic;",
                    "ot" => "color:#007020;",
                    "al" | "er" => "color:#ff0000;font-weight:bold;",
                    "fu" => "color:#06287e;",
                    "re" | "va" | "sc" => "color:#bb6688;",
                    "op" => "color:#666666;",
                    "bu" | "ex" => "color:#008000;",
                    "pp" => "color:#bc7a00;",
                    "at" => "color:#7d9029;",
                    "im" => "color:#008000;font-weight:bold;",
                    "do" | "an" => "color:#ba2121;font-style:italic;",
                    "in" | "wa" => "color:#607880;font-weight:bold;font-style:italic;",
                    "cn" => "color:#880000;",
                    _ => "",
                }
            }
            _ => "",
        };
        if style.is_empty() {
            out.push_str(tag);
        } else {
            out.push_str(&tag[..1 + name.len()]);
            out.push_str(" style=\"");
            out.push_str(style);
            out.push('"');
            out.push_str(&tag[1 + name.len()..]);
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out.push_str("</div>");
    out
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn document(body: &str) -> String {
    format!("<!doctype html>\n<html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Email</title></head><body>{body}</body></html>\n")
}

pub fn preview(body: &str, url: &str, subject: &str) -> String {
    let mut out = String::new();
    let mut template = include_str!("preview.html");
    while let Some(start) = template.find("{{") {
        out.push_str(&template[..start]);
        let end = template[start..].find("}}").unwrap() + start;
        match &template[start..end + 2] {
            "{{TITLE}}" => out.push_str(&escape(subject)),
            "{{URL}}" => out.push_str(&escape(url)),
            "{{BODY}}" => out.push_str(body),
            _ => unreachable!(),
        }
        template = &template[end + 2..];
    }
    out.push_str(template);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn token_styles_are_inline() {
        let html = inline(
            "<pre\nclass=\"sourceCode\"><code><span class=\"kw\">fn</span> &lt;x&gt;</code></pre>",
        );
        assert!(html.contains("<pre style=\"white-space:pre-wrap;"));
        assert!(html.contains("<span style=\"color:#007020;font-weight:bold;\" class=\"kw\">fn"));
        assert!(html.contains("&lt;x&gt;"));
    }
    #[test]
    fn preview_escapes_subject_and_url() {
        let html = preview(
            "<p>Hello</p>",
            "https://mail.google.com/?a=1&b=2",
            "</title><script>alert(1)</script>",
        );
        assert!(html.contains("&lt;/title&gt;"));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("?a=1&amp;b=2"));
    }
}
