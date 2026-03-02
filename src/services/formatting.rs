use tower_lsp::lsp_types::*;

fn position_at(text: &str, offset: usize) -> Position {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in text.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    Position::new(line, col)
}

/// Basic HTML formatting: normalizes indentation based on tag nesting.
pub fn format_html(text: &str, options: &FormattingOptions) -> Vec<TextEdit> {
    let tab_size = options.tab_size as usize;
    let use_spaces = options.insert_spaces;
    let indent_str = if use_spaces {
        " ".repeat(tab_size)
    } else {
        "\t".to_string()
    };

    let mut formatted = String::new();
    let mut indent_level: i32 = 0;
    let mut in_tag = false;
    let mut in_closing_tag = false;
    let mut in_comment = false;
    let mut in_pre = false;
    let mut tag_name = String::new();
    let mut collecting_tag_name = false;

    let self_closing_tags = [
        "area", "base", "br", "col", "embed", "hr", "img", "input",
        "link", "meta", "param", "source", "track", "wbr",
    ];

    for line in text.lines() {
        let trimmed = line.trim();

        if in_pre {
            formatted.push_str(line);
            formatted.push('\n');
            if trimmed.contains("</pre>") || trimmed.contains("</code>") || trimmed.contains("</textarea>") {
                in_pre = false;
            }
            continue;
        }

        if trimmed.is_empty() {
            formatted.push('\n');
            continue;
        }

        // Check if this line starts with a closing tag
        let starts_with_close = trimmed.starts_with("</");

        if starts_with_close {
            indent_level = (indent_level - 1).max(0);
        }

        // Apply indentation
        for _ in 0..indent_level {
            formatted.push_str(&indent_str);
        }
        formatted.push_str(trimmed);
        formatted.push('\n');

        // Check for pre/code/textarea
        if trimmed.contains("<pre") || trimmed.contains("<code") || trimmed.contains("<textarea") {
            in_pre = true;
        }

        // Count tag openings and closings on this line to determine indent for next line
        let mut i = 0;
        let bytes = trimmed.as_bytes();
        while i < bytes.len() {
            if bytes[i] == b'<' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    // Closing tag - already handled above
                    if !starts_with_close || i > 0 {
                        indent_level = (indent_level - 1).max(0);
                    }
                    // Skip past tag name
                    i += 2;
                    while i < bytes.len() && bytes[i] != b'>' {
                        i += 1;
                    }
                } else if i + 1 < bytes.len() && bytes[i + 1] == b'!' {
                    // Comment or doctype, skip
                    while i < bytes.len() && bytes[i] != b'>' {
                        i += 1;
                    }
                } else {
                    // Opening tag
                    let tag_start = i + 1;
                    let mut tag_end = tag_start;
                    while tag_end < bytes.len()
                        && bytes[tag_end] != b' '
                        && bytes[tag_end] != b'>'
                        && bytes[tag_end] != b'/'
                    {
                        tag_end += 1;
                    }
                    let tag = std::str::from_utf8(&bytes[tag_start..tag_end])
                        .unwrap_or("")
                        .to_lowercase();

                    // Check if self-closing (either void tag or />)
                    let mut is_self_close = self_closing_tags.contains(&tag.as_str());
                    if !is_self_close {
                        // Check for />
                        let mut j = tag_end;
                        while j < bytes.len() && bytes[j] != b'>' {
                            j += 1;
                        }
                        if j > 0 && bytes[j.saturating_sub(1)] == b'/' {
                            is_self_close = true;
                        }
                    }

                    if !is_self_close && !tag.is_empty() {
                        indent_level += 1;
                    }

                    i = tag_end;
                    while i < bytes.len() && bytes[i] != b'>' {
                        i += 1;
                    }
                }
            }
            i += 1;
        }
    }

    // Remove trailing newline to match original
    if formatted.ends_with('\n') && !text.ends_with('\n') {
        formatted.pop();
    }

    vec![TextEdit::new(
        Range::new(Position::new(0, 0), position_at(text, text.len())),
        formatted,
    )]
}
