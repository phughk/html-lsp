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
        offset += line.len() + 1; // +1 for \n
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

fn generate_documentation(desc: Option<&MarkupDescription>, refs: &[crate::language_facts::data_provider::Reference]) -> Option<Documentation> {
    let mut parts = Vec::new();

    if let Some(d) = desc {
        parts.push(d.value().to_string());
    }

    if !refs.is_empty() {
        let ref_strs: Vec<String> = refs.iter().map(|r| format!("[{}]({})", r.name, r.url)).collect();
        parts.push(ref_strs.join(" | "));
    }

    if parts.is_empty() {
        None
    } else {
        Some(Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: parts.join("\n\n"),
        }))
    }
}

fn is_quote(ch: u8) -> bool {
    ch == b'"' || ch == b'\''
}

fn is_whitespace(ch: u8) -> bool {
    matches!(ch, b' ' | b'\t' | b'\n' | b'\r')
}

fn is_letter_or_digit(ch: u8) -> bool {
    ch.is_ascii_alphanumeric()
}

fn is_followed_by(text: &str, offset: usize, initial_state: ScannerState, expected_token: TokenType) -> bool {
    let mut scanner = create_scanner(text, offset, initial_state, false);
    let mut token = scanner.scan();
    while token == TokenType::Whitespace {
        token = scanner.scan();
    }
    token == expected_token
}

fn get_word_start(s: &[u8], offset: usize, limit: usize) -> usize {
    let mut o = offset;
    while o > limit && !is_whitespace(s[o - 1]) {
        o -= 1;
    }
    o
}

fn get_word_end(s: &[u8], offset: usize, limit: usize) -> usize {
    let mut o = offset;
    while o < limit && !is_whitespace(s[o]) {
        o += 1;
    }
    o
}

