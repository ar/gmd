mod draft;
mod html;
mod print;

use draft::Draft;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command};

const USAGE: &str = "\
gmd — prepare a Gmail message for review in your browser

Usage:
  gmd init <file>             Create a text draft template (never overwrites)
  gmd <file> [options]        Open a saved draft in Gmail
  gmd [options]              Compose from command-line fields
  gmd print <file.md> [-o <file.pdf>]
                             Typeset Markdown with highlighted code (needs Typst)
  gmd html <file.md> [-o <file.html>]
                             Export HTML email with highlighted code (needs Pandoc)

Options:
  --to <addresses>           To recipients; repeat to add more
  --cc <addresses>           Cc recipients; repeat to add more
  --bcc <addresses>          Bcc recipients; repeat to add more
  --subject <text>           Subject (overrides the draft file)
  --body <text>              Markdown body (overrides the draft file)
  --body-file <path|->       Read body from a UTF-8 file or stdin
  --ascii                   Keep body as plain text instead of rendering Markdown
  --markdown                Explicitly select Markdown (the default)
  -a, --attach <path>        Attachment; repeat for multiple files
  --account <number>        Gmail account index (default: 0)
  --url                     Print compose URL without opening apps (needs --ascii)
  --help                    Show this help
  --version                 Show version

Draft format: To:, Cc:, Bcc:, Subject:, and repeated Attach: headers,
then a blank line and the Markdown body. File attachment paths are
relative to the draft; --attach paths are relative to the current directory.

Attachments cannot be uploaded through a compose URL. gmd opens a folder
of attachment copies alongside Gmail; select them and drag into the draft.
You review the message and click Send yourself. gmd never sends mail.
";

fn main() {
    if let Err(error) = run(env::args().skip(1).collect()) {
        eprintln!("gmd: {error}");
        process::exit(2);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    if args.is_empty() || args == ["--help"] || args == ["-h"] {
        print!("{USAGE}");
        return Ok(());
    }
    if args == ["--version"] {
        println!("gmd {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("print") {
        return print::run(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("html") {
        return html::run(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("init") {
        if args.len() != 2 {
            return Err("usage: gmd init <file>".into());
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&args[1])
            .map_err(|e| format!("{}: {e}", args[1]))?;
        file.write_all(draft::TEMPLATE.as_bytes())
            .map_err(|e| e.to_string())?;
        println!("Created {}", args[1]);
        return Ok(());
    }

    let mut fields = Draft::default();
    let mut file = None;
    let mut subject = None;
    let mut body = None;
    let mut account = "0".to_string();
    let mut url_only = false;
    let mut ascii = false;
    let mut explicit_markdown = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--url" => url_only = true,
            "--ascii" => ascii = true,
            "--markdown" => explicit_markdown = true,
            "--to" | "--cc" | "--bcc" | "--subject" | "--body" | "--body-file" | "--attach"
            | "-a" | "--account" => {
                let value = it.next().ok_or_else(|| format!("{arg} needs a value"))?;
                match arg.as_str() {
                    "--to" => fields.to.push(value.clone()),
                    "--cc" => fields.cc.push(value.clone()),
                    "--bcc" => fields.bcc.push(value.clone()),
                    "--subject" => subject = Some(value.clone()),
                    "--attach" | "-a" => fields.attachments.push(PathBuf::from(value)),
                    "--account" => account = value.clone(),
                    "--body" | "--body-file" => {
                        if body.is_some() {
                            return Err("use only one --body or --body-file".into());
                        }
                        body = Some((arg == "--body-file", value.clone()));
                    }
                    _ => unreachable!(),
                }
            }
            _ if arg.starts_with('-') => {
                return Err(format!("unknown option {arg}; see gmd --help"))
            }
            _ if file.is_none() => file = Some(PathBuf::from(arg)),
            _ => return Err("expected at most one draft file".into()),
        }
    }
    if ascii && explicit_markdown {
        return Err("--ascii and --markdown cannot be combined".into());
    }
    if !ascii && url_only {
        return Err("compose URLs cannot carry HTML; use --ascii --url for plain text, or gmd html to export HTML".into());
    }
    if account.is_empty() || !account.bytes().all(|b| b.is_ascii_digit()) {
        return Err("--account must be a nonnegative Gmail account index (0, 1, ...)".into());
    }
    let mut draft = match file {
        Some(path) => Draft::read(&path)?,
        None => Draft::default(),
    };
    draft.to.extend(fields.to);
    draft.cc.extend(fields.cc);
    draft.bcc.extend(fields.bcc);
    draft.attachments.extend(fields.attachments);
    if let Some(value) = subject {
        draft.subject = value;
    }
    if let Some((from_file, value)) = body {
        draft.body = if !from_file {
            value
        } else if value == "-" {
            let mut text = String::new();
            io::stdin()
                .read_to_string(&mut text)
                .map_err(|e| e.to_string())?;
            text
        } else {
            fs::read_to_string(&value).map_err(|e| format!("{value}: {e}"))?
        };
    }
    draft.validate()?;
    let rendered = if !ascii {
        let body = html::render(&draft.body)?;
        draft.body = html::plain(&draft.body)?;
        Some(body)
    } else {
        None
    };
    let url = draft.url(&account);
    if url.len() > 8000 {
        eprintln!("gmd: long compose URL ({} bytes); Gmail or your browser may reject it. Check the full body after opening.", url.len());
    }
    if url_only {
        if !draft.attachments.is_empty() {
            eprintln!(
                "gmd: {} attachment(s) validated but NOT included in the URL; add them in Gmail.",
                draft.attachments.len()
            );
        }
        println!("{url}");
        return Ok(());
    }
    let folder = if draft.attachments.is_empty() {
        None
    } else {
        Some(stage(&draft.attachments)?)
    };
    if let Some(path) = &folder {
        eprintln!("Attachment copies: {}", path.display());
        eprintln!("Select these files and drag them into Gmail before sending. Delete this folder when finished.");
    }
    if let Some(body) = rendered {
        let preview_folder = stage(&[])?;
        let preview = preview_folder.join("preview.html");
        fs::write(&preview, html::preview(&body, &url, &draft.subject))
            .map_err(|e| e.to_string())?;
        fs::write(preview_folder.join("email.html"), html::document(&body))
            .map_err(|e| e.to_string())?;
        eprintln!("HTML preview: {}", preview.display());
        eprintln!("Gmail opens with a plain-text body. Copy the formatted email, then replace that body by pasting in Gmail. Delete the preview folder when finished.");
        open(&preview).map_err(|e| {
            format!(
                "could not open preview: {e}; open {} manually",
                preview.display()
            )
        })?;
    } else {
        open(url.as_ref())
            .map_err(|e| format!("could not open Gmail: {e}; use --url to obtain the link"))?;
    }
    if let Some(path) = folder {
        open(&path).map_err(|e| {
            format!(
                "Could not open {}: {e}; open this attachment folder manually",
                path.display()
            )
        })?;
    }
    eprintln!(
        "Check the sending account, recipients, body, and attachments in Gmail before sending."
    );
    Ok(())
}

fn open(target: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "linux")]
    let mut command = Command::new("xdg-open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = Command::new("rundll32.exe");
        c.arg("url.dll,FileProtocolHandler");
        c
    };
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    return Err("unsupported platform; use --url".into());
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    {
        let status = command.arg(target).status().map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("browser/file-manager opener exited with {status}"))
        }
    }
}

