//! Subset ekspresi CEL untuk flow-ir/1 (ADR 0002). Yang didukung:
//! path (`flow.input.x`, `m1.out.field`, `error.code`), literal (string, angka, `true`,
//! `false`, `null`), `==`, `!=`, `<`, `<=`, `>`, `>=`, `&&`, `||`, `!`, unary `-`, tanda
//! kurung, dan ternary `c ? a : b`.
//!
//! Prioritas dari yang paling lemah: ternary, `||`, `&&`, perbandingan, unary, primer. Ini
//! sesuai urutan yang dipakai CEL. `&&` dan `||` short-circuit. Ekspresi di luar subset ini
//! ditolak saat parsing, tidak dicoba menebak artinya.

use serde_json::{Map, Value};

/// Pohon ekspresi.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Lit(Value),
    Path(Vec<String>),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Bin(Op, Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Cond(Box<Expr>, Box<Expr>, Box<Expr>),
}

/// Operator perbandingan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Str(String),
    Num(f64),
    Op(&'static str),
    LParen,
    RParen,
    Question,
    Colon,
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let n: f64 = text
                .parse()
                .map_err(|_| format!("angka {text:?} tidak valid"))?;
            out.push(Tok::Num(n));
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '.')
            {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().collect()));
            continue;
        }
        if c == '\'' || c == '"' {
            let quote = c;
            i += 1;
            let mut text = String::new();
            loop {
                if i >= chars.len() {
                    return Err("string tidak ditutup".to_owned());
                }
                if chars[i] == quote {
                    i += 1;
                    break;
                }
                if chars[i] == '\\' && i + 1 < chars.len() {
                    text.push(chars[i + 1]);
                    i += 2;
                    continue;
                }
                text.push(chars[i]);
                i += 1;
            }
            out.push(Tok::Str(text));
            continue;
        }
        let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
        let (tok, len) = match two.as_str() {
            "==" => (Tok::Op("=="), 2),
            "!=" => (Tok::Op("!="), 2),
            "<=" => (Tok::Op("<="), 2),
            ">=" => (Tok::Op(">="), 2),
            "&&" => (Tok::Op("&&"), 2),
            "||" => (Tok::Op("||"), 2),
            _ => match c {
                '<' => (Tok::Op("<"), 1),
                '>' => (Tok::Op(">"), 1),
                '!' => (Tok::Op("!"), 1),
                '-' => (Tok::Op("-"), 1),
                '(' => (Tok::LParen, 1),
                ')' => (Tok::RParen, 1),
                '?' => (Tok::Question, 1),
                ':' => (Tok::Colon, 1),
                other => return Err(format!("karakter {other:?} di luar subset ekspresi")),
            },
        };
        out.push(tok);
        i += len;
    }
    Ok(out)
}

/// Mengurai satu ekspresi. Sisa token setelah ekspresi lengkap dianggap galat.
///
/// # Errors
///
/// Teks di luar subset, atau sintaks yang tidak lengkap.
pub fn parse(src: &str) -> Result<Expr, String> {
    let tokens = lex(src)?;
    let mut parser = Parser { tokens, pos: 0 };
    let expr = parser.ternary()?;
    if parser.pos != parser.tokens.len() {
        return Err(format!("sisa token setelah ekspresi di {src:?}"));
    }
    Ok(expr)
}

