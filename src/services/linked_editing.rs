use tower_lsp::lsp_types::*;

use crate::parser::html_document::HtmlDocument;

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

pub fn find_linked_editing_ranges(
    text: &str,
    position: Position,
    document: &HtmlDocument,
) -> Option<LinkedEditingRanges> {
    let offset = offset_at(text, position);
    let node_id = document.find_node_at(offset);
    let node = document.node(node_id);

    let tag_length = node.tag.as_ref().map_or(0, |t| t.len());
    let end_tag_start = node.end_tag_start?;

    let in_open = node.start + 1 <= offset && offset <= node.start + 1 + tag_length;
    let in_close = end_tag_start + 2 <= offset && offset <= end_tag_start + 2 + tag_length;

    if in_open || in_close {
        Some(LinkedEditingRanges {
            ranges: vec![
                Range::new(
                    position_at(text, node.start + 1),
                    position_at(text, node.start + 1 + tag_length),
                ),
                Range::new(
                    position_at(text, end_tag_start + 2),
                    position_at(text, end_tag_start + 2 + tag_length),
                ),
            ],
            word_pattern: None,
        })
    } else {
        None
    }
}