// A private, uniquely created folder keeps unrelated files out of the drag selection.
fn stage(paths: &[PathBuf]) -> Result<PathBuf, String> {
    let base = env::temp_dir();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let folder = base.join(format!("gmd-{}-{stamp}", process::id()));
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&folder).map_err(|e| e.to_string())?;
    let result = (|| {
        for path in paths {
            let name = path.file_name().ok_or("attachment has no file name")?;
            let mut target = folder.join(name);
            let mut index = 2;
            while target.exists() {
                let mut renamed = std::ffi::OsString::from(format!("{index}-"));
                renamed.push(name);
                target = folder.join(renamed);
                index += 1;
            }
            fs::copy(path, &target).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        Ok(folder.clone())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&folder);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_copies_preserve_contents_and_handle_name_collisions() {
        let fixtures = stage(&[]).unwrap();
        fs::create_dir(fixtures.join("a")).unwrap();
        fs::create_dir(fixtures.join("b")).unwrap();
        let first = fixtures.join("a/report.txt");
        let second = fixtures.join("b/report.txt");
        fs::write(&first, "first report").unwrap();
        fs::write(&second, "second report").unwrap();
        let copies = stage(&[first.clone(), second.clone()]).unwrap();
        assert_eq!(
            fs::read_to_string(copies.join("report.txt")).unwrap(),
            "first report"
        );
        assert_eq!(
            fs::read_to_string(copies.join("2-report.txt")).unwrap(),
            "second report"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&copies).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        fs::write(&first, "changed original").unwrap();
        assert_eq!(
            fs::read_to_string(copies.join("report.txt")).unwrap(),
            "first report"
        );
        fs::remove_dir_all(copies).unwrap();
        assert_eq!(fs::read_to_string(second).unwrap(), "second report");
        fs::remove_dir_all(fixtures).unwrap();
    }
}