pub fn do_complete(
    text: &str,
    position: Position,
    document: &HtmlDocument,
    data_manager: &DataManager,
    uri: &Url,
) -> CompletionResponse {
    let mut items: Vec<CompletionItem> = Vec::new();
    let offset = offset_at(text, position);
    let provider = data_manager.provider();

    let node_id = document.find_node_before(offset);
    let node = document.node(node_id);

    let mut scanner = create_scanner(text, node.start, ScannerState::WithinContent, false);
    let mut current_tag = String::new();
    let mut current_attribute_name = String::new();

    let get_replace_range = |replace_start: usize, replace_end: usize| -> Range {
        let start = replace_start.min(offset);
        Range::new(position_at(text, start), position_at(text, replace_end))
    };

    // Collect open tag suggestions
    let collect_open_tag_suggestions = |items: &mut Vec<CompletionItem>, after_open: usize, tag_name_end: usize| {
        let range = get_replace_range(after_open, tag_name_end);
        for tag in provider.provide_tags() {
            items.push(CompletionItem {
                label: tag.name.clone(),
                kind: Some(CompletionItemKind::PROPERTY),
                documentation: generate_documentation(tag.description.as_ref(), &tag.references),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit::new(range, tag.name.clone()))),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                ..Default::default()
            });
        }
    };

    // Scan tokens
    let mut token = scanner.scan();
    while token != TokenType::EOS && scanner.get_token_offset() <= offset {
        match token {
            TokenType::StartTagOpen => {
                if scanner.get_token_end() == offset {
                    let end_pos = {
                        let mut ep = offset;
                        let mut s2 = create_scanner(text, offset, ScannerState::AfterOpeningStartTag, false);
                        let t = s2.scan();
                        if t == TokenType::StartTag && s2.get_token_offset() == offset {
                            ep = s2.get_token_end();
                        }
                        ep
                    };
                    // Suggest DOCTYPE on first line
                    if position.line == 0 {
                        let range = get_replace_range(offset, end_pos);
                        items.push(CompletionItem {
                            label: "!DOCTYPE".to_string(),
                            kind: Some(CompletionItemKind::PROPERTY),
                            documentation: Some(Documentation::String("A preamble for an HTML document.".to_string())),
                            text_edit: Some(CompletionTextEdit::Edit(TextEdit::new(range, "!DOCTYPE html>".to_string()))),
                            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                            ..Default::default()
                        });
                    }
                    // Collect tag + close tag suggestions
                    collect_open_tag_suggestions(&mut items, offset, end_pos);
                    collect_close_tag_suggestions(&mut items, text, offset, false, end_pos, document, node_id, &get_replace_range, provider, offset);
                    return CompletionResponse::List(CompletionList {
                        is_incomplete: false,
                        items,
                    });
                }
            }
            TokenType::StartTag => {
                if scanner.get_token_offset() <= offset && offset <= scanner.get_token_end() {
                    collect_open_tag_suggestions(&mut items, scanner.get_token_offset(), scanner.get_token_end());
                    return CompletionResponse::List(CompletionList {
                        is_incomplete: false,
                        items,
                    });
                }
                current_tag = scanner.get_token_text();
            }
            TokenType::AttributeName => {
                if scanner.get_token_offset() <= offset && offset <= scanner.get_token_end() {
                    collect_attribute_name_suggestions(
                        &mut items, text, &current_tag, scanner.get_token_offset(),
                        scanner.get_token_end(), offset, provider, document, node_id, &get_replace_range,
                    );
                    return CompletionResponse::List(CompletionList {
                        is_incomplete: false,
                        items,
                    });
                }
                current_attribute_name = scanner.get_token_text();
            }
            TokenType::DelimiterAssign => {
                if scanner.get_token_end() == offset {
                    let end_pos = {
                        let mut s2 = create_scanner(text, offset, ScannerState::BeforeAttributeValue, false);
                        let t = s2.scan();
                        if t == TokenType::AttributeValue && s2.get_token_offset() == offset {
                            s2.get_token_end()
                        } else {
                            offset
                        }
                    };
                    collect_attribute_value_suggestions(
                        &mut items, text, &current_tag, &current_attribute_name,
                        offset, end_pos, offset, provider, &get_replace_range,
                    );
                    return CompletionResponse::List(CompletionList {
                        is_incomplete: false,
                        items,
                    });
                }
            }
            TokenType::AttributeValue => {
                if scanner.get_token_offset() <= offset && offset <= scanner.get_token_end() {
                    collect_attribute_value_suggestions(
                        &mut items, text, &current_tag, &current_attribute_name,
                        scanner.get_token_offset(), scanner.get_token_end(), offset, provider, &get_replace_range,
                    );
                    return CompletionResponse::List(CompletionList {
                        is_incomplete: false,
                        items,
                    });
                }
            }
            TokenType::Whitespace => {
                if offset <= scanner.get_token_end() {
                    match scanner.get_scanner_state() {
                        ScannerState::AfterOpeningStartTag => {
                            let start_pos = scanner.get_token_offset();
                            let mut s2 = create_scanner(text, offset, ScannerState::AfterOpeningStartTag, false);
                            let t = s2.scan();
                            let end_tag_pos = if t == TokenType::StartTag && s2.get_token_offset() == offset {
                                s2.get_token_end()
                            } else {
                                offset
                            };
                            collect_open_tag_suggestions(&mut items, start_pos, end_tag_pos);
                            collect_close_tag_suggestions(&mut items, text, start_pos, true, end_tag_pos, document, node_id, &get_replace_range, provider, offset);
                            return CompletionResponse::List(CompletionList {
                                is_incomplete: false,
                                items,
                            });
                        }
                        ScannerState::WithinTag | ScannerState::AfterAttributeName => {
                            collect_attribute_name_suggestions(
                                &mut items, text, &current_tag, scanner.get_token_end(),
                                offset, offset, provider, document, node_id, &get_replace_range,
                            );
                            return CompletionResponse::List(CompletionList {
                                is_incomplete: false,
                                items,
                            });
                        }
                        ScannerState::BeforeAttributeValue => {
                            collect_attribute_value_suggestions(
                                &mut items, text, &current_tag, &current_attribute_name,
                                scanner.get_token_end(), offset, offset, provider, &get_replace_range,
                            );
                            return CompletionResponse::List(CompletionList {
                                is_incomplete: false,
                                items,
                            });
                        }
                        ScannerState::AfterOpeningEndTag => {
                            collect_close_tag_suggestions(
                                &mut items, text, scanner.get_token_offset().saturating_sub(1), false, offset,
                                document, node_id, &get_replace_range, provider, offset,
                            );
                            return CompletionResponse::List(CompletionList {
                                is_incomplete: false,
                                items,
                            });
                        }
                        ScannerState::WithinContent => {
                            collect_character_entity_proposals(&mut items, text, offset, position);
                            return CompletionResponse::List(CompletionList {
                                is_incomplete: false,
                                items,
                            });
                        }
                        _ => {}
                    }
                }
            }
            TokenType::EndTagOpen => {
                if offset <= scanner.get_token_end() {
                    let after_open_bracket = scanner.get_token_offset() + 1;
                    let mut s2 = create_scanner(text, offset, ScannerState::AfterOpeningEndTag, false);
                    let t = s2.scan();
                    let end_offset = if t == TokenType::EndTag && s2.get_token_offset() == offset {
                        s2.get_token_end()
                    } else {
                        offset
                    };
                    collect_close_tag_suggestions(
                        &mut items, text, after_open_bracket, false, end_offset,
                        document, node_id, &get_replace_range, provider, offset,
                    );
                    return CompletionResponse::List(CompletionList {
                        is_incomplete: false,
                        items,
                    });
                }
            }
            TokenType::EndTag => {
                if offset <= scanner.get_token_end() {
                    let mut start = scanner.get_token_offset().saturating_sub(1);
                    let bytes = text.as_bytes();
                    while start > 0 {
                        let ch = bytes[start];
                        if ch == b'/' {
                            collect_close_tag_suggestions(
                                &mut items, text, start, false, scanner.get_token_end(),
                                document, node_id, &get_replace_range, provider, offset,
                            );
                            return CompletionResponse::List(CompletionList {
                                is_incomplete: false,
                                items,
                            });
                        } else if !is_whitespace(ch) {
                            break;
                        }
                        start -= 1;
                    }
                }
            }
            TokenType::StartTagClose => {
                if offset <= scanner.get_token_end() {
                    if !current_tag.is_empty() && !data_manager.is_void_element(&current_tag) {
                        let pos = position_at(text, scanner.get_token_end());
                        items.push(CompletionItem {
                            label: format!("</{}>", current_tag),
                            kind: Some(CompletionItemKind::PROPERTY),
                            filter_text: Some(format!("</{}>", current_tag)),
                            text_edit: Some(CompletionTextEdit::Edit(TextEdit::new(
                                Range::new(pos, pos),
                                format!("$0</{}>", current_tag),
                            ))),
                            insert_text_format: Some(InsertTextFormat::SNIPPET),
                            ..Default::default()
                        });
                    }
                    return CompletionResponse::List(CompletionList {
                        is_incomplete: false,
                        items,
                    });
                }
            }
            TokenType::Content => {
                if offset <= scanner.get_token_end() {
                    collect_character_entity_proposals(&mut items, text, offset, position);
                    return CompletionResponse::List(CompletionList {
                        is_incomplete: false,
                        items,
                    });
                }
            }
            _ => {
                if offset <= scanner.get_token_end() {
                    return CompletionResponse::List(CompletionList {
                        is_incomplete: false,
                        items,
                    });
                }
            }
        }
        token = scanner.scan();
    }

    CompletionResponse::List(CompletionList {
        is_incomplete: false,
        items,
    })
}

