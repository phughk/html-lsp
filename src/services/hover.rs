use tower_lsp::lsp_types::*;

use crate::language_facts::data_manager::DataManager;
use crate::language_facts::data_provider::MarkupDescription;
use crate::parser::html_document::HtmlDocument;
use crate::parser::scanner::{create_scanner, ScannerState, TokenType};

fn offset_at(text: &str, position: Position) -> usize {
    let mut offset = 0;
    for (i, line) in text.split('\n').enumerate() {
        if i == position.line as usize {
            return offset + (position.character as usize).min(line.len());
        }
        offset += line.len() + 1;
    }
    offset.min(text.len())
}

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

fn generate_doc(desc: Option<&MarkupDescription>, refs: &[crate::language_facts::data_provider::Reference]) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(d) = desc {
        parts.push(d.value().to_string());
    }
    if !refs.is_empty() {
        let ref_strs: Vec<String> = refs.iter().map(|r| format!("[{}]({})", r.name, r.url)).collect();
        parts.push(ref_strs.join(" | "));
    }
    if parts.is_empty() { None } else { Some(parts.join("\n\n")) }
}

fn get_tag_name_range(text: &str, token_type: TokenType, start_offset: usize, target_offset: usize) -> Option<Range> {
    let mut scanner = create_scanner(text, start_offset, ScannerState::WithinContent, false);
    let mut token = scanner.scan();
    while token != TokenType::EOS
        && (scanner.get_token_end() < target_offset
            || (scanner.get_token_end() == target_offset && token != token_type))
    {
        token = scanner.scan();
    }
    if token == token_type && target_offset <= scanner.get_token_end() {
        Some(Range::new(
            position_at(text, scanner.get_token_offset()),
            position_at(text, scanner.get_token_end()),
        ))
    } else {
        None
    }
}

fn trim_quotes(s: &str) -> &str {
    if s.len() <= 1 {
        return s.trim_matches(|c| c == '"' || c == '\'');
    }
    let bytes = s.as_bytes();
    let start = if bytes[0] == b'"' || bytes[0] == b'\'' { 1 } else { 0 };
    let end = if bytes[s.len() - 1] == b'"' || bytes[s.len() - 1] == b'\'' {
        s.len() - 1
    } else {
        s.len()
    };
    &s[start..end]
}

fn is_letter_or_digit(ch: u8) -> bool {
    ch.is_ascii_alphanumeric()
}

pub fn do_hover(
    text: &str,
    position: Position,
    document: &HtmlDocument,
    data_manager: &DataManager,
) -> Option<Hover> {
    let offset = offset_at(text, position);
    let node_id = document.find_node_at(offset);
    let node = document.node(node_id);
    let provider = data_manager.provider();

    let tag = node.tag.as_deref()?;

    // Check if in end tag
    if let Some(end_tag_start) = node.end_tag_start {
        if offset >= end_tag_start {
            let range = get_tag_name_range(text, TokenType::EndTag, end_tag_start, offset)?;
            return get_tag_hover(tag, range, provider);
        }
    }

    // Check if on start tag
    if let Some(range) = get_tag_name_range(text, TokenType::StartTag, node.start, offset) {
        return get_tag_hover(tag, range, provider);
    }

    // Check if on attribute name
    if let Some(range) = get_tag_name_range(text, TokenType::AttributeName, node.start, offset) {
        let attr_text = &text[offset_at(text, range.start)..offset_at(text, range.end)];
        return get_attr_hover(tag, attr_text, range, provider);
    }

    // Check entity hover
    {
        let bytes = text.as_bytes();
        let mut k = offset.wrapping_sub(1);
        while k < bytes.len() && is_letter_or_digit(bytes[k]) {
            k = k.wrapping_sub(1);
        }
        if k < bytes.len() && bytes[k] == b'&' {
            let mut end = offset;
            while end < bytes.len() && is_letter_or_digit(bytes[end]) {
                end += 1;
            }
            let entity_name = &text[k..if end < bytes.len() && bytes[end] == b';' { end + 1 } else { end }];
            if let Some(hover) = get_entity_hover(entity_name, text, position, offset) {
                return Some(hover);
            }
        }
    }

    // Check attribute value hover
    if let Some(range) = get_tag_name_range(text, TokenType::AttributeValue, node.start, offset) {
        let attr_value = trim_quotes(&text[offset_at(text, range.start)..offset_at(text, range.end)]);
        // Find the attribute name for this value
        let attr_name = scan_attr_name(text, node.start, offset_at(text, range.start));
        if let Some(attr) = attr_name {
            return get_attr_value_hover(tag, &attr, attr_value, range, provider);
        }
    }

    None
}

