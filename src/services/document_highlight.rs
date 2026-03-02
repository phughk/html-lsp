use tower_lsp::lsp_types::*;

use crate::parser::html_document::HtmlDocument;
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

fn get_tag_name_range(text: &str, token_type: TokenType, start_offset: usize) -> Option<Range> {
    let mut scanner = create_scanner(text, start_offset, ScannerState::WithinContent, false);
    let mut token = scanner.scan();
    while token != TokenType::EOS && token != token_type {
        token = scanner.scan();
    }
    if token != TokenType::EOS {
        Some(Range::new(
            position_at(text, scanner.get_token_offset()),
            position_at(text, scanner.get_token_end()),
        ))
    } else {
        None
    }
}

fn covers(range: &Range, position: &Position) -> bool {
    (range.start.line < position.line
        || (range.start.line == position.line && range.start.character <= position.character))
        && (position.line < range.end.line
            || (position.line == range.end.line && position.character <= range.end.character))
}

pub fn find_document_highlights(
    text: &str,
    position: Position,
    document: &HtmlDocument,
) -> Vec<DocumentHighlight> {
    let offset = offset_at(text, position);
    let node_id = document.find_node_at(offset);
    let node = document.node(node_id);

    if node.tag.is_none() {
        return vec![];
    }

    let mut result = Vec::new();
    let start_tag_range = get_tag_name_range(text, TokenType::StartTag, node.start);
    let end_tag_range = node
        .end_tag_start
        .and_then(|ets| get_tag_name_range(text, TokenType::EndTag, ets));

    let in_start = start_tag_range.as_ref().map_or(false, |r| covers(r, &position));
    let in_end = end_tag_range.as_ref().map_or(false, |r| covers(r, &position));

    if in_start || in_end {
        if let Some(r) = start_tag_range {
            result.push(DocumentHighlight {
                range: r,
                kind: Some(DocumentHighlightKind::READ),
            });
        }
        if let Some(r) = end_tag_range {
            result.push(DocumentHighlight {
                range: r,
                kind: Some(DocumentHighlightKind::READ),
            });
        }
    }

    result
}
