#![allow(dead_code, unused_mut, unused_variables, unused_parens, non_snake_case, unreachable_code, unused_assignments, clippy::all)]
use std::io::{BufRead, Read, Write};
use std::sync::Arc;

type S = Arc<str>;
type L = Arc<Vec<V>>;
type LN = Arc<Vec<f64>>;

#[derive(Clone, Debug)]
enum V {
    N(f64),
    T(S),
    X,
}

fn rs(s: &str) -> S {
    Arc::from(s)
}
fn fmt(x: f64) -> S {
    if x == 0.0 { rs("0") } else { Arc::from(x.to_string().as_str()) }
}
fn parse(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(0.0)
}
fn vof(s: S) -> V {
    match s.parse::<f64>() {
        Ok(x) if *fmt(x) == *s => V::N(x),
        _ => V::T(s),
    }
}
fn vtext(v: &V) -> S {
    match v {
        V::N(x) => fmt(*x),
        V::T(s) => s.clone(),
        V::X => rs("\u{1}"),
    }
}
fn vnum(v: &V) -> f64 {
    match v {
        V::N(x) => *x,
        V::T(s) => parse(s),
        V::X => 0.0,
    }
}
fn veq(a: &V, b: &V) -> bool {
    match (a, b) {
        (V::N(x), V::N(y)) if !x.is_nan() && !y.is_nan() => x == y,
        _ => *vtext(a) == *vtext(b),
    }
}
fn cat(parts: &[&str]) -> S {
    let mut s = String::new();
    for p in parts {
        s.push_str(p);
    }
    Arc::from(s.as_str())
}
fn seq(a: &str, b: &str) -> bool {
    a == b
}
fn isx(v: &V) -> bool {
    matches!(v, V::X)
}
fn llen<T>(l: &Arc<Vec<T>>) -> f64 {
    l.len() as f64
}
fn slen(s: &str) -> f64 {
    s.chars().count() as f64
}
fn lnew<T>() -> Arc<Vec<T>> {
    Arc::new(Vec::new())
}
fn lpush<T: Clone>(mut l: Arc<Vec<T>>, v: T) -> Arc<Vec<T>> {
    Arc::make_mut(&mut l).push(v);
    l
}
fn lpop<T: Clone>(mut l: Arc<Vec<T>>) -> Arc<Vec<T>> {
    Arc::make_mut(&mut l).pop();
    l
}
fn llast(l: &L) -> V {
    l.last().cloned().unwrap_or_else(|| V::T(rs("404")))
}
fn at<T>(l: &Arc<Vec<T>>, i: f64) -> Option<usize> {
    let i = i as i64;
    let i = if i < 0 { i + l.len() as i64 } else { i };
    if i >= 0 && (i as usize) < l.len() { Some(i as usize) } else { None }
}
fn lhas<T>(l: &Arc<Vec<T>>, i: f64) -> bool {
    at(l, i).is_some()
}
fn lget(l: &L, i: f64) -> V {
    match at(l, i) {
        Some(k) => l[k].clone(),
        None => V::T(rs("404")),
    }
}
fn lset<T: Clone>(mut l: Arc<Vec<T>>, i: f64, v: T) -> Arc<Vec<T>> {
    if let Some(k) = at(&l, i) {
        Arc::make_mut(&mut l)[k] = v;
    }
    l
}
fn lcat(mut a: L, b: &L) -> L {
    Arc::make_mut(&mut a).extend(b.iter().cloned());
    a
}
fn clampk(k: f64, n: usize) -> usize {
    if !(k > 0.0) { 0 } else if k >= n as f64 { n } else { k as usize }
}
fn ltake(mut l: L, k: f64) -> L {
    let k = clampk(k, l.len());
    Arc::make_mut(&mut l).truncate(k);
    l
}
fn ldrop(mut l: L, k: f64) -> L {
    let k = clampk(k, l.len());
    Arc::make_mut(&mut l).drain(..k);
    l
}
/// S_FORKS set: each fork over numbers reports how it ran on stderr.
fn forks_on() -> bool {
    static F: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *F.get_or_init(|| std::env::var_os("S_FORKS").is_some())
}
/// The vectors a fork's kernel runs on, on this machine.
fn simd_level() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("avx512f") {
            return "f64×8 avx-512";
        }
        if std::is_x86_feature_detected!("avx2") {
            return "f64×4 avx2";
        }
        return "f64×2 sse2";
    }
    #[allow(unreachable_code)]
    "scalar"
}
/// Cores not yet running a lane: a call pair takes one when there is one, else runs in order.
fn spare() -> &'static std::sync::atomic::AtomicIsize {
    static S: std::sync::OnceLock<std::sync::atomic::AtomicIsize> = std::sync::OnceLock::new();
    S.get_or_init(|| std::sync::atomic::AtomicIsize::new(std::thread::available_parallelism().map_or(1, |n| n.get()) as isize - 1))
}
fn par_go<A: Send, B: Send>(a: impl FnOnce() -> A + Send, b: impl FnOnce() -> B + Send) -> (A, B) {
    use std::sync::atomic::Ordering::SeqCst;
    if spare().fetch_sub(1, SeqCst) > 0 {
        let r = std::thread::scope(|s| {
            let h = std::thread::Builder::new().stack_size(1 << 28).spawn_scoped(s, a).unwrap();
            let rb = b();
            (h.join().unwrap(), rb)
        });
        spare().fetch_add(1, SeqCst);
        r
    } else {
        spare().fetch_add(1, SeqCst);
        (a(), b())
    }
}
/// The remainder as the machine takes it: never negative for b > 0.
#[inline(always)]
fn srem(a: f64, b: f64) -> f64 {
    a.rem_euclid(b)
}
/// The same, exact for whole numbers below 2^53 and written so it runs in vectors; bad is set
/// for any other input, and the caller then takes the column again with srem.
#[inline(always)]
fn sremv(bad: &mut bool, a: f64, b: f64) -> f64 {
    const BIG: f64 = 9007199254740992.0;
    let ok = a.trunc() == a && b.trunc() == b && a.abs() < BIG && b.abs() < BIG && b != 0.0;
    *bad |= !ok;
    let q = (a / b).trunc();
    let r = a - q * b;
    let r = if r == 0.0 { 0.0f64.copysign(a) } else { r };
    if r < 0.0 { r + b.abs() } else { r }
}
fn lget_n(l: &LN, i: f64) -> V {
    match at(l, i) {
        Some(k) => V::N(l[k]),
        None => V::T(rs("404")),
    }
}
fn llast_n(l: &LN) -> V {
    l.last().map_or_else(|| V::T(rs("404")), |x| V::N(*x))
}
fn join_n(l: &LN, sep: &str) -> S {
    let ts: Vec<S> = l.iter().map(|x| fmt(*x)).collect();
    let ts: Vec<&str> = ts.iter().map(|t| &**t).collect();
    Arc::from(ts.join(sep).as_str())
}
fn to_l(l: &LN) -> L {
    Arc::new(l.iter().map(|x| V::N(*x)).collect())
}
fn splitws(s: &str) -> L {
    Arc::new(s.split_whitespace().map(|p| vof(rs(p))).collect())
}
fn splitchars(s: &str) -> L {
    Arc::new(s.chars().map(|c| vof(rs(&c.to_string()))).collect())
}
fn splitby(s: &str, sep: &str) -> L {
    if sep.is_empty() {
        return splitchars(s);
    }
    Arc::new(s.split(sep).map(|p| vof(rs(p))).collect())
}
fn join(l: &L, sep: &str) -> S {
    let ts: Vec<S> = l.iter().map(vtext).collect();
    let ts: Vec<&str> = ts.iter().map(|t| &**t).collect();
    Arc::from(ts.join(sep).as_str())
}
fn oneword(t: &str) -> bool {
    !t.is_empty() && !t.chars().any(|c| matches!(c, ' ' | '\n' | '\t' | '\r'))
}
fn active(t: &str) -> bool {
    if t.starts_with('⟨') {
        return true;
    }
    let long = t.chars().nth(1).is_some();
    long && (t.starts_with('\\') || t.chars().last().map_or(false, |c| "?!~$`#;:.@".contains(c)))
}
fn fx_ok(s: &str) -> bool {
    s.starts_with('1')
}
fn fx_val(s: &str) -> S {
    rs(if s.is_empty() { "" } else { &s[1..] })
}
fn sh_rc(s: &str) -> f64 {
    parse(s.split('\u{1}').next().unwrap_or(""))
}
fn sh_out(s: &str) -> S {
    rs(s.splitn(2, '\u{1}').nth(1).unwrap_or(""))
}
fn args() -> L {
    Arc::new(std::env::args().skip(1).map(|a| vof(rs(&a))).collect())
}

