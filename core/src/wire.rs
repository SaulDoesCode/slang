// Netstrings: `len:bytes,`. Every message between zones is a sequence of them.

pub fn put(out: &mut Vec<u8>, b: &[u8]) {
    out.extend_from_slice(b.len().to_string().as_bytes());
    out.push(b':');
    out.extend_from_slice(b);
    out.push(b',');
}

pub fn put_str(out: &mut Vec<u8>, s: &str) {
    put(out, s.as_bytes());
}

pub fn put_list(out: &mut Vec<u8>, items: &[String]) {
    let mut inner = Vec::new();
    for x in items {
        put_str(&mut inner, x);
    }
    put(out, &inner);
}

pub struct Reader<'a> {
    pub b: &'a [u8],
    pub i: usize,
}

impl<'a> Reader<'a> {
    pub fn new(b: &'a [u8]) -> Reader<'a> {
        Reader { b, i: 0 }
    }
    pub fn done(&self) -> bool {
        self.i >= self.b.len()
    }
    pub fn get(&mut self) -> Result<&'a [u8], String> {
        let start = self.i;
        while self.i < self.b.len() && self.b[self.i].is_ascii_digit() {
            self.i += 1;
        }
        if self.i == start || self.i >= self.b.len() || self.b[self.i] != b':' {
            return Err(format!("wire: bad length at {}", start));
        }
        let n: usize = std::str::from_utf8(&self.b[start..self.i]).unwrap().parse().map_err(|_| "wire: bad length".to_string())?;
        self.i += 1;
        if self.i + n >= self.b.len() + 1 || self.i + n > self.b.len() || self.b.get(self.i + n) != Some(&b',') {
            return Err(format!("wire: truncated netstring at {}", start));
        }
        let s = &self.b[self.i..self.i + n];
        self.i += n + 1;
        Ok(s)
    }
    pub fn str(&mut self) -> Result<String, String> {
        Ok(String::from_utf8_lossy(self.get()?).into_owned())
    }
    pub fn num(&mut self) -> Result<u64, String> {
        self.str()?.parse().map_err(|_| "wire: bad number".to_string())
    }
    pub fn list(&mut self) -> Result<Vec<String>, String> {
        let b = self.get()?;
        let mut r = Reader::new(b);
        let mut v = Vec::new();
        while !r.done() {
            v.push(r.str()?);
        }
        Ok(v)
    }
}

/// Reads one length-prefixed frame (8-byte big-endian length) from a stream.
pub fn read_frame(r: &mut dyn std::io::Read) -> std::io::Result<Vec<u8>> {
    let mut n = [0u8; 8];
    r.read_exact(&mut n)?;
    let n = u64::from_be_bytes(n) as usize;
    if n > 1 << 30 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "frame too large"));
    }
    let mut b = vec![0u8; n];
    r.read_exact(&mut b)?;
    Ok(b)
}

pub fn write_frame(w: &mut dyn std::io::Write, b: &[u8]) -> std::io::Result<()> {
    w.write_all(&(b.len() as u64).to_be_bytes())?;
    w.write_all(b)?;
    w.flush()
}

/// FNV-1a 64: the program hash a net zone checks.
pub fn fnv1a(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &x in b {
        h ^= x as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
