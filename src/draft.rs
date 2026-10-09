use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default, Debug)]
pub struct Draft {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub attachments: Vec<PathBuf>,
}

impl Draft {
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let text = text.replace("\r\n", "\n");
        let (headers, body) = text
            .split_once("\n\n")
            .ok_or("draft needs a blank line between headers and body")?;
        let mut draft = Self {
            body: body.into(),
            ..Self::default()
        };
        let mut subject_seen = false;
        for (i, line) in headers.lines().enumerate() {
            let (key, value) = line
                .split_once(':')
                .ok_or_else(|| format!("line {}: expected Header: value", i + 1))?;
            let value = value.trim();
            match key.to_ascii_lowercase().as_str() {
                "to" => draft.to.push(value.into()),
                "cc" => draft.cc.push(value.into()),
                "bcc" => draft.bcc.push(value.into()),
                "subject" if !subject_seen => {
                    draft.subject = value.into();
                    subject_seen = true;
                }
                "attach" if value.is_empty() => {}
                "attach" => draft
                    .attachments
                    .push(path.parent().unwrap_or(Path::new(".")).join(value)),
                _ => {
                    return Err(format!(
                        "line {}: unknown or repeated header {key:?}",
                        i + 1
                    ))
                }
            }
        }
        Ok(draft)
    }

    pub fn validate(&mut self) -> Result<(), String> {
        if self.to.iter().all(|v| v.trim().is_empty()) {
            return Err("specify at least one To recipient".into());
        }
        for value in self
            .to
            .iter()
            .chain(&self.cc)
            .chain(&self.bcc)
            .chain(std::iter::once(&self.subject))
        {
            if value.chars().any(char::is_control) {
                return Err(
                    "recipient and subject fields cannot contain control characters".into(),
                );
            }
        }
        let mut attachments = Vec::new();
        for path in &self.attachments {
            let full = fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))?;
            if !full.is_file() {
                return Err(format!(
                    "{}: attachment must be a regular file",
                    path.display()
                ));
            }
            fs::File::open(&full).map_err(|e| format!("{}: {e}", path.display()))?;
            if !attachments.contains(&full) {
                attachments.push(full);
            }
        }
        self.attachments = attachments;
        Ok(())
    }

    pub fn url(&self, account: &str) -> String {
        let mut url = format!("https://mail.google.com/mail/u/{account}/?view=cm&fs=1");
        for (key, value) in [
            ("to", self.to.join(",")),
            ("cc", self.cc.join(",")),
            ("bcc", self.bcc.join(",")),
            ("su", self.subject.clone()),
            ("body", self.body.clone()),
        ] {
            if !value.is_empty() {
                url.push('&');
                url.push_str(key);
                url.push('=');
                url.push_str(&encode(&value));
            }
        }
        url
    }
}

fn encode(text: &str) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(byte as char);
        } else {
            write!(out, "%{byte:02X}").unwrap();
        }
    }
    out
}

pub const TEMPLATE: &str =
    "To: recipient@example.com\nCc:\nBcc:\nSubject: Hello\nAttach:\n\nWrite your message here.\n";