struct Rng([u64; 4]);
fn splitmix(z: &mut u64) -> u64 {
    *z = z.wrapping_add(0x9E3779B97F4A7C15);
    let mut x = *z;
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
    x ^ (x >> 31)
}
impl Rng {
    fn seeded(a: u64, b: u64) -> Rng {
        let mut z = a ^ b.rotate_left(32);
        let mut s = [0u64; 4];
        for x in s.iter_mut() {
            *x = splitmix(&mut z);
        }
        Rng(s)
    }
    fn next(&mut self) -> u64 {
        let s = &mut self.0;
        let r = s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        r
    }
}
fn os_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(std::process::id() as u64);
    h.finish()
}

enum H {
    File(std::fs::File),
    Net(std::io::BufReader<std::net::TcpStream>),
    Listen(std::net::TcpListener),
}
struct FileH {
    f: H,
    eof: bool,
}
impl Read for H {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        match self {
            H::File(f) => f.read(b),
            H::Net(n) => n.read(b),
            H::Listen(_) => Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "a listener is not read")),
        }
    }
}
impl Write for H {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        match self {
            H::File(f) => f.write(b),
            H::Net(n) => n.get_mut().write(b),
            H::Listen(_) => Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "a listener is not written")),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            H::File(f) => f.flush(),
            H::Net(n) => n.get_mut().flush(),
            H::Listen(_) => Ok(()),
        }
    }
}
impl std::io::Seek for H {
    fn seek(&mut self, p: std::io::SeekFrom) -> std::io::Result<u64> {
        match self {
            H::File(f) => f.seek(p),
            _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "a connection has no place to seek to")),
        }
    }
}

