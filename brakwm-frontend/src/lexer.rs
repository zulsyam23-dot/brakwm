use brakwm_core::SourceMap;
use brakwm_ir_ast::ast::Type;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TokenKind {
    // Keywords
    Fn,
    Let,
    Mut,
    If,
    Else,
    While,
    For,
    In,
    Loop,
    Break,
    Continue,
    Return,
    Struct,
    Enum,
    Match,
    Const,
    Extern,
    True,
    False,
    Underscore,
    // Literals
    Int(i64),
    Float(f64),
    Str(String),
    // Identifiers and types
    Ident(String),
    Type(Type),
    // Punctuation / operators
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Semicolon,
    Dot,
    Arrow,       // ->
    FatArrow,    // =>
    Assign,      // =
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,          // ==
    Ne,          // !=
    Lt,          // <
    Le,          // <=
    Gt,          // >
    Ge,          // >=
    AndAnd,      // &&
    OrOr,        // ||
    Amp,         // &
    Pipe,        // |
    Caret,       // ^
    Shl,         // <<
    Shr,         // >>
    Bang,        // !
    Tilde,       // ~
    Range,       // ..
    Eof,
}

impl std::fmt::Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub offset: usize,
    pub len: usize,
}

pub trait BrakLexer {
    fn lex(&mut self, sm: &SourceMap) -> Vec<Token>;
}

/// A simple ASCII-oriented lexer producing the shared [`Token`] stream.
pub struct AsciiLexer;

impl AsciiLexer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AsciiLexer {
    fn default() -> Self {
        Self::new()
    }
}

fn keyword(word: &str) -> Option<TokenKind> {
    Some(match word {
        "fn" => TokenKind::Fn,
        "let" => TokenKind::Let,
        "mut" => TokenKind::Mut,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "while" => TokenKind::While,
        "for" => TokenKind::For,
        "in" => TokenKind::In,
        "loop" => TokenKind::Loop,
        "break" => TokenKind::Break,
        "continue" => TokenKind::Continue,
        "return" => TokenKind::Return,
        "struct" => TokenKind::Struct,
        "enum" => TokenKind::Enum,
        "match" => TokenKind::Match,
        "const" => TokenKind::Const,
        "extern" => TokenKind::Extern,
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        "_" => TokenKind::Underscore,
        _ => return None,
    })
}

fn type_token(word: &str) -> Option<TokenKind> {
    Some(match word {
        "i32" => TokenKind::Type(Type::I32),
        "i64" => TokenKind::Type(Type::I64),
        "f32" => TokenKind::Type(Type::F32),
        "f64" => TokenKind::Type(Type::F64),
        "bool" => TokenKind::Type(Type::Bool),
        "string" => TokenKind::Type(Type::String),
        "void" => TokenKind::Type(Type::Void),
        _ => return None,
    })
}