fn collect_close_tag_suggestions(
    items: &mut Vec<CompletionItem>,
    text: &str,
    after_open_bracket: usize,
    in_open_tag: bool,
    tag_name_end: usize,
    document: &HtmlDocument,
    node_id: usize,
    get_replace_range: &dyn Fn(usize, usize) -> Range,
    provider: &crate::language_facts::data_provider::HtmlDataProvider,
    offset: usize,
) {
    let range = get_replace_range(after_open_bracket, tag_name_end);
    let close_tag = if is_followed_by(text, tag_name_end, ScannerState::WithinEndTag, TokenType::EndTagClose) {
        ""
    } else {
        ">"
    };

    let mut curr_id = if in_open_tag {
        document.node(node_id).parent
    } else {
        Some(node_id)
    };

    while let Some(id) = curr_id {
        let n = document.node(id);
        if let Some(ref tag) = n.tag {
            if !n.closed || (n.end_tag_start.is_some() && n.end_tag_start.unwrap() > offset) {
                items.push(CompletionItem {
                    label: format!("/{}", tag),
                    kind: Some(CompletionItemKind::PROPERTY),
                    filter_text: Some(format!("/{}", tag)),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit::new(
                        range,
                        format!("/{}{}", tag, close_tag),
                    ))),
                    insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                    ..Default::default()
                });
                return;
            }
        }
        curr_id = n.parent;
    }

    if in_open_tag {
        return;
    }

    // Suggest all tags as close tags
    for tag in provider.provide_tags() {
        items.push(CompletionItem {
            label: format!("/{}", tag.name),
            kind: Some(CompletionItemKind::PROPERTY),
            documentation: generate_documentation(tag.description.as_ref(), &tag.references),
            filter_text: Some(format!("/{}{}", tag.name, close_tag)),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit::new(
                range,
                format!("/{}{}", tag.name, close_tag),
            ))),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..Default::default()
        });
    }
}

