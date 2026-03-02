use regex::Regex;
use once_cell::sync::Lazy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    StartCommentTag,
    Comment,
    EndCommentTag,
    StartTagOpen,
    StartTagClose,
    StartTagSelfClose,
    StartTag,
    EndTagOpen,
    EndTagClose,
    EndTag,
    DelimiterAssign,
    AttributeName,
    AttributeValue,
    StartDoctypeTag,
    Doctype,
    EndDoctypeTag,
    Content,
    Whitespace,
    Unknown,
    Script,
    Styles,
    EOS,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScannerState {
    WithinContent,
    AfterOpeningStartTag,
    AfterOpeningEndTag,
    WithinDoctype,
    WithinTag,
    WithinEndTag,
    WithinComment,
    WithinScriptContent,
    WithinStyleContent,
    AfterAttributeName,
    BeforeAttributeValue,
}

struct MultiLineStream {
    source: Vec<char>,
    len: usize,
    position: usize,
}

impl MultiLineStream {
    fn new(source: &str, position: usize) -> Self {
        let chars: Vec<char> = source.chars().collect();
        let len = chars.len();
        Self {
            source: chars,
            len,
            position,
        }
    }

    fn eos(&self) -> bool {
        self.len <= self.position
    }

    fn pos(&self) -> usize {
        self.position
    }

    fn go_back_to(&mut self, pos: usize) {
        self.position = pos;
    }

    fn go_back(&mut self, n: usize) {
        self.position = self.position.saturating_sub(n);
    }

    fn advance(&mut self, n: usize) {
        self.position += n;
    }

    fn go_to_end(&mut self) {
        self.position = self.len;
    }

    fn peek_char(&self, n: isize) -> char {
        let idx = self.position as isize + n;
        if idx < 0 || idx as usize >= self.len {
            '\0'
        } else {
            self.source[idx as usize]
        }
    }

