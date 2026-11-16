use super::*;

fn num(v: f64) -> Expr {
    Expr::Num(Value { v, dim: Dim::SCALAR })
}

fn bin(op: BinOp, a: Expr, b: Expr) -> Expr {
    Expr::Binary(op, Box::new(a), Box::new(b))
}

fn syn(src: &str) -> (usize, String) {
    match parse(src) {
        Err(ParamError::Syntax { pos, msg }) => (pos, msg),
        other => panic!("expected syntax error for {src:?}, got {other:?}"),
    }
}

#[test]
fn number_forms() {
    for (src, v) in [("12", 12.0), ("1.5", 1.5), (".5", 0.5), ("1e3", 1000.0), ("2.5e-2", 0.025), ("1E+2", 100.0)] {
        assert_eq!(parse(src).unwrap(), num(v), "{src}");
    }
}

#[test]
fn e_without_digits_is_not_exponent() {
    // `2e` is the number 2 followed by the constant e: a trailing token.
    assert_eq!(syn("2e"), (1, "unexpected token after expression".into()));
    assert_eq!(parse("2*e").unwrap(), bin(BinOp::Mul, num(2.0), num(std::f64::consts::E)));
    assert_eq!(syn("2e+").0, 1);
}

#[test]
fn units() {
    let mm = |v| Expr::Num(Value { v, dim: Dim::LENGTH });
    assert_eq!(parse("100mm").unwrap(), mm(100.0));
    assert_eq!(parse("100 mm").unwrap(), mm(100.0));
    assert_eq!(parse("2 cm").unwrap(), mm(20.0));
    assert_eq!(parse("1 m").unwrap(), mm(1000.0));
    assert_eq!(parse("1 in").unwrap(), mm(25.4));
    assert_eq!(parse("1 ft").unwrap(), mm(304.8));
    assert_eq!(parse("180 deg").unwrap(), Expr::Num(Value { v: std::f64::consts::PI, dim: Dim::ANGLE }));
    assert_eq!(parse("2 rad").unwrap(), Expr::Num(Value { v: 2.0, dim: Dim::ANGLE }));
}

#[test]
fn unit_without_number() {
    assert_eq!(syn("mm"), (0, "unit without a number".into()));
    assert_eq!(syn("1 + deg").0, 4);
    assert_eq!(syn("2 mm mm").0, 5);
}

#[test]
fn precedence_and_associativity() {
    assert_eq!(
        parse("1+2*3").unwrap(),
        bin(BinOp::Add, num(1.0), bin(BinOp::Mul, num(2.0), num(3.0)))
    );
    assert_eq!(
        parse("(1+2)*3").unwrap(),
        bin(BinOp::Mul, bin(BinOp::Add, num(1.0), num(2.0)), num(3.0))
    );
    assert_eq!(
        parse("2^3^2").unwrap(),
        bin(BinOp::Pow, num(2.0), bin(BinOp::Pow, num(3.0), num(2.0)))
    );
    assert_eq!(
        parse("-2^2").unwrap(),
        Expr::Neg(Box::new(bin(BinOp::Pow, num(2.0), num(2.0))))
    );
    assert_eq!(
        parse("1-2-3").unwrap(),
        bin(BinOp::Sub, bin(BinOp::Sub, num(1.0), num(2.0)), num(3.0))
    );
    assert_eq!(
        parse("8/2/2").unwrap(),
        bin(BinOp::Div, bin(BinOp::Div, num(8.0), num(2.0)), num(2.0))
    );
    assert_eq!(
        parse("2^-1").unwrap(),
        bin(BinOp::Pow, num(2.0), Expr::Neg(Box::new(num(1.0))))
    );
    assert_eq!(
        parse("-2*3").unwrap(),
        bin(BinOp::Mul, Expr::Neg(Box::new(num(2.0))), num(3.0))
    );
}