fn collect_attribute_name_suggestions(
    items: &mut Vec<CompletionItem>,
    text: &str,
    current_tag: &str,
    name_start: usize,
    name_end: usize,
    offset: usize,
    provider: &crate::language_facts::data_provider::HtmlDataProvider,
    document: &HtmlDocument,
    node_id: usize,
    get_replace_range: &dyn Fn(usize, usize) -> Range,
) {
    let bytes = text.as_bytes();
    let mut replace_end = offset;
    while replace_end < name_end && replace_end < bytes.len() && bytes[replace_end] != b'<' {
        replace_end += 1;
    }
    let range = get_replace_range(name_start, replace_end);

    let value = if !is_followed_by(text, name_end, ScannerState::AfterAttributeName, TokenType::DelimiterAssign) {
        "=\"$1\""
    } else {
        ""
    };

    let node = document.node(node_id);
    let mut seen: std::collections::HashSet<String> = node.attributes.keys().cloned().collect();
    let current_attr = &text[name_start..name_end.min(text.len())];
    seen.remove(current_attr);

    for attr in provider.provide_attributes(current_tag) {
        if seen.contains(&attr.name) {
            continue;
        }
        seen.insert(attr.name.clone());

        let snippet = if attr.value_set.as_deref() == Some("v") || value.is_empty() {
            attr.name.clone()
        } else {
            format!("{}{}", attr.name, value)
        };

        let kind = if attr.value_set.as_deref() == Some("handler") {
            CompletionItemKind::FUNCTION
        } else {
            CompletionItemKind::VALUE
        };

        items.push(CompletionItem {
            label: attr.name.clone(),
            kind: Some(kind),
            documentation: generate_documentation(attr.description.as_ref(), &attr.references),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit::new(range, snippet))),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
    }
}

fn collect_attribute_value_suggestions(
    items: &mut Vec<CompletionItem>,
    text: &str,
    current_tag: &str,
    current_attribute: &str,
    value_start: usize,
    value_end: usize,
    offset: usize,
    provider: &crate::language_facts::data_provider::HtmlDataProvider,
    get_replace_range: &dyn Fn(usize, usize) -> Range,
) {
    let bytes = text.as_bytes();
    let (range, add_quotes) = if offset > value_start
        && offset <= value_end
        && value_start < bytes.len()
        && is_quote(bytes[value_start])
    {
        let value_content_start = value_start + 1;
        let mut value_content_end = value_end;
        if value_end > value_start && value_end <= bytes.len() && bytes[value_end - 1] == bytes[value_start] {
            value_content_end -= 1;
        }
        let ws_before = get_word_start(bytes, offset, value_content_start);
        let ws_after = get_word_end(bytes, offset, value_content_end);
        (get_replace_range(ws_before, ws_after), false)
    } else {
        (get_replace_range(value_start, value_end), true)
    };

    for value in provider.provide_values(current_tag, current_attribute) {
        let insert_text = if add_quotes {
            format!("\"{}\"", value.name)
        } else {
            value.name.clone()
        };

        items.push(CompletionItem {
            label: value.name.clone(),
            filter_text: Some(insert_text.clone()),
            kind: Some(CompletionItemKind::UNIT),
            documentation: generate_documentation(value.description.as_ref(), &value.references),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit::new(range, insert_text))),
            insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
            ..Default::default()
        });
    }

    collect_character_entity_proposals(items, text, offset, position_at(text, offset));
}

