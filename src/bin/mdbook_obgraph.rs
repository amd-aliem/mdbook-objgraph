use std::io::{self, Read, Write};
use std::path::Path;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // `mdbook-obgraph supports <renderer>`
    if args.len() >= 3 && args[1] == "supports" {
        let renderer = &args[2];
        if renderer == "html" {
            process::exit(0);
        } else {
            process::exit(1);
        }
    }

    // `mdbook-obgraph install <path>`
    if args.len() >= 2 && args[1] == "install" {
        let root = if args.len() >= 3 {
            args[2].clone()
        } else {
            ".".to_string()
        };
        if let Err(e) = run_install(&root) {
            eprintln!("mdbook-obgraph install error: {e}");
            process::exit(1);
        }
        return;
    }

    // Default: preprocessor mode — read [context, book] from stdin, process, write to stdout
    if let Err(e) = run_preprocessor() {
        eprintln!("mdbook-obgraph error: {e}");
        process::exit(1);
    }
}

/// Read `[context, book]` JSON from stdin, walk chapters replacing ```obgraph blocks,
/// then write the modified book JSON to stdout.
fn run_preprocessor() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    let mut value: serde_json::Value = serde_json::from_str(&input)?;

    // The payload is a two-element array: [context, book]
    // We only need to modify `book` (index 1).
    let book = value
        .get_mut(1)
        .ok_or("expected a [context, book] JSON array")?;

    walk_book(book)?;

    let output = serde_json::to_string(&book)?;
    io::stdout().write_all(output.as_bytes())?;
    io::stdout().flush()?;

    Ok(())
}

/// Recursively walk the mdbook JSON structure and process obgraph code blocks
/// in every chapter's `content` field.
fn walk_book(value: &mut serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    match value {
        serde_json::Value::Object(map) => {
            // If this object has a "Chapter" key, process its content.
            if let Some(chapter) = map.get_mut("Chapter") {
                if let Some(content) = chapter.get_mut("content") && let Some(s) = content.as_str() {
                    let processed = process_markdown(s)?;
                    *content = serde_json::Value::String(processed);
                }
                // Also recurse into sub-items of the chapter.
                if let Some(sub_items) = chapter.get_mut("sub_items") {
                    walk_book(sub_items)?;
                }
            } else {
                // Generic object — recurse into all values.
                for v in map.values_mut() {
                    walk_book(v)?;
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                walk_book(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Replace every ```obgraph ... ``` fenced code block in `markdown` with the
/// rendered SVG/HTML fragment produced by `mdbook_obgraph::process`.
fn process_markdown(markdown: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut result = String::with_capacity(markdown.len());
    let mut remaining = markdown;

    while let Some(start) = find_obgraph_fence(remaining) {
        // Append everything before the fence.
        result.push_str(&remaining[..start]);

        let after_fence = &remaining[start + "```obgraph".len()..];

        // Find the closing ```.
        if let Some(end_offset) = find_closing_fence(after_fence) {
            let block_content = &after_fence[..end_offset];

            match mdbook_obgraph::process(block_content) {
                Ok(svg) => result.push_str(&svg),
                Err(e) => {
                    // Emit an HTML comment describing the error, then the
                    // original fenced block so the author can see what failed.
                    result.push_str(&format!(
                        "<!-- mdbook-obgraph error: {e} -->\n```obgraph{block_content}```"
                    ));
                }
            }

            // Advance past the closing fence (``` is 3 chars).
            remaining = &after_fence[end_offset + "```".len()..];
        } else {
            // No closing fence found — emit the rest as-is.
            result.push_str("```obgraph");
            result.push_str(after_fence);
            remaining = "";
        }
    }

    result.push_str(remaining);
    Ok(result)
}

/// Find the byte offset of the next ` ```obgraph ` opener in `s`.
/// The fence must appear at the start of a line (possibly with leading spaces).
fn find_obgraph_fence(s: &str) -> Option<usize> {
    // We look for "```obgraph" appearing at the start of a line.
    let needle = "```obgraph";
    let mut search = s;
    let mut base = 0usize;

    loop {
        let idx = search.find(needle)?;
        let abs = base + idx;

        // Check that it appears at the beginning of a line.
        let at_line_start = abs == 0 || s.as_bytes()[abs - 1] == b'\n';

        // After the info string there must be a newline (or end of string).
        let after = &search[idx + needle.len()..];
        let info_end = after.find('\n').unwrap_or(after.len());
        let info = after[..info_end].trim();
        // We accept "```obgraph" with optional trailing whitespace on the same line.
        let valid_info = info.is_empty();

        if at_line_start && valid_info {
            return Some(abs);
        }

        // Advance past this occurrence and keep searching.
        base += idx + needle.len();
        search = &search[idx + needle.len()..];
    }
}

/// Given the content *after* the opening fence line's newline, find the offset
/// of the closing ` ``` ` fence.  Returns the offset of the ` ``` ` in `s`.
fn find_closing_fence(s: &str) -> Option<usize> {
    // The content passed in starts right after "```obgraph" (before the newline).
    // We need to skip to the next line first.
    let after_newline_offset = s.find('\n').map(|i| i + 1)?;
    let search_area = &s[after_newline_offset..];

    // Now look for a line that is exactly "```" (possibly with trailing whitespace).
    let mut offset = after_newline_offset;
    for line in search_area.lines() {
        if line.trim() == "```" {
            return Some(offset);
        }
        offset += line.len() + 1; // +1 for the '\n'
    }
    None
}

/// Add `[preprocessor.obgraph]` to the `book.toml` at `<root>/book.toml`.
fn run_install(root: &str) -> Result<(), Box<dyn std::error::Error>> {
    let toml_path = Path::new(root).join("book.toml");

    let existing = if toml_path.exists() {
        std::fs::read_to_string(&toml_path)?
    } else {
        String::new()
    };

    if existing.contains("[preprocessor.obgraph]") {
        eprintln!("mdbook-obgraph: [preprocessor.obgraph] already present in book.toml");
        return Ok(());
    }

    let mut updated = existing;
    if !updated.ends_with('\n') && !updated.is_empty() {
        updated.push('\n');
    }
    updated.push_str("\n[preprocessor.obgraph]\n");

    std::fs::write(&toml_path, updated)?;
    eprintln!(
        "mdbook-obgraph: added [preprocessor.obgraph] to {}",
        toml_path.display()
    );
    Ok(())
}