#[test]
fn comparisons() {
    let cases = [
        ("1<2", BinOp::Lt),
        ("1<=2", BinOp::Le),
        ("1>2", BinOp::Gt),
        ("1>=2", BinOp::Ge),
        ("1==2", BinOp::Eq),
        ("1!=2", BinOp::Ne),
    ];
    for (src, op) in cases {
        assert_eq!(parse(src).unwrap(), bin(op, num(1.0), num(2.0)), "{src}");
    }
    assert_eq!(
        parse("1+1 < 2*2").unwrap(),
        bin(
            BinOp::Lt,
            bin(BinOp::Add, num(1.0), num(1.0)),
            bin(BinOp::Mul, num(2.0), num(2.0))
        )
    );
    assert_eq!(syn("1 < 2 < 3"), (6, "comparisons cannot be chained".into()));
}

#[test]
fn calls_and_nesting() {
    assert_eq!(
        parse("sin(x)").unwrap(),
        Expr::Call(Func::Sin, vec![Expr::Var("x".into())])
    );
    assert_eq!(
        parse("max(1, min(2, 3))").unwrap(),
        Expr::Call(
            Func::Max,
            vec![num(1.0), Expr::Call(Func::Min, vec![num(2.0), num(3.0)])]
        )
    );
    assert_eq!(
        parse("if(a < 1, 2, 3)").unwrap(),
        Expr::Call(
            Func::If,
            vec![bin(BinOp::Lt, Expr::Var("a".into()), num(1.0)), num(2.0), num(3.0)]
        )
    );
    assert!(parse("atan2(1, 2)").is_ok());
}

#[test]
fn constants() {
    assert_eq!(parse("pi").unwrap(), num(std::f64::consts::PI));
    assert_eq!(parse("e").unwrap(), num(std::f64::consts::E));
}

#[test]
fn errors_with_positions() {
    assert_eq!(syn(""), (0, "unexpected end of input, expected an expression".into()));
    assert_eq!(syn("1 +"), (3, "unexpected end of input, expected an expression".into()));
    assert_eq!(syn("1 2"), (2, "unexpected token after expression".into()));
    assert_eq!(syn("1 )"), (2, "unexpected token after expression".into()));
    assert_eq!(syn("(1+2"), (4, "missing ')'".into()));
    assert_eq!(syn("sin(1"), (5, "missing ')'".into()));
    assert_eq!(syn("1 $ 2"), (2, "unexpected character '$'".into()));
    assert_eq!(syn("a = 1").0, 2);
    assert_eq!(syn("a ! b").0, 2);
    assert_eq!(syn("1 + * 2"), (4, "expected an expression".into()));
    assert_eq!(syn(".").0, 0);
    assert_eq!(syn("1e999").0, 0);
}

#[test]
fn unknown_function_and_bad_args() {
    assert_eq!(parse("foo(1)"), Err(ParamError::UnknownName("foo".into())));
    assert_eq!(parse("pi(1)"), Err(ParamError::UnknownName("pi".into())));
    assert_eq!(parse("sin()"), Err(ParamError::BadArgs { func: "sin", expected: 1, got: 0 }));
    assert_eq!(parse("sin(1, 2)"), Err(ParamError::BadArgs { func: "sin", expected: 1, got: 2 }));
    assert_eq!(parse("atan2(1)"), Err(ParamError::BadArgs { func: "atan2", expected: 2, got: 1 }));
    assert_eq!(parse("min(1, 2, 3)"), Err(ParamError::BadArgs { func: "min", expected: 2, got: 3 }));
    assert_eq!(parse("if(1, 2)"), Err(ParamError::BadArgs { func: "if", expected: 3, got: 2 }));
}

#[test]
fn variables_collected() {
    let vars = parse("a*b + sin(c) + a").unwrap().variables();
    assert_eq!(vars, ["a", "b", "c"].iter().map(|s| s.to_string()).collect());
    assert!(parse("pi + 2 mm").unwrap().variables().is_empty());
}

#[test]
fn depth_limit() {
    let deep = format!("{}1{}", "(".repeat(250), ")".repeat(250));
    assert_eq!(syn(&deep).1, "expression too deep");
    let ok = format!("{}1{}", "(".repeat(150), ")".repeat(150));
    assert!(parse(&ok).is_ok());
    assert_eq!(syn(&format!("{}1", "-".repeat(250))).1, "expression too deep");
}
