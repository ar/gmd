# gmd — GMail Draft

A small, dependency-free Rust CLI that prepares a message and opens Gmail in your
default browser for review. Uses the browser's Gmail session; no API credentials
or OAuth setup. You click **Send** yourself.

## Install

```sh
cargo install --path .
```

macOS uses `open`, Linux uses `xdg-open`, and Windows uses the system URL handler.
Your default browser must have the Gmail session you want to use.

## Compose

```sh
gmd --to alice@example.com \
    --cc bob@example.com \
    --bcc archive@example.com \
    --subject 'Monthly report' \
    --body-file message.txt \
    --attach report.pdf \
    --attach figures.xlsx
```

Use `--body 'Hello!'` for inline text or `--body-file -` to read stdin. Repeat
`--to`, `--cc`, and `--bcc`, or supply a comma-separated list. Quote display names,
such as `--to 'Alice Smith <alice@example.com>'`. At least one To recipient is
required; other fields are optional. The body is Markdown by default, rendered as
HTML with highlighted code fences. Add `--ascii` to keep the body as plain text
and open Gmail directly. This preserves the original text, including Markdown
markers and Unicode; it does not transliterate or restrict characters to ASCII.
Plain-text mode does not require Pandoc. `--markdown` remains an explicit alias
for the default and cannot be combined with `--ascii`.

## Saved drafts

```sh
gmd init report.gmd
# Edit report.gmd in your editor.
gmd report.gmd
```

The format is ordinary UTF-8 text:

```text
To: Alice <alice@example.com>
Cc: bob@example.com
Bcc:
Subject: Monthly report
Attach: report.pdf
Attach: figures.xlsx

Hi Alice,

Here are this month's report and figures.

Thanks!
```

A blank line separates the headers from the body. Attachment paths are relative
to the draft file. Spaces in these paths do not need quotes. Empty `Cc:`, `Bcc:`,
and `Attach:` lines are allowed; omitted fields are also fine. Header names are
case insensitive. Repeat `Attach:` once per file. Paths are literal: there is no
tilde, environment-variable, or glob expansion inside draft files.

Flags may accompany a file: recipients and attachments are added; subject and body
replace those in the file. Command-line paths are relative to the working directory.
The original draft is never modified. `init` refuses to overwrite an existing file.

## HTML email from Markdown

```sh
gmd --to alice@example.com --subject 'Report' --body-file notes.md
gmd report.gmd               # Render the saved draft's body as Markdown
gmd report.gmd --ascii       # Keep the body as plain text
gmd html notes.md             # Export notes.html without opening applications
gmd html notes.md -o email.html
```

Requires [Pandoc](https://pandoc.org/) (`brew install pandoc` on macOS).
Headings, lists, tables, links, and code fences become formatted HTML. Language
tags such as `rust`, `python`, and `json` select syntax highlighting; unrecognized
or missing tags produce plain code. Formatting and token colors use inline styles.

By default, composing opens a local preview with **Copy formatted email** and **Open Gmail**
buttons. Gmail opens with recipients, subject, and a plain-text version of the
body prefilled. For HTML formatting, copy from the preview, click inside Gmail's
message body, select the existing body text, and replace it using normal paste
(Ctrl+V or Cmd+V). Gmail's plain-text mode must be off. Attach files as usual and
review before sending. Gmail may
adjust formatting; the CLI cannot verify the pasted result.

The preview is a clipboard bridge: Gmail's compose URL carries plain text, while
the copy button puts HTML and plain text on the clipboard. Opening Gmail does not
paste automatically. Browser isolation prevents a local page from editing Gmail.
Removing the manual paste step would require browser automation, an extension,
or Gmail API integration. `--ascii` needs no preview.

If code has no colors in the preview, check the fence label; unlabeled code is
not automatically language-detected. The common `shell` label maps to `bash`.
The preview reports blocks with no colored tokens. If colors are visible in the
preview but disappear in Gmail, check that plain-text mode is off and that you
used normal paste rather than paste-as-plain-text.

Gmail compose URLs cannot carry HTML, so `--url` requires `--ascii`.
The preview has a clipboard fallback: if automatic copying is blocked,
it selects the message for manual copying. Only the message is copied, not the
preview controls. The preview folder also contains `email.html`, a standalone
export without buttons or scripts. Delete this private temporary folder when done.

`gmd html` writes that same standalone HTML directly to your chosen file. Existing
output is replaced after successful rendering. Raw HTML in the Markdown is treated
as text. Links support HTTP, HTTPS, and mailto. Images become `[Image: alt text]`
placeholders; insert any desired images in Gmail. Rendering does not fetch remote
images or embed local files.

## Pretty-print Markdown

```sh
gmd print notes.md                  # Write notes.pdf
gmd print notes.md -o handout.pdf    # Choose the output path
```

Requires [Typst](https://typst.app/) (`brew install typst` on macOS). On first
use, Typst downloads the pinned `cmarker` Markdown package; subsequent runs can
work offline from its cache. No Rust dependencies are added.

The PDF uses A4 pages, page numbers, formatted headings, lists, links, tables, and
shaded code blocks. Add a language after the opening fence, such as `rust`,
`python`, `json`, or `bash`, for syntax highlighting. Unlabeled or unsupported
languages remain plain code. Inline code uses a monospace font. Local image paths
resolve relative to the Markdown file, within that directory; remote images must
be downloaded first. Raw Typst embedded in HTML comments is disabled.

Existing output PDFs are replaced on successful compilation; rendering errors
leave them unchanged. The Markdown source is preserved. Printing creates a PDF
without opening a viewer or sending anything to a physical printer. To email the
result, pass the PDF to `--attach` as usual.

## Attachments

**Gmail compose URLs cannot upload local files.** When attachments are specified,
`gmd` validates them, opens Gmail with the message filled in, and opens a private
temporary folder containing copies of just those files. Select the files and drag
them into Gmail, then wait for upload to finish before sending. You can also use
Gmail's paperclip button and select the files in that folder.

The folder's path is printed in the terminal. Copies remain there so you can finish
the upload after `gmd` exits; delete the folder when finished. Source files are
untouched. Repeated paths are deduplicated; filename collisions receive numeric
prefixes in the temporary folder.

Gmail applies its own attachment size and file-type limits. See
[Google's attachment instructions](https://support.google.com/mail/answer/6584?hl=en).
There is no browser automation or automatic attachment upload in this version.

## Accounts and URL output

```sh
gmd report.gmd --account 1     # Second account in this browser's Gmail session
gmd report.gmd --ascii --url   # Print the plain-text URL; open no apps or folders
```

The account index defaults to `0`. Verify the sending account in Gmail.
`--url` still validates attachments, but they are not part of the URL; a reminder
is printed to stderr. No attachment copies are created in this mode.

Gmail's compose URL parameters are an undocumented web interface, so Google may
change their behavior. Opening a link is not confirmation that Gmail saved a draft.
Check all fields in the browser. Long bodies can exceed browser or Gmail URL limits;
`gmd` warns above 8,000 URL bytes (a precaution, not a guaranteed Gmail limit).
The URL contains the message fields, including Bcc and body, so treat it as private;
it may appear in browser history. For large messages, paste the body into Gmail.

## Development

```sh
cargo test
cargo clippy --all-targets -- -D warnings
# With Pandoc and Typst installed, and cmarker cached:
cargo test -- --ignored
cargo run -- print examples/print.md -o target/print.pdf
```

Tests use local fixtures and do not open Gmail or send mail.
