use serde::{Deserialize, Serialize};

/// Custom capabilities beyond the standard LSP spec, designed for coding-agent
/// workflows (e.g. programmatic DOM navigation and content discovery).
///
/// Advertised via `ServerCapabilities.experimental` so that clients can
/// feature-detect before issuing custom requests.
#[derive(Serialize, Deserialize)]
pub struct CustomCapabilities {
    /// When `true`, the server supports the `xpath_from_regex` execute-command.
    ///
    /// ## `workspace/executeCommand` — `xpath_from_regex`
    ///
    /// Search a document's text for a regex pattern and return the XPath of
    /// every element that contains a match.
    ///
    /// ### Arguments (positional JSON array)
    ///
    /// | Index | Type   | Description                                        |
    /// |-------|--------|----------------------------------------------------|
    /// | 0     | string | Document URI (must be an open `textDocument`)       |
    /// | 1     | string | Regular expression pattern (Rust `regex` syntax)    |
    ///
    /// ### Response
    ///
    /// Returns `Value::Array` of objects, each containing:
    ///
    /// | Field          | Type   | Description                                   |
    /// |----------------|--------|-----------------------------------------------|
    /// | `xpath`        | string | XPath to the innermost containing element     |
    /// | `offset`       | number | Byte offset of the match in the source text   |
    /// | `length`       | number | Byte length of the matched text               |
    /// | `matched_text` | string | The text that matched the regex               |
    ///
    /// ### Example request
    ///
    /// ```json
    /// {
    ///   "command": "xpath_from_regex",
    ///   "arguments": ["file:///path/to/file.html", "TODO|FIXME"]
    /// }
    /// ```
    pub xpath_from_regex: bool,
}