fn collect_character_entity_proposals(
    items: &mut Vec<CompletionItem>,
    text: &str,
    offset: usize,
    position: Position,
) {
    let bytes = text.as_bytes();
    let mut k = offset.wrapping_sub(1);
    let mut char_start = position.character;

    while k < bytes.len() && is_letter_or_digit(bytes[k]) {
        k = k.wrapping_sub(1);
        char_start = char_start.saturating_sub(1);
    }

    if k < bytes.len() && bytes[k] == b'&' {
        let range = Range::new(
            Position::new(position.line, char_start.saturating_sub(1)),
            position,
        );

        // Common HTML entities
        let entities = [
            ("&amp;", "&", "Ampersand"),
            ("&lt;", "<", "Less-than sign"),
            ("&gt;", ">", "Greater-than sign"),
            ("&quot;", "\"", "Quotation mark"),
            ("&apos;", "'", "Apostrophe"),
            ("&nbsp;", "\u{00A0}", "Non-breaking space"),
            ("&copy;", "\u{00A9}", "Copyright sign"),
            ("&reg;", "\u{00AE}", "Registered sign"),
            ("&trade;", "\u{2122}", "Trade mark sign"),
            ("&euro;", "\u{20AC}", "Euro sign"),
            ("&pound;", "\u{00A3}", "Pound sign"),
            ("&yen;", "\u{00A5}", "Yen sign"),
            ("&cent;", "\u{00A2}", "Cent sign"),
            ("&mdash;", "\u{2014}", "Em dash"),
            ("&ndash;", "\u{2013}", "En dash"),
            ("&laquo;", "\u{00AB}", "Left double angle quotes"),
            ("&raquo;", "\u{00BB}", "Right double angle quotes"),
            ("&larr;", "\u{2190}", "Leftwards arrow"),
            ("&rarr;", "\u{2192}", "Rightwards arrow"),
            ("&uarr;", "\u{2191}", "Upwards arrow"),
            ("&darr;", "\u{2193}", "Downwards arrow"),
            ("&hellip;", "\u{2026}", "Horizontal ellipsis"),
            ("&bull;", "\u{2022}", "Bullet"),
            ("&times;", "\u{00D7}", "Multiplication sign"),
            ("&divide;", "\u{00F7}", "Division sign"),
            ("&plusmn;", "\u{00B1}", "Plus-minus sign"),
            ("&deg;", "\u{00B0}", "Degree sign"),
            ("&micro;", "\u{00B5}", "Micro sign"),
            ("&para;", "\u{00B6}", "Pilcrow sign"),
            ("&sect;", "\u{00A7}", "Section sign"),
            ("&infin;", "\u{221E}", "Infinity"),
            ("&ne;", "\u{2260}", "Not equal to"),
            ("&le;", "\u{2264}", "Less-than or equal to"),
            ("&ge;", "\u{2265}", "Greater-than or equal to"),
            ("&frac14;", "\u{00BC}", "Fraction one quarter"),
            ("&frac12;", "\u{00BD}", "Fraction one half"),
            ("&frac34;", "\u{00BE}", "Fraction three quarters"),
        ];

        for (label, char_repr, desc) in &entities {
            items.push(CompletionItem {
                label: label.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                documentation: Some(Documentation::String(format!(
                    "Character entity representing '{}' ({})",
                    char_repr, desc
                ))),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit::new(range, label.to_string()))),
                insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
                ..Default::default()
            });
        }
    }
}

pub fn do_tag_complete(
    text: &str,
    position: Position,
    document: &HtmlDocument,
    data_manager: &DataManager,
) -> Option<String> {
    let offset = offset_at(text, position);
    if offset == 0 {
        return None;
    }

    let bytes = text.as_bytes();
    let ch = bytes[offset - 1];

    if ch == b'>' {
        let node_id = document.find_node_before(offset);
        let node = document.node(node_id);
        if let Some(ref tag) = node.tag {
            if node.start < offset && (node.end_tag_start.is_none() || node.end_tag_start.unwrap() > offset) {
                if !data_manager.is_void_element(tag) {
                    let mut scanner = create_scanner(text, node.start, ScannerState::WithinContent, false);
                    let mut token = scanner.scan();
                    while token != TokenType::EOS && scanner.get_token_end() <= offset {
                        if token == TokenType::StartTagClose && scanner.get_token_end() == offset {
                            return Some(format!("$0</{}>", tag));
                        }
                        token = scanner.scan();
                    }
                }
            }
        }
    } else if ch == b'/' {
        let node_id = document.find_node_before(offset);
        let mut curr = Some(node_id);
        while let Some(id) = curr {
            let n = document.node(id);
            if n.closed && (n.end_tag_start.is_none() || n.end_tag_start.unwrap() <= offset) {
                curr = n.parent;
                continue;
            }
            break;
        }
        if let Some(id) = curr {
            let n = document.node(id);
            if let Some(ref tag) = n.tag {
                let mut scanner = create_scanner(text, n.start, ScannerState::WithinContent, false);
                let mut token = scanner.scan();
                while token != TokenType::EOS && scanner.get_token_end() <= offset {
                    if token == TokenType::EndTagOpen && scanner.get_token_end() == offset {
                        return if offset < bytes.len() && bytes[offset] == b'>' {
                            Some(tag.clone())
                        } else {
                            Some(format!("{}>", tag))
                        };
                    }
                    token = scanner.scan();
                }
            }
        }
    }

    None
}
