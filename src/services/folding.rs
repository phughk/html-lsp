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

pub fn get_folding_ranges(
    text: &str,
    data_manager: &DataManager,
    range_limit: Option<u32>,
) -> Vec<FoldingRange> {
    let mut scanner = create_scanner(text, 0, ScannerState::WithinContent, false);
    let mut token = scanner.scan();
    let mut ranges: Vec<FoldingRange> = Vec::new();
    let mut stack: Vec<(u32, String)> = Vec::new(); // (startLine, tagName)
    let mut last_tag_name: Option<String> = None;
    let mut prev_start: i64 = -1;

    while token != TokenType::EOS {
        match token {
            TokenType::StartTag => {
                let tag_name = scanner.get_token_text();
                let start_line = position_at(text, scanner.get_token_offset()).line;
                stack.push((start_line, tag_name.clone()));
                last_tag_name = Some(tag_name);
            }
            TokenType::EndTag => {
                last_tag_name = Some(scanner.get_token_text());
            }
            TokenType::StartTagClose => {
                if let Some(ref tag) = last_tag_name {
                    if data_manager.is_void_element(tag) {
                        // Pop from stack like EndTagClose
                        let tag_clone = tag.clone();
                        pop_and_fold(&mut stack, &tag_clone, &mut ranges, text, &scanner, &mut prev_start);
                    }
                }
            }
            TokenType::EndTagClose | TokenType::StartTagSelfClose => {
                if let Some(ref tag) = last_tag_name {
                    let tag_clone = tag.clone();
                    pop_and_fold(&mut stack, &tag_clone, &mut ranges, text, &scanner, &mut prev_start);
                }
            }
            TokenType::Comment => {
                let start_line = position_at(text, scanner.get_token_offset()).line;
                let comment_text = scanner.get_token_text();

                if comment_text.contains("#region") {
                    stack.push((start_line, String::new()));
                } else if comment_text.contains("#endregion") {
                    // Find matching region
                    let mut i = stack.len();
                    while i > 0 {
                        i -= 1;
                        if stack[i].1.is_empty() {
                            let (region_start, _) = stack.remove(i);
                            if start_line > region_start && prev_start != region_start as i64 {
                                ranges.push(FoldingRange {
                                    start_line: region_start,
                                    start_character: None,
                                    end_line: start_line,
                                    end_character: None,
                                    kind: Some(FoldingRangeKind::Region),
                                    collapsed_text: None,
                                });
                                prev_start = region_start as i64;
                            }
                            break;
                        }
                    }
                } else {
                    let end_line = position_at(text, scanner.get_token_offset() + scanner.get_token_length()).line;
                    if start_line < end_line {
                        ranges.push(FoldingRange {
                            start_line,
                            start_character: None,
                            end_line,
                            end_character: None,
                            kind: Some(FoldingRangeKind::Comment),
                            collapsed_text: None,
                        });
                        prev_start = start_line as i64;
                    }
                }
            }
            _ => {}
        }
        token = scanner.scan();
    }

    let limit = range_limit.unwrap_or(u32::MAX) as usize;
    if ranges.len() > limit {
        ranges.truncate(limit);
    }

    ranges
}

fn pop_and_fold(
    stack: &mut Vec<(u32, String)>,
    tag_name: &str,
    ranges: &mut Vec<FoldingRange>,
    text: &str,
    scanner: &crate::parser::scanner::Scanner,
    prev_start: &mut i64,
) {
    let mut i = stack.len();
    while i > 0 {
        i -= 1;
        if stack[i].1 == tag_name {
            let (start_line, _) = stack.remove(i);
            stack.truncate(i);
            let line = position_at(text, scanner.get_token_offset()).line;
            let end_line = line.saturating_sub(1);
            if end_line > start_line && *prev_start != start_line as i64 {
                ranges.push(FoldingRange {
                    start_line,
                    start_character: None,
                    end_line,
                    end_character: None,
                    kind: None,
                    collapsed_text: None,
                });
                *prev_start = start_line as i64;
            }
            break;
        }
    }
}
