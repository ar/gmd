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
            _ => return Err("usage: gmd print <file.md> [-o <file.pdf>]".into()),
        }
    }
    let input = input.ok_or("usage: gmd print <file.md> [-o <file.pdf>]")?;
    let output = output.unwrap_or_else(|| input.with_extension("pdf"));
    let input = fs::canonicalize(&input).map_err(|e| format!("{}: {e}", input.display()))?;
    if output.extension().and_then(|s| s.to_str()) != Some("pdf") {
        return Err("output must have a .pdf extension".into());
    }
    if fs::canonicalize(&output).is_ok_and(|p| p == input) {
        return Err("output must differ from the Markdown input".into());
    }
    let markdown = fs::read_to_string(&input).map_err(|e| format!("{}: {e}", input.display()))?;
    let source = render(
        &markdown,
        input.file_name().unwrap().to_string_lossy().as_ref(),
    );
    // Compile to memory so a Typst failure cannot truncate an existing PDF.
    // Running at the document's directory makes relative image paths work.
    let mut child = Command::new("typst")
        .args(["compile", "--format", "pdf", "-", "-"])
        .current_dir(input.parent().unwrap())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("typst: {e}; install Typst (macOS: brew install typst)"))?;
    // Write concurrently with stdout consumption to avoid pipe deadlock on large documents.
    let mut stdin = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || stdin.write_all(source.as_bytes()));
    let result = child.wait_with_output().map_err(|e| format!("typst: {e}"));
    let written = writer.join().map_err(|_| "could not write to Typst")?;
    let result = result?;
    if !result.status.success() {
        return Err("Typst could not render the Markdown; output left unchanged".into());
    }
    written.map_err(|e| format!("typst: {e}"))?;
    fs::write(&output, result.stdout).map_err(|e| format!("{}: {e}", output.display()))?;
    println!("Wrote {}", output.display());
    Ok(())
}

fn literal(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                use std::fmt::Write;
                write!(out, "\\u{{{:x}}}", c as u32).unwrap();
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn render(markdown: &str, name: &str) -> String {
    format!(
        r##"#import "@preview/cmarker:0.1.10"
#set page(paper: "a4", margin: 2cm, footer: context [
  #set text(size: 8pt, fill: luma(45%))
  {name} #h(1fr) #counter(page).display("1 / 1", both: true)
])
#set text(font: ("Helvetica Neue", "Libertinus Serif"), size: 10pt)
#set par(leading: 0.65em)
#set heading(numbering: none)
#set raw(theme: auto, tab-size: 4)
#show raw: set text(font: ("Menlo", "DejaVu Sans Mono"), size: 8.5pt)
#show raw.where(block: true): block.with(
  width: 100%, fill: rgb("#f5f7fa"), inset: 10pt, radius: 4pt, breakable: true,
)
#show raw.where(block: false): box.with(fill: luma(95%), inset: (x: 2pt, y: 0pt), radius: 2pt)
#show link: set text(fill: rgb("#2459a6"))
#show table: set table(stroke: 0.4pt + luma(80%), inset: 6pt)
#cmarker.render({markdown}, raw-typst: false, set-document-title: false,
  scope: (image: (source, ..args) => image(source, ..args)),
)
"##,
        name = format_args!("#{}", literal(name)),
        markdown = literal(markdown),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_is_passed_as_data() {
        assert_eq!(literal("\"\\\n\r\t\0é"), "\"\\\"\\\\\\n\\r\\t\\u{0}é\"");
        let source = render("```rust\nfn main() {}\n```", "note.md");
        assert!(source.contains("raw-typst: false"));
        assert!(source.contains("```rust\\nfn main() {}\\n```"));
    }
}
