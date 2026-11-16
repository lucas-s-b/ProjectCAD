use crate::value::ParamError;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Num(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    Comma,
    Lt,
    Le,
    Gt,
    Ge,
    EqEq,
    Ne,
}

/// Tokens with their byte positions.
pub fn lex(src: &str) -> Result<Vec<(Tok, usize)>, ParamError> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let start = i;
        let two = |next: u8| b.get(i + 1) == Some(&next);
        let tok = match c {
            b' ' | b'\t' | b'\r' | b'\n' => {
                i += 1;
                continue;
            }
            b'0'..=b'9' | b'.' => {
                i = lex_number(b, i);
                let text = &src[start..i];
                match text.parse::<f64>() {
                    Ok(v) if v.is_finite() => Tok::Num(v),
                    Ok(_) => return Err(syntax(start, "number out of range")),
                    Err(_) => return Err(syntax(start, format!("invalid number '{text}'"))),
                }
            }
            b'A'..=b'Z' | b'a'..=b'z' | b'_' => {
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                Tok::Ident(src[start..i].to_string())
            }
            b'+' | b'-' | b'*' | b'/' | b'^' | b'(' | b')' | b',' => {
                i += 1;
                match c {
                    b'+' => Tok::Plus,
                    b'-' => Tok::Minus,
                    b'*' => Tok::Star,
                    b'/' => Tok::Slash,
                    b'^' => Tok::Caret,
                    b'(' => Tok::LParen,
                    b')' => Tok::RParen,
                    _ => Tok::Comma,
                }
            }
            b'<' if two(b'=') => {
                i += 2;
                Tok::Le
            }
            b'<' => {
                i += 1;
                Tok::Lt
            }
            b'>' if two(b'=') => {
                i += 2;
                Tok::Ge
            }
            b'>' => {
                i += 1;
                Tok::Gt
            }
            b'=' if two(b'=') => {
                i += 2;
                Tok::EqEq
            }
            b'!' if two(b'=') => {
                i += 2;
                Tok::Ne
            }
            _ => {
                let ch = src[i..].chars().next().unwrap_or('?');
                return Err(syntax(start, format!("unexpected character '{ch}'")));
            }
        };
        out.push((tok, start));
    }
    Ok(out)
}

/// Returns the end index of the number starting at `i`.
fn lex_number(b: &[u8], mut i: usize) -> usize {
    let digits = |mut j: usize| {
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        j
    };
    i = digits(i);
    if b.get(i) == Some(&b'.') {
        i = digits(i + 1);
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(b.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        if b.get(j).is_some_and(u8::is_ascii_digit) {
            i = digits(j);
        }
    }
    i
}

pub fn syntax(pos: usize, msg: impl Into<String>) -> ParamError {
    ParamError::Syntax { pos, msg: msg.into() }
}
