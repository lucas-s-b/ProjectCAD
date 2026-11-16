use std::collections::BTreeSet;

use crate::lexer::{Tok, lex, syntax};
use crate::value::{Dim, ParamError, Value, unit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Sqrt,
    Abs,
    Min,
    Max,
    Floor,
    Ceil,
    Round,
    If,
}

impl Func {
    pub fn from_name(name: &str) -> Option<Func> {
        Some(match name {
            "sin" => Func::Sin,
            "cos" => Func::Cos,
            "tan" => Func::Tan,
            "asin" => Func::Asin,
            "acos" => Func::Acos,
            "atan" => Func::Atan,
            "atan2" => Func::Atan2,
            "sqrt" => Func::Sqrt,
            "abs" => Func::Abs,
            "min" => Func::Min,
            "max" => Func::Max,
            "floor" => Func::Floor,
            "ceil" => Func::Ceil,
            "round" => Func::Round,
            "if" => Func::If,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Func::Sin => "sin",
            Func::Cos => "cos",
            Func::Tan => "tan",
            Func::Asin => "asin",
            Func::Acos => "acos",
            Func::Atan => "atan",
            Func::Atan2 => "atan2",
            Func::Sqrt => "sqrt",
            Func::Abs => "abs",
            Func::Min => "min",
            Func::Max => "max",
            Func::Floor => "floor",
            Func::Ceil => "ceil",
            Func::Round => "round",
            Func::If => "if",
        }
    }

    pub fn arity(self) -> usize {
        match self {
            Func::Atan2 | Func::Min | Func::Max => 2,
            Func::If => 3,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(Value),
    Var(String),
    Neg(Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Call(Func, Vec<Expr>),
}

impl Expr {
    pub fn variables(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        self.collect(&mut out);
        out
    }

    fn collect(&self, out: &mut BTreeSet<String>) {
        match self {
            Expr::Num(_) => {}
            Expr::Var(n) => {
                out.insert(n.clone());
            }
            Expr::Neg(a) => a.collect(out),
            Expr::Binary(_, a, b) => {
                a.collect(out);
                b.collect(out);
            }
            Expr::Call(_, args) => args.iter().for_each(|a| a.collect(out)),
        }
    }
}

const MAX_DEPTH: usize = 200;

pub fn parse(src: &str) -> Result<Expr, ParamError> {
    let mut p = Parser { toks: lex(src)?, i: 0, end: src.len(), depth: 0 };
    let e = p.expr()?;
    match p.toks.get(p.i) {
        None => Ok(e),
        Some((_, pos)) => Err(syntax(*pos, "unexpected token after expression")),
    }
}

struct Parser {
    toks: Vec<(Tok, usize)>,
    i: usize,
    end: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i).map(|(t, _)| t)
    }

    fn pos(&self) -> usize {
        self.toks.get(self.i).map_or(self.end, |(_, p)| *p)
    }

    fn enter(&mut self) -> Result<(), ParamError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(syntax(self.pos(), "expression too deep"));
        }
        Ok(())
    }

    fn expr(&mut self) -> Result<Expr, ParamError> {
        self.enter()?;
        let lhs = self.sum()?;
        let op = match self.peek() {
            Some(Tok::Lt) => BinOp::Lt,
            Some(Tok::Le) => BinOp::Le,
            Some(Tok::Gt) => BinOp::Gt,
            Some(Tok::Ge) => BinOp::Ge,
            Some(Tok::EqEq) => BinOp::Eq,
            Some(Tok::Ne) => BinOp::Ne,
            _ => {
                self.depth -= 1;
                return Ok(lhs);
            }
        };
        self.i += 1;
        let rhs = self.sum()?;
        if matches!(self.peek(), Some(Tok::Lt | Tok::Le | Tok::Gt | Tok::Ge | Tok::EqEq | Tok::Ne)) {
            return Err(syntax(self.pos(), "comparisons cannot be chained"));
        }
        self.depth -= 1;
        Ok(Expr::Binary(op, Box::new(lhs), Box::new(rhs)))
    }

    fn sum(&mut self) -> Result<Expr, ParamError> {
        let mut lhs = self.term()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Plus) => BinOp::Add,
                Some(Tok::Minus) => BinOp::Sub,
                _ => return Ok(lhs),
            };
            self.i += 1;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(self.term()?));
        }
    }

    fn term(&mut self) -> Result<Expr, ParamError> {
        let mut lhs = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Star) => BinOp::Mul,
                Some(Tok::Slash) => BinOp::Div,
                _ => return Ok(lhs),
            };
            self.i += 1;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(self.unary()?));
        }
    }

    fn unary(&mut self) -> Result<Expr, ParamError> {
        if self.peek() == Some(&Tok::Minus) {
            self.enter()?;
            self.i += 1;
            let e = self.unary()?;
            self.depth -= 1;
            return Ok(Expr::Neg(Box::new(e)));
        }
        let base = self.primary()?;
        if self.peek() == Some(&Tok::Caret) {
            self.i += 1;
            let exp = self.unary()?; // right-associative, allows 2^-3
            return Ok(Expr::Binary(BinOp::Pow, Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn primary(&mut self) -> Result<Expr, ParamError> {
        let pos = self.pos();
        let Some(tok) = self.peek().cloned() else {
            return Err(syntax(pos, "unexpected end of input, expected an expression"));
        };
        self.i += 1;
        match tok {
            Tok::Num(v) => {
                if let Some(Tok::Ident(name)) = self.peek()
                    && let Some(u) = unit(name)
                {
                    self.i += 1;
                    return Ok(Expr::Num(Value { v: v * u.v, dim: u.dim }));
                }
                Ok(Expr::Num(Value { v, dim: Dim::SCALAR }))
            }
            Tok::Ident(name) => self.name(name, pos),
            Tok::LParen => {
                let e = self.expr()?;
                if self.peek() != Some(&Tok::RParen) {
                    return Err(syntax(self.pos(), "missing ')'"));
                }
                self.i += 1;
                Ok(e)
            }
            _ => Err(syntax(pos, "expected an expression")),
        }
    }

    fn name(&mut self, name: String, pos: usize) -> Result<Expr, ParamError> {
        if self.peek() == Some(&Tok::LParen) {
            let f = Func::from_name(&name).ok_or_else(|| ParamError::UnknownName(name.clone()))?;
            self.i += 1;
            let mut args = Vec::new();
            if self.peek() != Some(&Tok::RParen) {
                loop {
                    args.push(self.expr()?);
                    if self.peek() == Some(&Tok::Comma) {
                        self.i += 1;
                    } else {
                        break;
                    }
                }
            }
            if self.peek() != Some(&Tok::RParen) {
                return Err(syntax(self.pos(), "missing ')'"));
            }
            self.i += 1;
            if args.len() != f.arity() {
                return Err(ParamError::BadArgs { func: f.name(), expected: f.arity(), got: args.len() });
            }
            return Ok(Expr::Call(f, args));
        }
        let scalar = |v: f64| Expr::Num(Value { v, dim: Dim::SCALAR });
        match name.as_str() {
            "pi" => Ok(scalar(std::f64::consts::PI)),
            "e" => Ok(scalar(std::f64::consts::E)),
            _ if unit(&name).is_some() => Err(syntax(pos, "unit without a number")),
            _ => Ok(Expr::Var(name)),
        }
    }
}

#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;
