// JSON values, a parser and a writer: the world format and the server's messages.

#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

pub static NULL: J = J::Null;

impl J {
    pub fn obj() -> J {
        J::Obj(Vec::new())
    }
    pub fn get(&self, k: &str) -> &J {
        match self {
            J::Obj(v) => v.iter().find(|(a, _)| a == k).map(|(_, b)| b).unwrap_or(&NULL),
            _ => &NULL,
        }
    }
    pub fn has(&self, k: &str) -> bool {
        matches!(self, J::Obj(v) if v.iter().any(|(a, _)| a == k))
    }
    pub fn set(&mut self, k: &str, v: J) {
        if let J::Obj(o) = self {
            if let Some(e) = o.iter_mut().find(|(a, _)| a == k) {
                e.1 = v;
            } else {
                o.push((k.to_string(), v));
            }
        }
    }
    pub fn with(mut self, k: &str, v: J) -> J {
        self.set(k, v);
        self
    }
    pub fn push(&mut self, v: J) {
        if let J::Arr(a) = self {
            a.push(v);
        }
    }
    pub fn is_null(&self) -> bool {
        matches!(self, J::Null)
    }
    pub fn num(&self) -> Option<f64> {
        match self {
            J::Num(x) => Some(*x),
            J::Str(s) => s.parse().ok(),
            _ => None,
        }
    }
    pub fn f(&self, d: f64) -> f64 {
        self.num().unwrap_or(d)
    }
    pub fn i(&self, d: i64) -> i64 {
        self.num().map(|x| x as i64).unwrap_or(d)
    }
    pub fn u(&self, d: u32) -> u32 {
        self.num().map(|x| x as u32).unwrap_or(d)
    }
    pub fn b(&self, d: bool) -> bool {
        match self {
            J::Bool(b) => *b,
            _ => d,
        }
    }
    pub fn str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn s(&self, d: &str) -> String {
        self.str().unwrap_or(d).to_string()
    }
    pub fn arr(&self) -> &[J] {
        match self {
            J::Arr(a) => a,
            _ => &[],
        }
    }
    pub fn entries(&self) -> &[(String, J)] {
        match self {
            J::Obj(o) => o,
            _ => &[],
        }
    }
    pub fn v3(&self, d: [f64; 3]) -> [f64; 3] {
        let a = self.arr();
        if a.len() == 3 {
            [a[0].f(d[0]), a[1].f(d[1]), a[2].f(d[2])]
        } else {
            d
        }
    }
    pub fn i3(&self, d: [i32; 3]) -> [i32; 3] {
        let a = self.arr();
        if a.len() == 3 {
            [a[0].i(d[0] as i64) as i32, a[1].i(d[1] as i64) as i32, a[2].i(d[2] as i64) as i32]
        } else {
            d
        }
    }

    pub fn parse(s: &str) -> Result<J, String> {
        let mut p = P { b: s.as_bytes(), i: 0 };
        p.ws();
        let v = p.val(0)?;
        p.ws();
        if p.i != p.b.len() {
            return Err(format!("json: trailing text at {}", p.i));
        }
        Ok(v)
    }

    pub fn write(&self, o: &mut String) {
        match self {
            J::Null => o.push_str("null"),
            J::Bool(b) => o.push_str(if *b { "true" } else { "false" }),
            J::Num(x) => {
                if !x.is_finite() {
                    o.push_str("null")
                } else if x.fract() == 0.0 && x.abs() < 1e15 {
                    o.push_str(&format!("{}", *x as i64))
                } else {
                    o.push_str(&format!("{}", x))
                }
            }
            J::Str(s) => quote(s, o),
            J::Arr(a) => {
                o.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        o.push(',');
                    }
                    v.write(o);
                }
                o.push(']');
            }
            J::Obj(m) => {
                o.push('{');
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        o.push(',');
                    }
                    quote(k, o);
                    o.push(':');
                    v.write(o);
                }
                o.push('}');
            }
        }
    }

    pub fn dump(&self) -> String {
        let mut o = String::new();
        self.write(&mut o);
        o
    }

    /// One record per line inside top-level arrays: diffs well and stays readable.
    pub fn pretty(&self) -> String {
        match self {
            J::Obj(m) => {
                let mut o = String::from("{\n");
                for (i, (k, v)) in m.iter().enumerate() {
                    o.push_str("  ");
                    quote(k, &mut o);
                    o.push_str(": ");
                    match v {
                        J::Arr(a) if !a.is_empty() && a.iter().all(|x| matches!(x, J::Obj(_))) => {
                            o.push_str("[\n");
                            for (j, x) in a.iter().enumerate() {
                                o.push_str("    ");
                                x.write(&mut o);
                                if j + 1 < a.len() {
                                    o.push(',');
                                }
                                o.push('\n');
                            }
                            o.push_str("  ]");
                        }
                        _ => v.write(&mut o),
                    }
                    if i + 1 < m.len() {
                        o.push(',');
                    }
                    o.push('\n');
                }
                o.push('}');
                o.push('\n');
                o
            }
            _ => self.dump(),
        }
    }
}