fn scan_attr_name(text: &str, node_start: usize, attr_value_start: usize) -> Option<String> {
    let mut scanner = create_scanner(text, node_start, ScannerState::WithinContent, false);
    let mut token = scanner.scan();
    let mut prev_attr = None;
    while token != TokenType::EOS && scanner.get_token_end() <= attr_value_start {
        token = scanner.scan();
        if token == TokenType::AttributeName {
            prev_attr = Some(scanner.get_token_text());
        }
    }
    prev_attr
}

fn get_tag_hover(
    tag: &str,
    range: Range,
    provider: &crate::language_facts::data_provider::HtmlDataProvider,
) -> Option<Hover> {
    for t in provider.provide_tags() {
        if t.name.eq_ignore_ascii_case(tag) {
            let doc = generate_doc(t.description.as_ref(), &t.references);
            return Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.unwrap_or_default(),
                }),
                range: Some(range),
            });
        }
    }
    None
}

fn get_attr_hover(
    tag: &str,
    attr: &str,
    range: Range,
    provider: &crate::language_facts::data_provider::HtmlDataProvider,
) -> Option<Hover> {
    for a in provider.provide_attributes(tag) {
        if a.name == attr {
            if let Some(doc) = generate_doc(a.description.as_ref(), &a.references) {
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: doc,
                    }),
                    range: Some(range),
                });
            }
        }
    }
    None
}

fn get_attr_value_hover(
    tag: &str,
    attr: &str,
    value: &str,
    range: Range,
    provider: &crate::language_facts::data_provider::HtmlDataProvider,
) -> Option<Hover> {
    for v in provider.provide_values(tag, attr) {
        if v.name == value {
            if let Some(doc) = generate_doc(v.description.as_ref(), &v.references) {
                return Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: doc,
                    }),
                    range: Some(range),
                });
            }
        }
    }
    None
}

fn get_entity_hover(entity: &str, text: &str, position: Position, offset: usize) -> Option<Hover> {
    // Map common entities
    let entities: &[(&str, &str, u32)] = &[
        ("&amp;", "&", 0x0026),
        ("&lt;", "<", 0x003C),
        ("&gt;", ">", 0x003E),
        ("&quot;", "\"", 0x0022),
        ("&apos;", "'", 0x0027),
        ("&nbsp;", "\u{00A0}", 0x00A0),
        ("&copy;", "\u{00A9}", 0x00A9),
        ("&reg;", "\u{00AE}", 0x00AE),
    ];

    for (name, repr, code) in entities {
        if entity == *name || (entity.len() > 1 && !entity.ends_with(';') && name.starts_with(entity)) {
            let hex = format!("U+{:04X}", code);
            let doc = format!("Character entity representing '{}', unicode equivalent '{}'", repr, hex);
            let bytes = text.as_bytes();
            let mut start = offset.wrapping_sub(1);
            while start < bytes.len() && is_letter_or_digit(bytes[start]) {
                start = start.wrapping_sub(1);
            }
            // start should now point to '&'
            let range = Range::new(
                position_at(text, start),
                position_at(text, offset + if offset < bytes.len() && bytes.get(offset) == Some(&b';') { 1 } else { 0 }),
            );
            return Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(doc)),
                range: Some(range),
            });
        }
    }
    None
}
