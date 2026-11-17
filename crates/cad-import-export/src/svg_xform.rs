//! Number scanning, `transform` lists and the unit scale of the root element for SVG import.
use cad_geometry::{DAffine2, DVec2};

/// A cursor over SVG number lists (transform arguments, points, path data).
pub(crate) struct Scan<'a> {
    b: &'a [u8],
    pub i: usize,
}

impl<'a> Scan<'a> {
    pub fn new(s: &'a str) -> Self {
        Self { b: s.as_bytes(), i: 0 }
    }

    pub fn skip_space(&mut self) {
        while self.b.get(self.i).is_some_and(u8::is_ascii_whitespace) {
            self.i += 1;
        }
    }

    /// The next byte that is not white space.
    pub fn peek(&mut self) -> Option<u8> {
        self.skip_space();
        self.b.get(self.i).copied()
    }

    pub fn eat(&mut self, c: u8) -> bool {
        let hit = self.peek() == Some(c);
        self.i += usize::from(hit);
        hit
    }

    fn skip_comma(&mut self) {
        self.skip_space();
        if self.b.get(self.i) == Some(&b',') {
            self.i += 1;
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.i;
        while self.b.get(self.i).is_some_and(u8::is_ascii_digit) {
            self.i += 1;
        }
        self.i - start
    }

    /// A number with an optional leading comma; None (and nothing consumed) when the next token is not a number.
    pub fn number(&mut self) -> Option<f64> {
        let saved = self.i;
        self.skip_comma();
        self.skip_space();
        let start = self.i;
        if matches!(self.b.get(self.i), Some(b'+' | b'-')) {
            self.i += 1;
        }
        let mut count = self.digits();
        if self.b.get(self.i) == Some(&b'.') {
            self.i += 1;
            count += self.digits();
        }
        if count == 0 {
            self.i = saved;
            return None;
        }
        if matches!(self.b.get(self.i), Some(b'e' | b'E')) {
            let mark = self.i;
            self.i += 1;
            if matches!(self.b.get(self.i), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if self.digits() == 0 {
                self.i = mark;
            }
        }
        let v = std::str::from_utf8(&self.b[start..self.i]).ok()?.parse::<f64>().ok().filter(|v| v.is_finite());
        if v.is_none() {
            self.i = saved;
        }
        v
    }

    /// An arc flag: one character, so "a1 1 0 00.5.5" reads as two flags and two numbers.
    pub fn flag(&mut self) -> Option<bool> {
        self.skip_comma();
        self.skip_space();
        let v = match self.b.get(self.i)? {
            b'0' => false,
            b'1' => true,
            _ => return None,
        };
        self.i += 1;
        Some(v)
    }

    pub fn at_end(&mut self) -> bool {
        self.peek().is_none()
    }

    /// Every remaining number; None when something else is in the way.
    pub fn all_numbers(&mut self) -> Option<Vec<f64>> {
        let mut out = Vec::new();
        while let Some(v) = self.number() {
            out.push(v);
        }
        self.skip_comma();
        self.at_end().then_some(out)
    }
}

/// The matrix of a `transform` attribute (translate, scale, rotate, matrix); None for anything else.
pub(crate) fn parse_transform(text: &str) -> Option<DAffine2> {
    let mut s = Scan::new(text);
    let mut m = DAffine2::IDENTITY;
    while s.peek().is_some() {
        s.eat(b',');
        let start = s.i;
        while s.b.get(s.i).is_some_and(u8::is_ascii_alphabetic) {
            s.i += 1;
        }
        let name = std::str::from_utf8(&s.b[start..s.i]).ok()?;
        if !s.eat(b'(') {
            return None;
        }
        let mut args = Vec::new();
        while let Some(v) = s.number() {
            args.push(v);
        }
        if !s.eat(b')') {
            return None;
        }
        let step = match (name, args.as_slice()) {
            ("translate", [x]) => DAffine2::from_translation(DVec2::new(*x, 0.0)),
            ("translate", [x, y]) => DAffine2::from_translation(DVec2::new(*x, *y)),
            ("scale", [k]) => DAffine2::from_scale(DVec2::splat(*k)),
            ("scale", [x, y]) => DAffine2::from_scale(DVec2::new(*x, *y)),
            ("rotate", [a]) => DAffine2::from_angle(a.to_radians()),
            ("rotate", [a, cx, cy]) => {
                let c = DVec2::new(*cx, *cy);
                DAffine2::from_translation(c) * DAffine2::from_angle(a.to_radians()) * DAffine2::from_translation(-c)
            }
            ("matrix", [a, b, c, d, e, f]) => DAffine2::from_cols_array(&[*a, *b, *c, *d, *e, *f]),
            _ => return None,
        };
        m *= step;
    }
    Some(m)
}

/// Millimetres in one unit of a length such as "50mm" or "3in"; a bare number is a pixel (96 per inch).
fn length_mm(text: &str) -> Option<f64> {
    let mut s = Scan::new(text);
    let v = s.number()?;
    let unit = text[s.i..].trim();
    let per_unit = match unit {
        "mm" => 1.0,
        "cm" => 10.0,
        "in" => 25.4,
        "pt" => 25.4 / 72.0,
        "" | "px" => 25.4 / 96.0,
        _ => return None,
    };
    Some(v * per_unit)
}

/// Millimetres per user unit: the width over the viewBox width when both are given, else 1.
pub(crate) fn user_unit_mm(width: Option<&str>, view_box: Option<&str>) -> f64 {
    let width_mm = width.and_then(length_mm);
    let box_width = view_box.and_then(|v| Scan::new(v).all_numbers()).and_then(|n| n.get(2).copied());
    match (width_mm, box_width) {
        (Some(w), Some(b)) if w > 0.0 && b > 0.0 => w / b,
        _ => 1.0,
    }
}