impl From<&str> for J {
    fn from(s: &str) -> J {
        J::Str(s.to_string())
    }
}
impl From<String> for J {
    fn from(s: String) -> J {
        J::Str(s)
    }
}
impl From<f64> for J {
    fn from(x: f64) -> J {
        J::Num(x)
    }
}
impl From<u32> for J {
    fn from(x: u32) -> J {
        J::Num(x as f64)
    }
}
impl From<i32> for J {
    fn from(x: i32) -> J {
        J::Num(x as f64)
    }
}
impl From<usize> for J {
    fn from(x: usize) -> J {
        J::Num(x as f64)
    }
}
impl From<bool> for J {
    fn from(b: bool) -> J {
        J::Bool(b)
    }
}
impl From<[f64; 3]> for J {
    fn from(v: [f64; 3]) -> J {
        J::Arr(v.iter().map(|x| J::Num(*x)).collect())
    }
}
impl From<[i32; 3]> for J {
    fn from(v: [i32; 3]) -> J {
        J::Arr(v.iter().map(|x| J::Num(*x as f64)).collect())
    }
}
impl<T: Into<J>> From<Vec<T>> for J {
    fn from(v: Vec<T>) -> J {
        J::Arr(v.into_iter().map(|x| x.into()).collect())
    }
}

pub fn quote(s: &str, o: &mut String) {
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\n' | b'\r' | b'\t') {
            self.i += 1;
        }
    }
    fn err<T>(&self, m: &str) -> Result<T, String> {
        Err(format!("json: {} at {}", m, self.i))
    }
    fn val(&mut self, depth: u32) -> Result<J, String> {
        if depth > 200 {
            return self.err("too deep");
        }
        self.ws();
        let c = match self.b.get(self.i) {
            Some(c) => *c,
            None => return self.err("unexpected end"),
        };
        match c {
            b'{' => {
                self.i += 1;
                let mut m = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(J::Obj(m));
                }
                loop {
                    self.ws();
                    let k = match self.val(depth + 1)? {
                        J::Str(s) => s,
                        _ => return self.err("key must be a string"),
                    };
                    self.ws();
                    if self.b.get(self.i) != Some(&b':') {
                        return self.err("expected :");
                    }
                    self.i += 1;
                    let v = self.val(depth + 1)?;
                    m.push((k, v));
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(J::Obj(m));
                        }
                        _ => return self.err("expected , or }"),
                    }
                }
            }
            b'[' => {
                self.i += 1;
                let mut a = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(J::Arr(a));
                }
                loop {
                    a.push(self.val(depth + 1)?);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(J::Arr(a));
                        }
                        _ => return self.err("expected , or ]"),
                    }
                }
            }
            b'"' => {
                self.i += 1;
                let mut s: Vec<u8> = Vec::new();
                loop {
                    let c = match self.b.get(self.i) {
                        Some(c) => *c,
                        None => return self.err("unclosed string"),
                    };
                    self.i += 1;
                    match c {
                        b'"' => break,
                        b'\\' => {
                            let e = match self.b.get(self.i) {
                                Some(e) => *e,
                                None => return self.err("bad escape"),
                            };
                            self.i += 1;
                            match e {
                                b'"' => s.push(b'"'),
                                b'\\' => s.push(b'\\'),
                                b'/' => s.push(b'/'),
                                b'b' => s.push(8),
                                b'f' => s.push(12),
                                b'n' => s.push(b'\n'),
                                b'r' => s.push(b'\r'),
                                b't' => s.push(b'\t'),
                                b'u' => {
                                    let mut cp = self.hex4()?;
                                    if (0xD800..0xDC00).contains(&cp) && self.b.get(self.i) == Some(&b'\\') && self.b.get(self.i + 1) == Some(&b'u') {
                                        self.i += 2;
                                        let lo = self.hex4()?;
                                        cp = 0x10000 + ((cp - 0xD800) << 10) + (lo.wrapping_sub(0xDC00) & 0x3FF);
                                    }
                                    let ch = char::from_u32(cp).unwrap_or('\u{FFFD}');
                                    let mut buf = [0u8; 4];
                                    s.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                                }
                                _ => return self.err("bad escape"),
                            }
                        }
                        c => s.push(c),
                    }
                }
                Ok(J::Str(String::from_utf8_lossy(&s).into_owned()))
            }
            b't' if self.b[self.i..].starts_with(b"true") => {
                self.i += 4;
                Ok(J::Bool(true))
            }
            b'f' if self.b[self.i..].starts_with(b"false") => {
                self.i += 5;
                Ok(J::Bool(false))
            }
            b'n' if self.b[self.i..].starts_with(b"null") => {
                self.i += 4;
                Ok(J::Null)
            }
            _ => {
                let st = self.i;
                while self.i < self.b.len() && matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    self.i += 1;
                }
                let t = std::str::from_utf8(&self.b[st..self.i]).unwrap_or("");
                match t.parse::<f64>() {
                    Ok(x) if !t.is_empty() => Ok(J::Num(x)),
                    _ => {
                        self.i = st;
                        self.err("unexpected character")
                    }
                }
            }
        }
    }
    fn hex4(&mut self) -> Result<u32, String> {
        if self.i + 4 > self.b.len() {
            return self.err("bad \\u");
        }
        let t = std::str::from_utf8(&self.b[self.i..self.i + 4]).unwrap_or("");
        self.i += 4;
        u32::from_str_radix(t, 16).or_else(|_| self.err("bad \\u"))
    }
}