    fn advance_if_char(&mut self, ch: char) -> bool {
        if self.position < self.len && self.source[self.position] == ch {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn advance_if_chars(&mut self, chars: &[char]) -> bool {
        if self.position + chars.len() > self.len {
            return false;
        }
        for (i, &ch) in chars.iter().enumerate() {
            if self.source[self.position + i] != ch {
                return false;
            }
        }
        self.position += chars.len();
        true
    }

    fn advance_if_regexp(&mut self, re: &Regex) -> String {
        let remaining: String = self.source[self.position..].iter().collect();
        if let Some(m) = re.find(&remaining) {
            let matched = m.as_str().to_string();
            self.position += m.end();
            matched
        } else {
            String::new()
        }
    }

    fn advance_until_regexp(&mut self, re: &Regex) -> String {
        let remaining: String = self.source[self.position..].iter().collect();
        if let Some(m) = re.find(&remaining) {
            self.position += m.start();
            m.as_str().to_string()
        } else {
            self.go_to_end();
            String::new()
        }
    }

    fn advance_until_char(&mut self, ch: char) -> bool {
        while self.position < self.len {
            if self.source[self.position] == ch {
                return true;
            }
            self.position += 1;
        }
        false
    }

    fn advance_until_chars(&mut self, chars: &[char]) -> bool {
        while self.position + chars.len() <= self.len {
            let mut matched = true;
            for (i, &ch) in chars.iter().enumerate() {
                if self.source[self.position + i] != ch {
                    matched = false;
                    break;
                }
            }
            if matched {
                return true;
            }
            self.position += 1;
        }
        self.go_to_end();
        false
    }

    fn skip_whitespace(&mut self) -> bool {
        let n = self.advance_while_char(|ch| {
            ch == ' ' || ch == '\t' || ch == '\n' || ch == '\x0C' || ch == '\r'
        });
        n > 0
    }

    fn advance_while_char<F: Fn(char) -> bool>(&mut self, condition: F) -> usize {
        let start = self.position;
        while self.position < self.len && condition(self.source[self.position]) {
            self.position += 1;
        }
        self.position - start
    }

    fn get_source(&self) -> String {
        self.source.iter().collect()
    }

    fn substring(&self, start: usize, end: usize) -> String {
        let s = start.min(self.len);
        let e = end.min(self.len);
        self.source[s..e].iter().collect()
    }
}

static RE_ELEMENT_NAME: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[_:\w][_:\w\-.\d]*").unwrap());
static RE_ATTRIBUTE_NAME: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^[^\s"'></=\x00-\x0F\x7F\x80-\x9F]*"#).unwrap());
static RE_UNQUOTED_VALUE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^[^\s"'`=<>]+"#).unwrap());
static RE_SCRIPT_TAG: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)<!--|-->|</?script\s*/?>?").unwrap());
static RE_STYLE_END: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)</style").unwrap());
static RE_DOCTYPE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^!doctype").unwrap());

fn is_html_script_content(type_value: &str) -> bool {
    matches!(type_value, "text/x-handlebars-template" | "text/html")
}

pub struct Scanner {
    stream: MultiLineStream,
    state: ScannerState,
    token_offset: usize,
    token_type: TokenType,
    token_error: Option<String>,
    has_space_after_tag: bool,
    last_tag: String,
    last_attribute_name: Option<String>,
    last_type_value: Option<String>,
    emit_pseudo_close_tags: bool,
}

impl Scanner {
    pub fn new(input: &str, initial_offset: usize, initial_state: ScannerState, emit_pseudo_close_tags: bool) -> Self {
        Self {
            stream: MultiLineStream::new(input, initial_offset),
            state: initial_state,
            token_offset: 0,
            token_type: TokenType::Unknown,
            token_error: None,
            has_space_after_tag: false,
            last_tag: String::new(),
            last_attribute_name: None,
            last_type_value: None,
            emit_pseudo_close_tags,
        }
    }

    fn next_element_name(&mut self) -> String {
        self.stream.advance_if_regexp(&RE_ELEMENT_NAME).to_lowercase()
    }

    fn next_attribute_name(&mut self) -> String {
        self.stream.advance_if_regexp(&RE_ATTRIBUTE_NAME).to_lowercase()
    }

    fn finish_token(&mut self, offset: usize, token_type: TokenType, error: Option<String>) -> TokenType {
        self.token_type = token_type;
        self.token_offset = offset;
        self.token_error = error;
        token_type
    }

    pub fn scan(&mut self) -> TokenType {
        let offset = self.stream.pos();
        let old_state = self.state;
        let token = self.internal_scan();
        if token != TokenType::EOS
            && offset == self.stream.pos()
            && !(self.emit_pseudo_close_tags
                && (token == TokenType::StartTagClose || token == TokenType::EndTagClose))
        {
            eprintln!(
                "Scanner.scan has not advanced at offset {}, state before: {:?} after: {:?}",
                offset, old_state, self.state
            );
            self.stream.advance(1);
            return self.finish_token(offset, TokenType::Unknown, None);
        }
        token
    }

    fn internal_scan(&mut self) -> TokenType {
        let offset = self.stream.pos();
        if self.stream.eos() {
            return self.finish_token(offset, TokenType::EOS, None);
        }

        match self.state {
            ScannerState::WithinComment => {
                if self.stream.advance_if_chars(&['-', '-', '>']) {
                    self.state = ScannerState::WithinContent;
                    return self.finish_token(offset, TokenType::EndCommentTag, None);
                }
                self.stream.advance_until_chars(&['-', '-', '>']);
                return self.finish_token(offset, TokenType::Comment, None);
            }
            ScannerState::WithinDoctype => {
                if self.stream.advance_if_char('>') {
                    self.state = ScannerState::WithinContent;
                    return self.finish_token(offset, TokenType::EndDoctypeTag, None);
                }
                self.stream.advance_until_char('>');
                return self.finish_token(offset, TokenType::Doctype, None);
            }
            ScannerState::WithinContent => {
                if self.stream.advance_if_char('<') {
                    if !self.stream.eos() && self.stream.peek_char(0) == '!' {
                        if self.stream.advance_if_chars(&['!', '-', '-']) {
                            self.state = ScannerState::WithinComment;
                            return self.finish_token(offset, TokenType::StartCommentTag, None);
                        }
                        if !self.stream.advance_if_regexp(&RE_DOCTYPE).is_empty() {
                            self.state = ScannerState::WithinDoctype;
                            return self.finish_token(offset, TokenType::StartDoctypeTag, None);
                        }
                    }
                    if self.stream.advance_if_char('/') {
                        self.state = ScannerState::AfterOpeningEndTag;
                        return self.finish_token(offset, TokenType::EndTagOpen, None);
                    }
                    self.state = ScannerState::AfterOpeningStartTag;
                    return self.finish_token(offset, TokenType::StartTagOpen, None);
                }
                self.stream.advance_until_char('<');
                return self.finish_token(offset, TokenType::Content, None);
            }
            ScannerState::AfterOpeningEndTag => {
                let tag_name = self.next_element_name();
                if !tag_name.is_empty() {
                    self.state = ScannerState::WithinEndTag;
                    return self.finish_token(offset, TokenType::EndTag, None);
                }
                if self.stream.skip_whitespace() {
                    return self.finish_token(
                        offset,
                        TokenType::Whitespace,
                        Some("Tag name must directly follow the open bracket.".to_string()),
                    );
                }
                self.state = ScannerState::WithinEndTag;
                self.stream.advance_until_char('>');
                if offset < self.stream.pos() {
                    return self.finish_token(
                        offset,
                        TokenType::Unknown,
                        Some("End tag name expected.".to_string()),
                    );
                }
                return self.internal_scan();
            }
            ScannerState::WithinEndTag => {
                if self.stream.skip_whitespace() {
                    return self.finish_token(offset, TokenType::Whitespace, None);
                }
                if self.stream.advance_if_char('>') {
                    self.state = ScannerState::WithinContent;
                    return self.finish_token(offset, TokenType::EndTagClose, None);
                }
                if self.emit_pseudo_close_tags && self.stream.peek_char(0) == '<' {
                    self.state = ScannerState::WithinContent;
                    return self.finish_token(
                        offset,
                        TokenType::EndTagClose,
                        Some("Closing bracket missing.".to_string()),
                    );
                }
                let error_message = Some("Closing bracket expected.".to_string());
                self.stream.advance(1);
                self.state = ScannerState::WithinContent;
                return self.finish_token(offset, TokenType::Unknown, error_message);
            }
            ScannerState::AfterOpeningStartTag => {
                self.last_tag = self.next_element_name();
                self.last_type_value = None;
                self.last_attribute_name = None;
                if !self.last_tag.is_empty() {
                    self.has_space_after_tag = false;
                    self.state = ScannerState::WithinTag;
                    return self.finish_token(offset, TokenType::StartTag, None);
                }
                if self.stream.skip_whitespace() {
                    return self.finish_token(
                        offset,
                        TokenType::Whitespace,
                        Some("Tag name must directly follow the open bracket.".to_string()),
                    );
                }
                self.state = ScannerState::WithinTag;
                self.stream.advance_until_char('>');
                if offset < self.stream.pos() {
                    return self.finish_token(
                        offset,
                        TokenType::Unknown,
                        Some("Start tag name expected.".to_string()),
                    );
                }
                return self.internal_scan();
            }
            ScannerState::WithinTag => {
                if self.stream.skip_whitespace() {
                    self.has_space_after_tag = true;
                    return self.finish_token(offset, TokenType::Whitespace, None);
                }
                if self.has_space_after_tag {
                    let attr = self.next_attribute_name();
                    if !attr.is_empty() {
                        self.last_attribute_name = Some(attr);
                        self.state = ScannerState::AfterAttributeName;
                        self.has_space_after_tag = false;
                        return self.finish_token(offset, TokenType::AttributeName, None);
                    }
                }
                if self.stream.advance_if_chars(&['/', '>']) {
                    self.state = ScannerState::WithinContent;
                    return self.finish_token(offset, TokenType::StartTagSelfClose, None);
                }
                if self.stream.advance_if_char('>') {
                    if self.last_tag == "script" {
                        if let Some(ref tv) = self.last_type_value {
                            if is_html_script_content(tv) {
                                self.state = ScannerState::WithinContent;
                            } else {
                                self.state = ScannerState::WithinScriptContent;
                            }
                        } else {
                            self.state = ScannerState::WithinScriptContent;
                        }
                    } else if self.last_tag == "style" {
                        self.state = ScannerState::WithinStyleContent;
                    } else {
                        self.state = ScannerState::WithinContent;
                    }
                    return self.finish_token(offset, TokenType::StartTagClose, None);
                }
                if self.emit_pseudo_close_tags && self.stream.peek_char(0) == '<' {
                    self.state = ScannerState::WithinContent;
                    return self.finish_token(
                        offset,
                        TokenType::StartTagClose,
                        Some("Closing bracket missing.".to_string()),
                    );
                }
                self.stream.advance(1);
                return self.finish_token(
                    offset,
                    TokenType::Unknown,
                    Some("Unexpected character in tag.".to_string()),
                );
            }
            ScannerState::AfterAttributeName => {
                if self.stream.skip_whitespace() {
                    self.has_space_after_tag = true;
                    return self.finish_token(offset, TokenType::Whitespace, None);
                }
                if self.stream.advance_if_char('=') {
                    self.state = ScannerState::BeforeAttributeValue;
                    return self.finish_token(offset, TokenType::DelimiterAssign, None);
                }
                self.state = ScannerState::WithinTag;
                return self.internal_scan();
            }
            ScannerState::BeforeAttributeValue => {
                if self.stream.skip_whitespace() {
                    return self.finish_token(offset, TokenType::Whitespace, None);
                }
                let mut attribute_value = self.stream.advance_if_regexp(&RE_UNQUOTED_VALUE);
                if !attribute_value.is_empty() {
                    if self.stream.peek_char(0) == '>' && self.stream.peek_char(-1) == '/' {
                        self.stream.go_back(1);
                        attribute_value = attribute_value[..attribute_value.len() - 1].to_string();
                    }
                    if let Some(ref attr_name) = self.last_attribute_name {
                        if attr_name == "type" {
                            self.last_type_value = Some(attribute_value.clone());
                        }
                    }
                    if !attribute_value.is_empty() {
                        self.state = ScannerState::WithinTag;
                        self.has_space_after_tag = false;
                        return self.finish_token(offset, TokenType::AttributeValue, None);
                    }
                }
                let ch = self.stream.peek_char(0);
                if ch == '\'' || ch == '"' {
                    self.stream.advance(1);
                    if self.stream.advance_until_char(ch) {
                        self.stream.advance(1);
                    }
                    if let Some(ref attr_name) = self.last_attribute_name {
                        if attr_name == "type" {
                            let src = self.stream.get_source();
                            let start = offset + 1;
                            let end = self.stream.pos().saturating_sub(1);
                            if start < end && end <= src.len() {
                                self.last_type_value = Some(src[start..end].to_string());
                            }
                        }
                    }
                    self.state = ScannerState::WithinTag;
                    self.has_space_after_tag = false;
                    return self.finish_token(offset, TokenType::AttributeValue, None);
                }
                self.state = ScannerState::WithinTag;
                self.has_space_after_tag = false;
                return self.internal_scan();
            }
            ScannerState::WithinScriptContent => {
                let mut script_state = 1;
                while !self.stream.eos() {
                    let matched = self.stream.advance_if_regexp(&RE_SCRIPT_TAG);
                    if matched.is_empty() {
                        self.stream.go_to_end();
                        self.state = ScannerState::WithinContent;
                        return self.finish_token(offset, TokenType::Script, None);
                    } else if matched == "<!--" {
                        if script_state == 1 {
                            script_state = 2;
                        }
                    } else if matched == "-->" {
                        script_state = 1;
                    } else if !matched.starts_with("</") {
                        // <script
                        if script_state == 2 {
                            script_state = 3;
                        }
                    } else {
                        // </script
                        if script_state == 3 {
                            script_state = 2;
                        } else {
                            self.stream.go_back(matched.len());
                            break;
                        }
                    }
                }
                self.state = ScannerState::WithinContent;
                if offset < self.stream.pos() {
                    return self.finish_token(offset, TokenType::Script, None);
                }
                return self.internal_scan();
            }
            ScannerState::WithinStyleContent => {
                self.stream.advance_until_regexp(&RE_STYLE_END);
                self.state = ScannerState::WithinContent;
                if offset < self.stream.pos() {
                    return self.finish_token(offset, TokenType::Styles, None);
                }
                return self.internal_scan();
            }
        }
    }

    pub fn get_token_type(&self) -> TokenType {
        self.token_type
    }

    pub fn get_token_offset(&self) -> usize {
        self.token_offset
    }

    pub fn get_token_length(&self) -> usize {
        self.stream.pos() - self.token_offset
    }

    pub fn get_token_end(&self) -> usize {
        self.stream.pos()
    }

    pub fn get_token_text(&self) -> String {
        self.stream.substring(self.token_offset, self.stream.pos())
    }

    pub fn get_token_error(&self) -> Option<&str> {
        self.token_error.as_deref()
    }

    pub fn get_scanner_state(&self) -> ScannerState {
        self.state
    }
}

pub fn create_scanner(input: &str, initial_offset: usize, initial_state: ScannerState, emit_pseudo_close_tags: bool) -> Scanner {
    Scanner::new(input, initial_offset, initial_state, emit_pseudo_close_tags)
}
