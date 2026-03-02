use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::parser::html_document::HtmlDocument;

/// A single XPath match result returned by `xpath_from_regex`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XPathMatch {
    /// The XPath expression that locates the containing element (e.g. `/html/body/div[2]/p[1]`).
    pub xpath: String,
    /// The byte offset in the document where the regex matched.
    pub offset: usize,
    /// The length of the matched text in bytes.
    pub length: usize,
    /// The matched text content.
    pub matched_text: String,
}

/// Build an XPath expression for the node at `node_id` in the given document.
///
/// Walks from the node up through its ancestors to the root, constructing a
/// path like `/html/body/div[2]/span`. The positional predicate `[n]` is
/// included only when there are multiple siblings with the same tag name
/// (1-indexed, matching XPath semantics).
fn build_xpath(document: &HtmlDocument, node_id: usize) -> String {
    // Collect ancestor chain (excluding the synthetic root node 0).
    let mut chain: Vec<usize> = Vec::new();
    let mut current = node_id;
    while current != 0 {
        chain.push(current);
        current = document.node(current).parent.unwrap_or(0);
    }
    chain.reverse();

    let mut xpath = String::new();
    for &id in &chain {
        let node = document.node(id);
        let tag = match &node.tag {
            Some(t) => t.to_lowercase(),
            None => continue,
        };

        // Determine sibling index among same-tag siblings.
        let (position, same_tag_count) = if let Some(parent_id) = node.parent {
            let parent = document.node(parent_id);
            let mut pos = 0;
            let mut total = 0;
            for &sibling_id in &parent.children {
                let sibling = document.node(sibling_id);
                if sibling.tag.as_ref().map(|t| t.to_lowercase()).as_deref() == Some(&tag) {
                    total += 1;
                    if sibling_id == id {
                        pos = total;
                    }
                }
            }
            (pos, total)
        } else {
            (1, 1)
        };

        xpath.push('/');
        xpath.push_str(&tag);
        if same_tag_count > 1 {
            xpath.push('[');
            xpath.push_str(&position.to_string());
            xpath.push(']');
        }
    }

    if xpath.is_empty() {
        "/".to_string()
    } else {
        xpath
    }
}

/// Find all regex matches in `text` and return the XPath of the innermost
/// element that contains each match.
///
/// # Arguments
/// * `text` – the full HTML document source
/// * `document` – the parsed HTML tree
/// * `pattern` – a regular expression pattern to search for
///
/// # Returns
/// A list of [`XPathMatch`] results, or an error string if the regex is invalid.
pub fn xpath_from_regex(
    text: &str,
    document: &HtmlDocument,
    pattern: &str,
) -> Result<Vec<XPathMatch>, String> {
    let re = Regex::new(pattern).map_err(|e| format!("Invalid regex: {e}"))?;

    let mut results = Vec::new();
    for m in re.find_iter(text) {
        let offset = m.start();
        let node_id = document.find_node_at(offset);
        let xpath = build_xpath(document, node_id);

        results.push(XPathMatch {
            xpath,
            offset,
            length: m.len(),
            matched_text: m.as_str().to_string(),
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::html_document;

    fn parse(text: &str) -> HtmlDocument {
        let void_elements = vec![
            "br".to_string(),
            "hr".to_string(),
            "img".to_string(),
            "input".to_string(),
        ];
        html_document::parse(text, &void_elements)
    }

    #[test]
    fn simple_xpath() {
        let text = "<html><body><p>hello world</p></body></html>";
        let doc = parse(text);
        let results = xpath_from_regex(text, &doc, "hello").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].xpath, "/html/body/p");
        assert_eq!(results[0].matched_text, "hello");
    }

    #[test]
    fn sibling_index() {
        let text = "<html><body><div>first</div><div>second</div></body></html>";
        let doc = parse(text);
        let results = xpath_from_regex(text, &doc, "second").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].xpath, "/html/body/div[2]");
    }

    #[test]
    fn multiple_matches() {
        let text = "<html><body><p>foo</p><p>foo</p></body></html>";
        let doc = parse(text);
        let results = xpath_from_regex(text, &doc, "foo").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].xpath, "/html/body/p[1]");
        assert_eq!(results[1].xpath, "/html/body/p[2]");
    }

    #[test]
    fn invalid_regex_returns_error() {
        let text = "<html><body></body></html>";
        let doc = parse(text);
        let result = xpath_from_regex(text, &doc, "[invalid");
        assert!(result.is_err());
    }

    #[test]
    fn no_matches_returns_empty() {
        let text = "<html><body><p>hello</p></body></html>";
        let doc = parse(text);
        let results = xpath_from_regex(text, &doc, "notfound").unwrap();
        assert!(results.is_empty());
    }
}