impl BrakLexer for AsciiLexer {
    fn lex(&mut self, sm: &SourceMap) -> Vec<Token> {
        let src: Vec<char> = sm.source().chars().collect();
        let n = src.len();
        let mut i = 0usize;
        let mut out = Vec::new();

        // Skip a leading UTF-8 BOM so files saved by Windows editors parse.
        if n >= 1 && src[0] == '\u{feff}' {
            i += 1;
        }

        while i < n {
            let c = src[i];

            // Whitespace
            if c.is_whitespace() {
                i += 1;
                continue;
            }

            // Comments: // line, /* block */
            if c == '/' && i + 1 < n && src[i + 1] == '/' {
                while i < n && src[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            if c == '/' && i + 1 < n && src[i + 1] == '*' {
                i += 2;
                while i + 1 < n && !(src[i] == '*' && src[i + 1] == '/') {
                    i += 1;
                }
                i = (i + 2).min(n);
                continue;
            }

            let start = i;

            // Identifier / keyword / type
            if c.is_alphabetic() || c == '_' {
                while i < n && (src[i].is_alphanumeric() || src[i] == '_') {
                    i += 1;
                }
                let word: String = src[start..i].iter().collect();
                let kind = keyword(&word)
                    .or_else(|| type_token(&word))
                    .unwrap_or(TokenKind::Ident(word.clone()));
                out.push(Token { kind, lexeme: word, offset: start, len: i - start });
                continue;
            }

            // Numbers
            if c.is_ascii_digit() {
                let mut is_float = false;
                while i < n && (src[i].is_ascii_digit() || src[i] == '_') {
                    i += 1;
                }
                if i < n && src[i] == '.' && i + 1 < n && src[i + 1].is_ascii_digit() {
                    is_float = true;
                    i += 1;
                    while i < n && (src[i].is_ascii_digit() || src[i] == '_') {
                        i += 1;
                    }
                }
                let text: String = src[start..i].iter().collect();
                let cleaned: String = text.chars().filter(|ch| *ch != '_').collect();
                let kind = if is_float {
                    TokenKind::Float(cleaned.parse().unwrap_or(0.0))
                } else {
                    let v: i64 = cleaned.parse().unwrap_or(0);
                    TokenKind::Int(v)
                };
                out.push(Token { kind, lexeme: text.clone(), offset: start, len: i - start });
                continue;
            }

            // Strings
            if c == '"' {
                i += 1;
                let mut s = String::new();
                while i < n && src[i] != '"' {
                    let ch = src[i];
                    if ch == '\\' && i + 1 < n {
                        let esc = src[i + 1];
                        match esc {
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            'r' => s.push('\r'),
                            '"' => s.push('"'),
                            '\\' => s.push('\\'),
                            '0' => s.push('\0'),
                            other => {
                                s.push('\\');
                                s.push(other);
                            }
                        }
                        i += 2;
                    } else {
                        s.push(ch);
                        i += 1;
                    }
                }
                if i < n {
                    i += 1; // closing quote
                }
                out.push(Token {
                    kind: TokenKind::Str(s.clone()),
                    lexeme: s,
                    offset: start,
                    len: i - start,
                });
                continue;
            }

            // Multi-char operators
            let two = if i + 1 < n {
                Some((c, src[i + 1]))
            } else {
                None
            };
            let (kind, adv): (TokenKind, usize) = match two {
                Some(('-', '>')) => (TokenKind::Arrow, 2),
                Some(('=', '>')) => (TokenKind::FatArrow, 2),
                Some(('=', '=')) => (TokenKind::Eq, 2),
                Some(('!', '=')) => (TokenKind::Ne, 2),
                Some(('<', '=')) => (TokenKind::Le, 2),
                Some(('>', '=')) => (TokenKind::Ge, 2),
                Some(('&', '&')) => (TokenKind::AndAnd, 2),
                Some(('|', '|')) => (TokenKind::OrOr, 2),
                Some(('<', '<')) => (TokenKind::Shl, 2),
                Some(('>', '>')) => (TokenKind::Shr, 2),
                Some(('.', '.')) => (TokenKind::Range, 2),
                _ => match c {
                    '(' => (TokenKind::LParen, 1),
                    ')' => (TokenKind::RParen, 1),
                    '{' => (TokenKind::LBrace, 1),
                    '}' => (TokenKind::RBrace, 1),
                    '[' => (TokenKind::LBracket, 1),
                    ']' => (TokenKind::RBracket, 1),
                    ',' => (TokenKind::Comma, 1),
                    ':' => (TokenKind::Colon, 1),
                    ';' => (TokenKind::Semicolon, 1),
                    '.' => (TokenKind::Dot, 1),
                    '=' => (TokenKind::Assign, 1),
                    '+' => (TokenKind::Plus, 1),
                    '-' => (TokenKind::Minus, 1),
                    '*' => (TokenKind::Star, 1),
                    '/' => (TokenKind::Slash, 1),
                    '%' => (TokenKind::Percent, 1),
                    '<' => (TokenKind::Lt, 1),
                    '>' => (TokenKind::Gt, 1),
                    '&' => (TokenKind::Amp, 1),
                    '|' => (TokenKind::Pipe, 1),
                    '^' => (TokenKind::Caret, 1),
                    '!' => (TokenKind::Bang, 1),
                    '~' => (TokenKind::Tilde, 1),
                    other => {
                        out.push(Token {
                            kind: TokenKind::Ident(format!("<invalid:{other}>")),
                            lexeme: other.to_string(),
                            offset: start,
                            len: 1,
                        });
                        (TokenKind::Eof, 1)
                    }
                },
            };
            out.push(Token {
                kind,
                lexeme: src[start..start + adv].iter().collect(),
                offset: start,
                len: adv,
            });
            i += adv;
        }

        out.push(Token {
            kind: TokenKind::Eof,
            lexeme: String::new(),
            offset: n,
            len: 0,
        });
        out
    }
}

impl Token {
    pub fn is(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(&self.kind) == std::mem::discriminant(kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(s: &str) -> Vec<Token> {
        let sm = SourceMap::new("t.brk", s);
        AsciiLexer::new().lex(&sm)
    }

    #[test]
    fn lexes_keywords_and_identifiers() {
        let ts = lex("fn main() -> i32 { 42 }");
        let kinds: Vec<_> = ts.iter().map(|t| t.kind.clone()).collect();
        assert!(kinds.contains(&TokenKind::Fn));
        assert!(kinds.contains(&TokenKind::Ident("main".into())));
        assert!(kinds.contains(&TokenKind::Type(Type::I32)));
        assert!(ts.iter().any(|t| t.is(&TokenKind::Int(42))));
    }

    #[test]
    fn lexes_strings_and_floats() {
        let ts = lex(r#"let s = "halo"; let x = 3.14;"#);
        assert!(ts.iter().any(|t| t.is(&TokenKind::Str("halo".into()))));
        assert!(ts.iter().any(|t| matches!(t.kind, TokenKind::Float(v) if (v - 3.14).abs() < 1e-9)));
    }

    #[test]
    fn skips_comments() {
        let ts = lex("// c\nfn f() {} /* b */ let x = 1;");
        assert!(!ts.iter().any(|t| t.lexeme.contains("c")));
    }
}