struct Parser {
    tokens: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn eat(&mut self, tok: &Tok) -> bool {
        if self.peek() == Some(tok) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn ternary(&mut self) -> Result<Expr, String> {
        let cond = self.or()?;
        if self.eat(&Tok::Question) {
            let then = self.ternary()?;
            if !self.eat(&Tok::Colon) {
                return Err("':' untuk ternary tidak ada".to_owned());
            }
            let other = self.ternary()?;
            return Ok(Expr::Cond(Box::new(cond), Box::new(then), Box::new(other)));
        }
        Ok(cond)
    }

    fn or(&mut self) -> Result<Expr, String> {
        let mut left = self.and()?;
        while self.eat(&Tok::Op("||")) {
            let right = self.and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Expr, String> {
        let mut left = self.compare()?;
        while self.eat(&Tok::Op("&&")) {
            let right = self.compare()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn compare(&mut self) -> Result<Expr, String> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Op("==")) => Op::Eq,
                Some(Tok::Op("!=")) => Op::Ne,
                Some(Tok::Op("<")) => Op::Lt,
                Some(Tok::Op("<=")) => Op::Le,
                Some(Tok::Op(">")) => Op::Gt,
                Some(Tok::Op(">=")) => Op::Ge,
                _ => break,
            };
            self.pos += 1;
            let right = self.unary()?;
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.eat(&Tok::Op("!")) {
            return Ok(Expr::Not(Box::new(self.unary()?)));
        }
        if self.eat(&Tok::Op("-")) {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, String> {
        let Some(tok) = self.peek().cloned() else {
            return Err("ekspresi berakhir tanpa nilai".to_owned());
        };
        self.pos += 1;
        match tok {
            Tok::Num(n) => Ok(Expr::Lit(
                serde_json::Number::from_f64(n).map_or(Value::Null, Value::Number),
            )),
            Tok::Str(s) => Ok(Expr::Lit(Value::String(s))),
            Tok::Ident(name) => Ok(match name.as_str() {
                "true" => Expr::Lit(Value::Bool(true)),
                "false" => Expr::Lit(Value::Bool(false)),
                "null" => Expr::Lit(Value::Null),
                _ => Expr::Path(name.split('.').map(str::to_owned).collect()),
            }),
            Tok::LParen => {
                let inner = self.ternary()?;
                if !self.eat(&Tok::RParen) {
                    return Err("')' tidak ditutup".to_owned());
                }
                Ok(inner)
            }
            other => Err(format!("token {other:?} tidak bisa berada di sini")),
        }
    }
}

/// Lingkungan evaluasi: nama akar (`flow`, id modul, `error`, `item`) ke nilai.
pub type Env = Map<String, Value>;

/// Mengevaluasi ekspresi di `env`.
///
/// # Errors
///
/// Nama atau kunci tidak ada, atau tipe operand tidak cocok.
pub fn eval(expr: &Expr, env: &Env) -> Result<Value, String> {
    match expr {
        Expr::Lit(v) => Ok(v.clone()),
        Expr::Path(segments) => lookup(segments, env),
        Expr::Not(inner) => Ok(Value::Bool(!as_bool(&eval(inner, env)?)?)),
        Expr::Neg(inner) => {
            let n = as_number(&eval(inner, env)?)?;
            Ok(number(-n))
        }
        Expr::And(l, r) => {
            if !as_bool(&eval(l, env)?)? {
                return Ok(Value::Bool(false));
            }
            Ok(Value::Bool(as_bool(&eval(r, env)?)?))
        }
        Expr::Or(l, r) => {
            if as_bool(&eval(l, env)?)? {
                return Ok(Value::Bool(true));
            }
            Ok(Value::Bool(as_bool(&eval(r, env)?)?))
        }
        Expr::Cond(c, t, f) => {
            if as_bool(&eval(c, env)?)? {
                eval(t, env)
            } else {
                eval(f, env)
            }
        }
        Expr::Bin(op, l, r) => {
            let a = eval(l, env)?;
            let b = eval(r, env)?;
            compare(*op, &a, &b).map(Value::Bool)
        }
    }
}

/// Mengevaluasi ekspresi yang harus bernilai bool.
///
/// # Errors
///
/// Galat evaluasi, atau hasil bukan bool.
pub fn eval_bool(expr: &Expr, env: &Env) -> Result<bool, String> {
    as_bool(&eval(expr, env)?)
}

fn lookup(segments: &[String], env: &Env) -> Result<Value, String> {
    let root = segments.first().ok_or_else(|| "path kosong".to_owned())?;
    let mut value = env
        .get(root)
        .cloned()
        .ok_or_else(|| format!("nama {root:?} tidak dikenal"))?;
    for key in &segments[1..] {
        value = value
            .get(key)
            .cloned()
            .ok_or_else(|| format!("kunci {key:?} tidak ada di {root}"))?;
    }
    Ok(value)
}

fn as_bool(v: &Value) -> Result<bool, String> {
    v.as_bool().ok_or_else(|| format!("nilai {v} bukan bool"))
}

fn as_number(v: &Value) -> Result<f64, String> {
    v.as_f64().ok_or_else(|| format!("nilai {v} bukan angka"))
}

fn number(n: f64) -> Value {
    serde_json::Number::from_f64(n).map_or(Value::Null, Value::Number)
}

fn compare(op: Op, a: &Value, b: &Value) -> Result<bool, String> {
    match op {
        Op::Eq => Ok(equal(a, b)),
        Op::Ne => Ok(!equal(a, b)),
        Op::Lt | Op::Le | Op::Gt | Op::Ge => {
            let ordering = match (a, b) {
                (Value::String(x), Value::String(y)) => x.cmp(y),
                _ => as_number(a)?
                    .partial_cmp(&as_number(b)?)
                    .ok_or_else(|| "perbandingan dengan NaN".to_owned())?,
            };
            Ok(match op {
                Op::Lt => ordering.is_lt(),
                Op::Le => ordering.is_le(),
                Op::Gt => ordering.is_gt(),
                _ => ordering.is_ge(),
            })
        }
    }
}

/// Kesetaraan nilai JSON. Angka dibandingkan sebagai `f64`, jadi `1` sama dengan `1.0`.
fn equal(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}
