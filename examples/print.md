# Markdown printing

Prepare a readable handout with **formatted text**, *emphasis*, and `inline code`.

## Highlighted code

Language tags on fenced blocks select the syntax highlighting.

```rust
fn main() {
    let recipient = "Alice";
    println!("Hello, {recipient}!");
}
```

```python
def total(prices):
    # Sum the invoice items.
    return sum(price for price in prices if price > 0)
```

```json
{
  "subject": "Monthly report",
  "attachments": ["report.pdf"],
  "review_required": true
}
```

## Before sending

1. Review the message.
2. Add the attachments.
3. Send from Gmail.

| File | Purpose |
| --- | --- |
| `notes.md` | Editable source |
| `notes.pdf` | Formatted attachment |

> Printing leaves the original Markdown file unchanged.

See the [Typst documentation](https://typst.app/docs/) for its supported languages.
