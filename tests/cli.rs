use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "gmd-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn gmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_gmd"))
}

#[test]
fn url_preserves_unicode_and_does_not_inject_parameters() {
    let output = gmd()
        .args([
            "--to",
            "Alice <alice+tag@example.com>",
            "--cc",
            "cc@example.com",
            "--bcc",
            "private@example.com",
            "--subject",
            "Hola &bcc=other@example.com",
            "--body",
            "Línea 1\n100% + #?",
            "--account",
            "2",
            "--ascii",
            "--url",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let url = String::from_utf8(output.stdout).unwrap();
    assert!(url.starts_with(
        "https://mail.google.com/mail/u/2/?view=cm&fs=1&to=Alice%20%3Calice%2Btag%40example.com%3E"
    ));
    assert!(url.contains("&cc=cc%40example.com&bcc=private%40example.com"));
    assert!(url.contains("&su=Hola%20%26bcc%3Dother%40example.com"));
    assert!(url.contains("&body=L%C3%ADnea%201%0A100%25%20%2B%20%23%3F"));
}

#[test]
fn saved_draft_resolves_files_and_merges_flags() {
    let tmp = Temp::new();
    fs::write(tmp.0.join("report.txt"), "report").unwrap();
    fs::write(tmp.0.join("message.gmd"), "To: first@example.com\r\nSubject: Original\r\nAttach: report.txt\r\n\r\nHello\r\nworld\r\n").unwrap();
    let output = gmd()
        .arg(tmp.0.join("message.gmd"))
        .args([
            "--to",
            "second@example.com",
            "--subject",
            "Revised",
            "--ascii",
            "--url",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    let url = String::from_utf8(output.stdout).unwrap();
    assert!(url.contains("to=first%40example.com%2Csecond%40example.com"));
    assert!(url.contains("su=Revised&body=Hello%0Aworld%0A"));
    assert!(!url.contains("report.txt"));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("NOT included"));
}

#[test]
fn reads_body_from_stdin() {
    let mut child = gmd()
        .args([
            "--to",
            "a@example.com",
            "--body-file",
            "-",
            "--ascii",
            "--url",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"one\n\ntwo")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("body=one%0A%0Atwo"));
}

#[test]
fn invalid_inputs_fail_without_a_url() {
    let tmp = Temp::new();
    for args in [
        vec!["--ascii", "--url"],
        vec![
            "--to",
            "a@example.com",
            "--account",
            "../1",
            "--ascii",
            "--url",
        ],
        vec![
            "--to",
            "a@example.com\nBcc: other@example.com",
            "--ascii",
            "--url",
        ],
        vec![
            "--to",
            "a@example.com",
            "--body",
            "a",
            "--body-file",
            "-",
            "--ascii",
            "--url",
        ],
        vec![
            "--to",
            "a@example.com",
            "--attach",
            "nonexistent-file",
            "--ascii",
            "--url",
        ],
        vec!["--to", "a@example.com", "--subject"],
        vec!["--typo"],
    ] {
        let output = gmd().current_dir(&tmp.0).args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    let output = gmd()
        .args(["--to", "a@example.com", "--ascii", "--url", "--attach"])
        .arg(&tmp.0)
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn init_never_overwrites() {
    let tmp = Temp::new();
    let file = tmp.0.join("mail.gmd");
    assert!(gmd()
        .arg("init")
        .arg(&file)
        .output()
        .unwrap()
        .status
        .success());
    fs::write(&file, "Keep me").unwrap();
    assert!(!gmd()
        .arg("init")
        .arg(&file)
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(fs::read_to_string(file).unwrap(), "Keep me");
}

#[test]
fn malformed_headers_are_reported() {
    let tmp = Temp::new();
    for content in [
        "To: a@example.com",
        "To: a@example.com\nAtttach: report.pdf\n\nBody",
        "To: a@example.com\nSubject: A\nSubject: B\n\nBody",
    ] {
        let file = tmp.0.join("bad.gmd");
        fs::write(&file, content).unwrap();
        assert!(!gmd()
            .arg(file)
            .args(["--ascii", "--url"])
            .output()
            .unwrap()
            .status
            .success());
    }
}

#[test]
fn print_rejects_invalid_arguments_and_source_overwrite() {
    let tmp = Temp::new();
    let source = tmp.0.join("source.pdf");
    fs::write(&source, "# Markdown in a file with a PDF extension").unwrap();
    for args in [
        vec!["print"],
        vec!["print", "missing.md", "-o"],
        vec!["print", "missing.md", "--typo"],
    ] {
        assert!(!gmd().args(args).output().unwrap().status.success());
    }
    let output = gmd().arg("print").arg(&source).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("must differ"));
    assert!(fs::read_to_string(source)
        .unwrap()
        .starts_with("# Markdown"));
}

#[test]
#[ignore = "requires Typst and its cached cmarker 0.1.10 package"]
fn print_compiles_markdown_with_relative_images_and_preserves_pdf_on_failure() {
    let tmp = Temp::new();
    fs::write(tmp.0.join("image.svg"), r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect width="20" height="20" fill="red"/></svg>"#).unwrap();
    let source = tmp.0.join("notes.md");
    let pdf = tmp.0.join("notes.pdf");
    fs::write(&source, "# Notes\n\n![Local image](image.svg)\n\n```rust\nfn main() { println!(\"Hello\"); }\n```\n\n<!--raw-typst #panic(\"must not execute\") -->").unwrap();
    let output = gmd().arg("print").arg(&source).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let original = fs::read(&pdf).unwrap();
    assert!(original.starts_with(b"%PDF-"));
    fs::write(&source, "![Missing](missing.svg)").unwrap();
    let output = gmd().arg("print").arg(&source).output().unwrap();
    assert!(!output.status.success());
    assert_eq!(fs::read(pdf).unwrap(), original);
}

#[test]
fn markdown_cannot_be_encoded_in_a_compose_url() {
    let output = gmd()
        .args(["--to", "a@example.com", "--url"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("cannot carry HTML"));
}

#[test]
fn ascii_preserves_markdown_and_unicode_without_pandoc() {
    let output = gmd()
        .env("PATH", "")
        .args([
            "--to",
            "a@example.com",
            "--body",
            "**Café**",
            "--ascii",
            "--url",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("body=%2A%2ACaf%C3%A9%2A%2A"));
    for flags in [["--ascii", "--markdown"], ["--markdown", "--ascii"]] {
        let output = gmd()
            .args(["--to", "a@example.com"])
            .args(flags)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("cannot be combined"));
    }
}

#[test]
#[ignore = "requires Pandoc"]
fn html_export_has_inline_highlighting_and_no_active_markdown_content() {
    let tmp = Temp::new();
    let source = tmp.0.join("notes.md");
    let html = tmp.0.join("notes.html");
    fs::write(&source, "# Hola é\n\n```rust\nfn main() {}\n```\n\n<script>alert(1)</script>\n\n[bad](javascript:alert%281%29)\n\n[good](https://example.com)\n\n![a long caption](local.png)\n\n```unknown_language\n<literal>\n```").unwrap();
    let output = gmd().arg("html").arg(&source).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body = fs::read_to_string(&html).unwrap();
    assert!(body.contains("color:#007020;font-weight:bold;"));
    assert!(body.contains("Hola é"));
    assert!(!body.contains("<script>"));
    assert!(!body.contains("href=\"javascript:"));
    assert!(body.contains("href=\"https://example.com\""));
    assert!(body.contains("[Image: a long caption]"));
    assert!(!body.contains("<img"));
    assert!(body.contains("&lt;literal&gt;"));
    let output = gmd()
        .arg("html")
        .arg(&source)
        .arg("-o")
        .arg(&source)
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
#[ignore = "requires Pandoc"]
fn shell_fences_get_highlighted_and_code_lines_remain_intact() {
    let tmp = Temp::new();
    let source = tmp.0.join("shell.md");
    fs::write(
        &source,
        "```shell\n# greeting\necho \"hello\"\nprintf '%s\\n' \"world\"\n```\n",
    )
    .unwrap();
    let output = gmd().arg("html").arg(&source).output().unwrap();
    assert!(output.status.success());
    let html = fs::read_to_string(source.with_extension("html")).unwrap();
    assert!(html.contains("sourceCode bash"));
    assert!(html.contains("color:#4070a0;"));
    assert!(html.contains("# greeting"));
    assert!(html.contains("world"));
    assert!(html.contains("white-space:pre-wrap"));
}

#[test]
#[cfg(unix)]
#[ignore = "requires Pandoc; uses a mock application opener"]
fn markdown_is_default_and_preview_keeps_html_out_of_gmail_url() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = Temp::new();
    for name in ["open", "xdg-open"] {
        let path = tmp.0.join(name);
        fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let path = format!("{}:{}", tmp.0.display(), std::env::var("PATH").unwrap());
    let output = gmd()
        .env("PATH", path)
        .args([
            "--to",
            "a@example.com",
            "--cc",
            "c@example.com",
            "--bcc",
            "b@example.com",
            "--subject",
            "Hello {{BODY}}",
            "--body",
            "**Bold**",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    let path = PathBuf::from(
        stderr
            .lines()
            .find_map(|line| line.strip_prefix("HTML preview: "))
            .unwrap(),
    );
    let html = fs::read_to_string(&path).unwrap();
    assert!(html.contains("<strong>Bold</strong>"));
    assert!(html.contains("Hello {{BODY}}"));
    assert!(html.contains("&amp;to=a%40example.com&amp;cc=c%40example.com&amp;bcc=b%40example.com"));
    assert!(html.contains("&amp;body=Bold%0A"));
    assert!(!html.contains("&amp;body=%3C"));
    assert!(html.contains("Copying does not fill Gmail automatically"));
    assert!(path.parent().unwrap().join("email.html").exists());
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
