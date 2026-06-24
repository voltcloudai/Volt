use crate::diagnostics::{Diagnostic, DiagnosticBag, SourceFile, Span};

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Int(i64),
    Float(f64),
    String(String),
    Function,
    Type,
    Const,
    Let,
    Return,
    Route,
    Try,
    If,
    Else,
    True,
    False,
    Colon,
    Comma,
    Dot,
    Ellipsis,
    Equals,
    Plus,
    Minus,
    Star,
    Slash,
    EqEqEq,
    BangEqEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Eof,

    Import,
    Export,
    From,
}

pub fn lex(source: &SourceFile) -> Result<Vec<Token>, DiagnosticBag> {
    let mut lexer = Lexer {
        source,
        chars: source.source.char_indices().peekable(),
        diagnostics: DiagnosticBag::new(),
        tokens: Vec::new(),
    };
    lexer.run();

    if lexer.diagnostics.is_empty() {
        Ok(lexer.tokens)
    } else {
        Err(lexer.diagnostics)
    }
}

struct Lexer<'a> {
    source: &'a SourceFile,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    diagnostics: DiagnosticBag,
    tokens: Vec<Token>,
}

impl Lexer<'_> {
    fn run(&mut self) {
        while let Some((start, ch)) = self.chars.next() {
            match ch {
                c if c.is_whitespace() => {}
                '/' if self.consume_if('/') => self.skip_line_comment(),
                ':' => self.push(TokenKind::Colon, start, start + 1),
                ',' => self.push(TokenKind::Comma, start, start + 1),
                '.' if self.consume_if('.') && self.consume_if('.') => {
                    self.push(TokenKind::Ellipsis, start, start + 3)
                }
                '.' => self.push(TokenKind::Dot, start, start + 1),
                '+' => self.push(TokenKind::Plus, start, start + 1),
                '-' => self.push(TokenKind::Minus, start, start + 1),
                '*' => self.push(TokenKind::Star, start, start + 1),
                '/' => self.push(TokenKind::Slash, start, start + 1),
                '(' => self.push(TokenKind::LParen, start, start + 1),
                ')' => self.push(TokenKind::RParen, start, start + 1),
                '{' => self.push(TokenKind::LBrace, start, start + 1),
                '}' => self.push(TokenKind::RBrace, start, start + 1),
                '[' => self.push(TokenKind::LBracket, start, start + 1),
                ']' => self.push(TokenKind::RBracket, start, start + 1),
                '=' => self.equals(start),
                '!' => self.bang(start),
                '<' if self.consume_if('=') => self.push(TokenKind::LtEq, start, start + 2),
                '>' if self.consume_if('=') => self.push(TokenKind::GtEq, start, start + 2),
                '<' => self.push(TokenKind::Lt, start, start + 1),
                '>' => self.push(TokenKind::Gt, start, start + 1),
                '"' => self.string(start),
                c if c.is_ascii_digit() => self.number(start, c),
                c if is_ident_start(c) => self.ident(start, c),
                _ => self.diagnostics.push(Diagnostic::new(
                    "E001",
                    format!("unexpected character `{ch}`"),
                    self.source,
                    Span::new(start, start + ch.len_utf8()),
                    Some("remove this character or use supported Volt syntax".to_string()),
                )),
            }
        }

        let end = self.source.source.len();
        self.push(TokenKind::Eof, end, end);
    }

    fn push(&mut self, kind: TokenKind, start: usize, end: usize) {
        self.tokens.push(Token {
            kind,
            span: Span::new(start, end),
        });
    }

    fn consume_if(&mut self, expected: char) -> bool {
        if self.chars.peek().is_some_and(|(_, ch)| *ch == expected) {
            self.chars.next();
            true
        } else {
            false
        }
    }

    fn skip_line_comment(&mut self) {
        while let Some((_, ch)) = self.chars.peek() {
            if *ch == '\n' {
                break;
            }
            self.chars.next();
        }
    }

    fn equals(&mut self, start: usize) {
        if self.consume_if('=') {
            if self.consume_if('=') {
                self.push(TokenKind::EqEqEq, start, start + 3);
            } else {
                self.diagnostics.push(Diagnostic::new(
                    "E004",
                    "`==` is not supported",
                    self.source,
                    Span::new(start, start + 2),
                    Some("use `===` for equality".to_string()),
                ));
            }
        } else {
            self.push(TokenKind::Equals, start, start + 1);
        }
    }

    fn bang(&mut self, start: usize) {
        if self.consume_if('=') {
            if self.consume_if('=') {
                self.push(TokenKind::BangEqEq, start, start + 3);
            } else {
                self.diagnostics.push(Diagnostic::new(
                    "E005",
                    "`!=` is not supported",
                    self.source,
                    Span::new(start, start + 2),
                    Some("use `!==` for inequality".to_string()),
                ));
            }
        } else {
            self.diagnostics.push(Diagnostic::new(
                "E006",
                "unexpected character `!`",
                self.source,
                Span::new(start, start + 1),
                Some("Volt v0.1 does not have unary `!` yet".to_string()),
            ));
        }
    }

    fn string(&mut self, start: usize) {
        let mut value = String::new();
        let mut end = start + 1;
        while let Some((idx, ch)) = self.chars.next() {
            end = idx + ch.len_utf8();
            match ch {
                '"' => {
                    self.push(TokenKind::String(value), start, end);
                    return;
                }
                '\\' => {
                    if let Some((esc_idx, esc)) = self.chars.next() {
                        end = esc_idx + esc.len_utf8();
                        let escaped = match esc {
                            'n' => '\n',
                            't' => '\t',
                            'r' => '\r',
                            '"' => '"',
                            '\\' => '\\',
                            other => other,
                        };
                        value.push(escaped);
                    }
                }
                other => value.push(other),
            }
        }

        self.diagnostics.push(Diagnostic::new(
            "E002",
            "unterminated string literal",
            self.source,
            Span::new(start, end),
            Some("add a closing double quote".to_string()),
        ));
    }

    fn number(&mut self, start: usize, first: char) {
        let mut text = first.to_string();
        let mut end = start + first.len_utf8();
        let mut has_dot = false;

        while let Some((idx, ch)) = self.chars.peek().copied() {
            if ch.is_ascii_digit() {
                text.push(ch);
                end = idx + ch.len_utf8();
                self.chars.next();
            } else if ch == '.' && !has_dot {
                has_dot = true;
                text.push(ch);
                end = idx + 1;
                self.chars.next();
            } else {
                break;
            }
        }

        if has_dot {
            match text.parse::<f64>() {
                Ok(value) => self.push(TokenKind::Float(value), start, end),
                Err(_) => self.invalid_number(start, end),
            }
        } else {
            match text.parse::<i64>() {
                Ok(value) => self.push(TokenKind::Int(value), start, end),
                Err(_) => self.invalid_number(start, end),
            }
        }
    }

    fn invalid_number(&mut self, start: usize, end: usize) {
        self.diagnostics.push(Diagnostic::new(
            "E003",
            "invalid number literal",
            self.source,
            Span::new(start, end),
            Some("use a valid integer or float literal".to_string()),
        ));
    }

    fn ident(&mut self, start: usize, first: char) {
        let mut text = first.to_string();
        let mut end = start + first.len_utf8();
        while let Some((idx, ch)) = self.chars.peek().copied() {
            if is_ident_continue(ch) {
                text.push(ch);
                end = idx + ch.len_utf8();
                self.chars.next();
            } else {
                break;
            }
        }

        let kind = match text.as_str() {
            "function" => TokenKind::Function,
            "type" => TokenKind::Type,
            "const" => TokenKind::Const,
            "let" => TokenKind::Let,
            "return" => TokenKind::Return,
            "route" => TokenKind::Route,
            "try" => TokenKind::Try,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "import" => TokenKind::Import,
            "export" => TokenKind::Export,
            "from" => TokenKind::From,

            _ => TokenKind::Ident(text),
        };
        self.push(kind, start, end);
    }
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}