struct Io {
    out: std::io::BufWriter<std::io::Stdout>,
    rng: Rng,
    files: std::collections::HashMap<String, FileH>,
}

impl Io {
    fn new() -> Io {
        Io {
            out: std::io::BufWriter::with_capacity(1 << 16, std::io::stdout()),
            rng: Rng::seeded(os_u64(), os_u64().rotate_left(17)),
            files: std::collections::HashMap::new(),
        }
    }
    fn flush(&mut self) {
        let _ = self.out.flush();
    }
    fn w(&mut self, s: &str) {
        let _ = self.out.write_all(s.as_bytes());
    }
    fn read_line(&mut self) -> (V, f64) {
        let _ = self.out.flush();
        let mut line = String::new();
        let n = std::io::stdin().lock().read_line(&mut line).unwrap_or(0);
        while line.ends_with('\n') || line.ends_with('\r') {
            line.pop();
        }
        (vof(Arc::from(line.as_str())), if n == 0 { 1.0 } else { 0.0 })
    }
    fn exit(&mut self, code: f64) -> ! {
        let _ = self.out.flush();
        std::process::exit(code as i32)
    }
    fn fail(&mut self, msg: &str) -> ! {
        let _ = self.out.flush();
        eprintln!("\ns: {}", msg);
        std::process::exit(1)
    }
    fn seed(&mut self, b: &str) {
        let p: Vec<f64> = b.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        if let Some(a) = p.first() {
            self.rng = Rng::seeded(*a as i64 as u64, p.get(1).map_or(0, |y| *y as i64 as u64));
        }
    }
    fn fx(&mut self, op: &str, a: &str, b: &str) -> S {
        let ok = |s: &str| -> S { Arc::from(format!("1{}", s).as_str()) };
        let no = |e: std::io::Error| -> S { Arc::from(format!("0{}", e).as_str()) };
        let _ = self.out.flush();
        match op {
            "r" => match std::fs::read(a) {
                Ok(b) => ok(&String::from_utf8_lossy(&b)),
                Err(e) => no(e),
            },
            "ra" => {
                let p: Vec<f64> = b.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                let (off, len) = (p.first().copied().unwrap_or(0.0) as u64, p.get(1).copied().unwrap_or(0.0) as u64);
                let r = std::fs::File::open(a).and_then(|mut f| {
                    use std::io::Seek;
                    f.seek(std::io::SeekFrom::Start(off))?;
                    let mut v = Vec::new();
                    f.take(len).read_to_end(&mut v)?;
                    Ok(v)
                });
                match r {
                    Ok(v) => ok(&String::from_utf8_lossy(&v)),
                    Err(e) => no(e),
                }
            }
            "w" => match std::fs::write(a, b) {
                Ok(()) => ok(""),
                Err(e) => no(e),
            },
            "a" => match std::fs::OpenOptions::new().append(true).create(true).open(a).and_then(|mut f| f.write_all(b.as_bytes())) {
                Ok(()) => ok(""),
                Err(e) => no(e),
            },
            "mv" => match std::fs::rename(a, b).or_else(|_| std::fs::copy(a, b).and_then(|_| std::fs::remove_file(a))) {
                Ok(()) => ok(""),
                Err(e) => no(e),
            },
            "sh" => match std::process::Command::new("sh").arg("-c").arg(a).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::inherit()).output() {
                Ok(o) => ok(&format!("{}\u{1}{}", o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout))),
                Err(e) => no(e),
            },
            "pn" => {
                self.seed(b);
                let x = (self.rng.next() >> 11) as f64 / (1u64 << 53) as f64;
                ok(&x.to_string())
            }
            "ps" => {
                self.seed(b);
                const AZ: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
                let s: String = (0..32).map(|_| AZ[(self.rng.next() % 52) as usize] as char).collect();
                ok(&s)
            }
            "fo" => {
                let mut it = b.splitn(2, '\u{1}');
                let (name, mode) = (it.next().unwrap_or(""), it.next().unwrap_or("r"));
                let m: String = mode.chars().filter(|&c| c != 'b').collect();
                let mut o = std::fs::OpenOptions::new();
                match m.as_str() {
                    "r" => o.read(true),
                    "w" => o.write(true).create(true).truncate(true),
                    "a" => o.append(true).create(true),
                    "r+" => o.read(true).write(true),
                    "w+" => o.read(true).write(true).create(true).truncate(true),
                    "a+" => o.read(true).append(true).create(true),
                    _ => return Arc::from(format!("0bad mode '{}'", mode).as_str()),
                };
                match o.open(a) {
                    Ok(f) => {
                        self.files.insert(name.to_string(), FileH { f: H::File(f), eof: false });
                        ok("")
                    }
                    Err(e) => no(e),
                }
            }
            "chk" => match std::fs::read(a) {
                Ok(bytes) => {
                    let mut h: u64 = 0xcbf29ce484222325;
                    for x in &bytes {
                        h ^= *x as u64;
                        h = h.wrapping_mul(0x100000001b3);
                    }
                    if format!("{:016x}", h) == b { ok("") } else { rs("0changed since this program was built; build it again") }
                }
                Err(e) => no(e),
            },
            "tc" => match std::net::TcpStream::connect(a) {
                Ok(c) => {
                    self.files.insert(b.to_string(), FileH { f: H::Net(std::io::BufReader::new(c)), eof: false });
                    ok("")
                }
                Err(e) => no(e),
            },
            "tl" => match std::net::TcpListener::bind(a) {
                Ok(l) => {
                    let port = l.local_addr().map(|x| x.port()).unwrap_or(0);
                    self.files.insert(b.to_string(), FileH { f: H::Listen(l), eof: false });
                    ok(&port.to_string())
                }
                Err(e) => no(e),
            },
            "ta" => {
                let r = match self.files.get(a).map(|h| &h.f) {
                    Some(H::Listen(l)) => l.accept(),
                    _ => Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no listener with that name")),
                };
                match r {
                    Ok((c, peer)) => {
                        self.files.insert(b.to_string(), FileH { f: H::Net(std::io::BufReader::new(c)), eof: false });
                        ok(&peer.to_string())
                    }
                    Err(e) => no(e),
                }
            }
            "fc" => {
                self.files.remove(a);
                ok("")
            }
            "feof" => ok(if self.files.get(a).map_or(true, |h| h.eof) { "1" } else { "0" }),
            "fw" | "fat" | "fra" | "frl" => {
                use std::io::Seek;
                let h = match self.files.get_mut(a) {
                    Some(h) => h,
                    None => return rs("0no open file"),
                };
                let r: std::io::Result<String> = match op {
                    "fw" => h.f.write_all(b.as_bytes()).map(|_| String::new()),
                    "fat" => {
                        let mut it = b.splitn(2, '\u{1}');
                        let off = parse(it.next().unwrap_or("0")) as u64;
                        let d = it.next().unwrap_or("");
                        h.f.seek(std::io::SeekFrom::Start(off)).and_then(|_| h.f.write_all(d.as_bytes())).map(|_| String::new())
                    }
                    "fra" => {
                        let p: Vec<f64> = b.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                        let (off, len) = (p.first().copied().unwrap_or(0.0) as u64, p.get(1).copied().unwrap_or(0.0) as usize);
                        let mut v = Vec::new();
                        let r = h.f.seek(std::io::SeekFrom::Start(off)).and_then(|_| (&mut h.f).take(len as u64).read_to_end(&mut v));
                        h.eof = v.len() < len;
                        r.map(|_| String::from_utf8_lossy(&v).into_owned())
                    }
                    _ if matches!(h.f, H::Net(_)) => {
                        let n = match &mut h.f {
                            H::Net(n) => n,
                            _ => unreachable!(),
                        };
                        let mut v = Vec::new();
                        let r = n.read_until(b'\n', &mut v);
                        h.eof = matches!(r, Ok(0));
                        while v.last() == Some(&b'\n') || v.last() == Some(&b'\r') {
                            v.pop();
                        }
                        r.map(|_| String::from_utf8_lossy(&v).into_owned())
                    }
                    _ => {
                        let mut out = Vec::new();
                        let mut buf = [0u8; 4096];
                        let mut got = false;
                        let mut res = Ok(());
                        loop {
                            let n = match h.f.read(&mut buf) {
                                Ok(n) => n,
                                Err(e) => {
                                    res = Err(e);
                                    break;
                                }
                            };
                            if n == 0 {
                                break;
                            }
                            got = true;
                            if let Some(p) = buf[..n].iter().position(|&c| c == b'\n') {
                                out.extend_from_slice(&buf[..p]);
                                let back = (n - p - 1) as i64;
                                if back > 0 {
                                    let _ = h.f.seek(std::io::SeekFrom::Current(-back));
                                }
                                break;
                            }
                            out.extend_from_slice(&buf[..n]);
                        }
                        h.eof = !got;
                        if out.last() == Some(&b'\r') {
                            out.pop();
                        }
                        res.map(|_| String::from_utf8_lossy(&out).into_owned())
                    }
                };
                match r {
                    Ok(s) => ok(&s),
                    Err(e) => no(e),
                }
            }
            _ => rs("0unknown effect"),
        }
    }
}

/// A lane function over a list, in order, for lanes with effects.
fn par_chunks<X: Sync, T: Send, F: Fn(&[X]) -> T + Sync>(xs: &[X], f: F) -> Vec<T> {
    let n = std::thread::available_parallelism().map_or(1, |n| n.get()).max(1);
    if n == 1 || xs.len() < 4096 {
        return vec![f(xs)];
    }
    let size = (xs.len() + n - 1) / n;
    std::thread::scope(|s| {
        let hs: Vec<_> = xs.chunks(size).map(|c| s.spawn(|| f(c))).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    })
}
