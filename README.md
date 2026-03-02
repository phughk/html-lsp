# html-lsp

A CLI HTML Language Server written in Rust, communicating via JSON-RPC over stdin/stdout. Implements the [Language Server Protocol](https://microsoft.github.io/language-server-protocol/) for HTML, porting the logic from VS Code's [vscode-html-languageservice](https://github.com/microsoft/vscode-html-languageservice).

## Features

- **Completion** — tags, attributes, attribute values, HTML entities, auto-close tags, DOCTYPE
- **Hover** — documentation for tags, attributes, attribute values, and entities (with MDN references)
- **Document Symbols** — hierarchical outline with CSS-style selectors (`div#id.class`)
- **Document Highlights** — matching open/close tag highlighting
- **Folding Ranges** — element-based, comment, and region folding
- **Rename** — tag pair rename (open + close tag)
- **Linked Editing** — synchronized tag name editing
- **Document Links** — detection of URLs in `href`, `src`, and other path attributes
- **Formatting** — indentation-based HTML formatting

## Building

```sh
cargo build --release
```

The binary is output to `./target/release/html-lsp`.

## Usage

The server communicates over stdin/stdout using the LSP JSON-RPC protocol. Point any LSP-compatible editor at the binary.

### Neovim (nvim-lspconfig)

```lua
vim.lsp.start({
  name = "html-lsp",
  cmd = { "/path/to/html-lsp" },
  filetypes = { "html" },
})
```

### VS Code

Use a generic LSP client extension and configure it to run the `html-lsp` binary for HTML files.

### Helix

In `languages.toml`:

```toml
[[language]]
name = "html"
language-servers = ["html-lsp"]

[language-server.html-lsp]
command = "/path/to/html-lsp"
```

## HTML Data

Ships with the full HTML5 tag and attribute dataset (116 tags, 150 global attributes, 41 value sets) derived from the [`@vscode/web-custom-data`](https://github.com/nicolo-ribaudo/vscode-web-custom-data) package. This includes descriptions, browser compatibility info, and MDN references.

## License

BSD 3-Clause. See [LICENSE](LICENSE).
