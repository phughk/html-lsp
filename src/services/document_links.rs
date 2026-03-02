use tower_lsp::lsp_types::*;

use crate::language_facts::data_manager::DataManager;
use crate::parser::scanner::{create_scanner, ScannerState, TokenType};

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

fn normalize_ref(url: &str) -> &str {
    let bytes = url.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if first == last && (first == b'"' || first == b'\'') {
            return &url[1..url.len() - 1];
        }
    }
    url
}

fn validate_ref(url: &str) -> bool {
    !url.is_empty()
}

pub fn find_document_links(
    text: &str,
    data_manager: &DataManager,
    uri: &Url,
) -> Vec<DocumentLink> {
    let mut links = Vec::new();
    let mut scanner = create_scanner(text, 0, ScannerState::WithinContent, false);
    let mut token = scanner.scan();
    let mut last_attribute_name: Option<String> = None;
    let mut last_tag_name: Option<String> = None;

    while token != TokenType::EOS {
        match token {
            TokenType::StartTag => {
                last_tag_name = Some(scanner.get_token_text().to_lowercase());
            }
            TokenType::AttributeName => {
                last_attribute_name = Some(scanner.get_token_text().to_lowercase());
            }
            TokenType::AttributeValue => {
                if let (Some(ref tag), Some(ref attr)) = (&last_tag_name, &last_attribute_name) {
                    if data_manager.is_path_attribute(tag, attr) {
                        let value = scanner.get_token_text();
                        let content = normalize_ref(&value);
                        if validate_ref(content) {
                            let mut start_offset = scanner.get_token_offset();
                            let mut end_offset = scanner.get_token_end();

                            // Adjust for quotes
                            if content.len() < value.len() {
                                start_offset += 1;
                                end_offset -= 1;
                            }

                            // Try to parse as URL
                            let target = if content.starts_with("http://")
                                || content.starts_with("https://")
                                || content.starts_with("//")
                            {
                                let full = if content.starts_with("//") {
                                    format!("https:{}", content)
                                } else {
                                    content.to_string()
                                };
                                Url::parse(&full).ok()
                            } else if content.starts_with('#') {
                                // Fragment reference
                                let mut target = uri.clone();
                                target.set_fragment(Some(&content[1..]));
                                Some(target)
                            } else {
                                // Relative URL
                                uri.join(content).ok()
                            };

                            if let Some(target) = target {
                                links.push(DocumentLink {
                                    range: Range::new(
                                        position_at(text, start_offset),
                                        position_at(text, end_offset),
                                    ),
                                    target: Some(target),
                                    tooltip: None,
                                    data: None,
                                });
                            }
                        }
                    }
                }
                last_attribute_name = None;
            }
            _ => {}
        }
        token = scanner.scan();
    }

    links
}
