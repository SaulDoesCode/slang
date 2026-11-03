// s machine: values, parsing, the interpreter and the runtime that compiled programs share.
// Evaluation never recurses in Rust: calls, expansions and units are frames on `fs`.
#![allow(dead_code, unused_imports, unused_variables, unused_mut, unreachable_code, unused_macros, clippy::all)]

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::panic::{self, AssertUnwindSafe};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, Once, OnceLock};
use std::thread;

use crate::graph::{self, Graph, HOST_WORDS};
use crate::lanes::{self, EachRun, Fork, ForkRun, ForkSeen, Joined, LaneLog, View};
use std::cell::RefCell;

pub const MAIN_STACK: usize = 64 << 20;
pub const WORKER_STACK: usize = 16 << 20;

/// Set by the command line: `<--` and errors in workers end the process, as in s 1.0.
pub static CLI: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct Sym(pub u32);

pub const RESERVED: [&str; 14] = [
    "=", "n", "r", "rc", "rn", "$", "err", "eof", "out", "threads", "_", "args", "argc", "?",
];
pub const S_EQ: Sym = Sym(0);
pub const S_N: Sym = Sym(1);
pub const S_R: Sym = Sym(2);
pub const S_RC: Sym = Sym(3);
pub const S_RN: Sym = Sym(4);
pub const S_DOLLAR: Sym = Sym(5);
pub const S_ERR: Sym = Sym(6);
pub const S_EOF: Sym = Sym(7);
pub const S_OUT: Sym = Sym(8);
pub const S_THREADS: Sym = Sym(9);
pub const S_UNDER: Sym = Sym(10);
pub const S_ARGS: Sym = Sym(11);
pub const S_ARGC: Sym = Sym(12);
pub const S_Q: Sym = Sym(13);

#[derive(Clone, Debug)]
pub enum Val {
    N(f64),
    L(&'static str),
    R(Rc<str>),
    W(&'static str, Sym),
    K(Rc<str>, Sym),
}

impl Val {
    pub fn text(&self) -> Cow<'_, str> {
        match self {
            Val::N(x) => Cow::Owned(fmt_num(*x)),
            Val::L(s) => Cow::Borrowed(s),
            Val::R(s) | Val::K(s, _) => Cow::Borrowed(&**s),
            Val::W(s, _) => Cow::Borrowed(s),
        }
    }
    pub fn num(&self) -> f64 {
        match self {
            Val::N(x) => *x,
            Val::L(s) | Val::W(s, _) => parse_num(s),
            Val::R(s) | Val::K(s, _) => parse_num(s),
        }
    }
    pub fn of(s: String) -> Val {
        match canon_num(&s) {
            Some(x) => Val::N(x),
            None => Val::R(Rc::from(s)),
        }
    }
}

/// A list's items. A list whose items are all numbers keeps them as numbers; the first item
/// that is not a number turns it into values. What a program sees is the same either way.
#[derive(Clone, Debug)]
pub enum List {
    /// Numbers, shared until one holder changes them.
    N(Arc<Vec<f64>>),
    V(Vec<Val>),
}

impl Default for List {
    fn default() -> List {
        List::N(Arc::new(Vec::new()))
    }
}

impl List {
    pub fn nums_of(v: Vec<f64>) -> List {
        List::N(Arc::new(v))
    }
    pub fn from_vals(v: Vec<Val>) -> List {
        if v.iter().all(|x| matches!(x, Val::N(_))) {
            List::nums_of(v.into_iter().map(|x| if let Val::N(n) = x { n } else { 0.0 }).collect())
        } else {
            List::V(v)
        }
    }
    pub fn len(&self) -> usize {
        match self {
            List::N(v) => v.len(),
            List::V(v) => v.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn get(&self, i: usize) -> Option<Val> {
        match self {
            List::N(v) => v.get(i).map(|x| Val::N(*x)),
            List::V(v) => v.get(i).cloned(),
        }
    }
    fn vals(&mut self) -> &mut Vec<Val> {
        if let List::N(v) = self {
            *self = List::V(v.iter().map(|x| Val::N(*x)).collect());
        }
        match self {
            List::V(v) => v,
            List::N(_) => unreachable!(),
        }
    }
    pub fn push(&mut self, x: Val) {
        match (&mut *self, x) {
            (List::N(v), Val::N(n)) => Arc::make_mut(v).push(n),
            (List::V(v), x) => v.push(x),
            (_, x) => self.vals().push(x),
        }
    }
    pub fn pop(&mut self) -> Option<Val> {
        match self {
            List::N(v) => Arc::make_mut(v).pop().map(Val::N),
            List::V(v) => v.pop(),
        }
    }
    pub fn set(&mut self, i: usize, x: Val) {
        match (&mut *self, x) {
            (List::N(v), Val::N(n)) => Arc::make_mut(v)[i] = n,
            (List::V(v), x) => v[i] = x,
            (_, x) => self.vals()[i] = x,
        }
    }
    pub fn to_vals(&self) -> Vec<Val> {
        match self {
            List::N(v) => v.iter().map(|x| Val::N(*x)).collect(),
            List::V(v) => v.clone(),
        }
    }
    /// The items as numbers, when the list holds only numbers.
    pub fn nums(&self) -> Option<&[f64]> {
        match self {
            List::N(v) => Some(v),
            List::V(_) => None,
        }
    }
    pub fn texts(&self) -> Vec<String> {
        match self {
            List::N(v) => v.iter().map(|x| fmt_num(*x)).collect(),
            List::V(v) => v.iter().map(|x| x.text().into_owned()).collect(),
        }
    }
}

pub fn fmt_num(x: f64) -> String {
    if x == 0.0 { "0".to_string() } else { x.to_string() }
}

pub fn parse_num(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(0.0)
}

pub fn canon_num(s: &str) -> Option<f64> {
    let x = s.parse::<f64>().ok()?;
    if fmt_num(x) == s { Some(x) } else { None }
}

pub fn val_eq(a: &Val, b: &Val) -> bool {
    match (a, b) {
        (Val::N(x), Val::N(y)) if !x.is_nan() && !y.is_nan() => x == y,
        _ => a.text() == b.text(),
    }
}

#[derive(Default, Clone, Copy)]
pub struct Fx(u64);

impl std::hash::Hasher for Fx {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, b: &[u8]) {
        for c in b.chunks(8) {
            let mut v = 0u64;
            for (i, &x) in c.iter().enumerate() {
                v |= (x as u64) << (8 * i);
            }
            self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x517c_c1b7_2722_0a95);
        }
    }
    fn write_u32(&mut self, i: u32) {
        self.0 = (self.0.rotate_left(5) ^ i as u64).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

pub type FxMap<K, V> = HashMap<K, V, std::hash::BuildHasherDefault<Fx>>;
pub type FxSet<K> = HashSet<K, std::hash::BuildHasherDefault<Fx>>;

/// A name is local when it starts with `:` and has more after it.
pub fn is_local_name(s: &str) -> bool {
    s.len() > 1 && s.starts_with(':')
}

pub struct Interner {
    pub map: FxMap<Rc<str>, u32>,
    pub names: Vec<Rc<str>>,
    pub local: Vec<bool>,
    pub host: Vec<u8>,
}

impl Interner {
    pub fn new() -> Interner {
        let mut it = Interner { map: FxMap::default(), names: Vec::new(), local: Vec::new(), host: Vec::new() };
        for n in RESERVED.iter() {
            it.sym(n);
        }
        it
    }
    pub fn sym(&mut self, s: &str) -> Sym {
        if let Some(&k) = self.map.get(s) {
            return Sym(k);
        }
        let rc: Rc<str> = Rc::from(s);
        let k = self.names.len() as u32;
        self.names.push(rc.clone());
        self.local.push(is_local_name(s));
        self.host.push(HOST_WORDS.iter().position(|w| *w == s).map(|i| i as u8 + 1).unwrap_or(0));
        self.map.insert(rc, k);
        Sym(k)
    }
    pub fn name(&self, s: Sym) -> &str {
        &self.names[s.0 as usize]
    }
    #[inline(always)]
    pub fn is_local(&self, s: Sym) -> bool {
        self.local[s.0 as usize]
    }
}

macro_rules! cmds {
    ($($w:literal => $c:ident,)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Cmd { $($c,)* }
        pub fn cmd_of(w: &str) -> Option<Cmd> {
            match w { $($w => Some(Cmd::$c),)* _ => None }
        }
        impl Cmd {
            pub fn word(self) -> &'static str { match self { $(Cmd::$c => $w,)* } }
        }
        pub const CMD_WORDS: &[&str] = &[$($w,)*];
    };
}

cmds! {
    ">" => Print, ">_" => PrintRaw, "<" => Shell, "<_" => ReadIn,
    "w" => Write, "+w" => Append, "mv" => Move, "r" => Read, "r@" => ReadAt,
    "?)" => Match, "&&&" => Repeat, "***" => Par,
    "n" => SetN, "+" => Add, "-" => Sub, "*" => Mul, "/" => Div, "^" => Pow, "%" => Mod,
    "!!" => Not, "=" => Eq, "!=" => Ne, ";" => AndNe, "&" => AndEq,
    "==" => NumEq, "<<" => Lt, ">>" => Gt, "<<=" => Le, ">>=" => Ge,
    "l" => LPush, "-l" => LPop, "[l]" => LGet, "l=" => LSet, "l-" => LDel,
    "#l" => LLen, "/l" => Split, "l/" => Join, "#s" => SLen,
    "f" => Open, "+f" => FWriteAt, "@f" => FReadAt, "f-" => FClose,
    "f_" => FWriteLn, "_f" => FReadLn, "?eof" => Eof,
    "t" => Spawn, "-t" => Kill, "<t" => Send, ">t" => Recv,
    "#prng" => PrngN, "`prng" => PrngS,
    "><" => Replace, "//" => Comment, "<-" => Ret, "<--" => Exit,
    "(" => Open1, ")" => Close1, "~" => Eval, "##" => Depth,
    "tcp" => Tcp,
}

impl Cmd {
    /// Commands that reach outside the machine: the shell, stdin, files, threads, the
    /// network, and random numbers. A folding run stops at them.
    pub fn outside(self) -> bool {
        use Cmd::*;
        matches!(
            self,
            Shell | ReadIn | Write | Append | Move | Read | ReadAt | Par | Open | FWriteAt | FReadAt | FWriteLn | FReadLn | Eof | FClose | Spawn | Kill | Send | Recv | Tcp | PrngN | PrngS
        )
    }
    pub fn runs_code(self) -> bool {
        matches!(self, Cmd::Match | Cmd::Par | Cmd::Recv | Cmd::Open | Cmd::FWriteAt | Cmd::FWriteLn | Cmd::Eval | Cmd::Tcp | Cmd::Repeat)
    }
    pub fn keeps_stack(self) -> bool {
        matches!(self, Cmd::Open1 | Cmd::Close1 | Cmd::Eval | Cmd::Depth | Cmd::Ret)
    }
    /// Commands that may have to wait for something outside the machine.
    pub fn may_wait(self) -> bool {
        matches!(self, Cmd::Recv)
    }
}

#[derive(Debug, Clone)]
pub enum Part {
    T(String),
    V(Sym),
}

#[derive(Debug, Clone)]
pub enum Op {
    Push(Val),
    Interp(Vec<Part>),
    Word(Rc<str>, Sym, Option<f64>),
    Cmd(Cmd),
    Native(Cmd),
    Lookup(Sym),
    Pop(Sym),
    Def(Sym),
    SetMem(Sym),
    SetNum(Sym),
    Inc(Sym),
    Dec(Sym),
    Call(Sym),
    CallPass(Sym),
    Cond(bool, Box<Op>),
    /// A bead region: a fork, its lanes, its join.
    Lanes(Rc<Fork>),
}

pub const NO_UNIT: u32 = u32::MAX;

/// Parsed text: one op per token, with the character offset where each token starts.
#[derive(Debug)]
pub struct Code {
    pub ops: Vec<Op>,
    pub spans: Vec<u32>,
    pub unit: u32,
    pub src: Rc<str>,
}

pub fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\n' | '\t' | '\r')
}

pub const SUFFIX: &str = "?!~$`#;:.@";

pub fn active_form(t: &str) -> bool {
    let mut cs = t.chars();
    let first = cs.next();
    let last = t.chars().last();
    let long = t.chars().nth(1).is_some();
    long && (first == Some('\\') || last.map_or(false, |c| SUFFIX.contains(c)))
}

pub fn classify(w: &str, it: &mut Interner) -> Op {
    if let Some(c) = cmd_of(w) {
        return Op::Cmd(c);
    }
    if w.len() > 1 && w.starts_with('\\') {
        let rest = &w[1..];
        if let Some(c) = cmd_of(rest) {
            return Op::Native(c);
        }
        return match classify(rest, it) {
            Op::Word(t, _, _) => Op::Push(Val::of(t.to_string())),
            op => op,
        };
    }
    let last = w.chars().last().unwrap();
    let stem = &w[..w.len() - last.len_utf8()];
    if !stem.is_empty() {
        match last {
            '?' => return Op::Cond(true, Box::new(classify(stem, it))),
            '!' => return Op::Cond(false, Box::new(classify(stem, it))),
            '~' => return Op::Lookup(it.sym(stem)),
            '@' => return Op::Pop(it.sym(stem)),
            '$' => return Op::Def(it.sym(stem)),
            '`' => return Op::SetMem(it.sym(stem)),
            '#' => return Op::SetNum(it.sym(stem)),
            ';' => return Op::Inc(it.sym(stem)),
            ':' => return Op::Dec(it.sym(stem)),
            '.' => {
                if stem.len() > 1 && stem.ends_with('.') {
                    return Op::CallPass(it.sym(&stem[..stem.len() - 1]));
                }
                return Op::Call(it.sym(stem));
            }
            _ => {}
        }
    }
    Op::Word(Rc::from(w), it.sym(w), canon_num(w))
}

/// `⟨stem⟩suffix`: the stem is taken whole and the suffix chooses the form.
pub fn classify_angle(stem: &str, suf: &str, it: &mut Interner) -> Result<Op, String> {
    if suf.is_empty() {
        return Ok(Op::Word(Rc::from(stem), it.sym(stem), None));
    }
    if suf == ".." {
        return Ok(Op::CallPass(it.sym(stem)));
    }
    let last = suf.chars().last().unwrap();
    let head = &suf[..suf.len() - last.len_utf8()];
    if last == '?' || last == '!' {
        let inner = classify_angle(stem, head, it)?;
        return Ok(Op::Cond(last == '?', Box::new(inner)));
    }
    if !head.is_empty() {
        return Err(format!("⟨{}⟩{}: unknown suffix", stem, suf));
    }
    match last {
        '~' => Ok(Op::Lookup(it.sym(stem))),
        '@' => Ok(Op::Pop(it.sym(stem))),
        '$' => Ok(Op::Def(it.sym(stem))),
        '`' => Ok(Op::SetMem(it.sym(stem))),
        '#' => Ok(Op::SetNum(it.sym(stem))),
        ';' => Ok(Op::Inc(it.sym(stem))),
        ':' => Ok(Op::Dec(it.sym(stem))),
        '.' => Ok(Op::Call(it.sym(stem))),
        _ => Err(format!("⟨{}⟩{}: unknown suffix", stem, suf)),
    }
}

fn brace(cs: &[char], i: usize, it: &mut Interner) -> Result<(Vec<Part>, usize), String> {
    let n = cs.len();
    let mut depth = 1;
    let mut j = i + 1;
    let mut parts = Vec::new();
    let mut cur = String::new();
    while j < n {
        let c = cs[j];
        match c {
            '{' => {
                depth += 1;
                cur.push(c);
                j += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    if !cur.is_empty() {
                        parts.push(Part::T(cur));
                    }
                    return Ok((parts, j + 1));
                }
                cur.push(c);
                j += 1;
            }
            '~' if depth == 1 => {
                let mut k = j + 1;
                while k < n && cs[k] != '~' && cs[k] != '{' && cs[k] != '}' && !is_ws(cs[k]) {
                    k += 1;
                }
                if k < n && cs[k] == '~' {
                    if k == j + 1 {
                        cur.push('~');
                    } else {
                        if !cur.is_empty() {
                            parts.push(Part::T(std::mem::take(&mut cur)));
                        }
                        let name: String = cs[j + 1..k].iter().collect();
                        parts.push(Part::V(it.sym(&name)));
                    }
                    j = k + 1;
                } else {
                    cur.push('~');
                    j += 1;
                }
            }
            _ => {
                cur.push(c);
                j += 1;
            }
        }
    }
    Err("unclosed {".to_string())
}

fn lit_op(prefix: String, parts: Vec<Part>) -> Op {
    let mut ps: Vec<Part> = Vec::new();
    for p in std::iter::once(Part::T(prefix)).chain(parts) {
        match (ps.last_mut(), p) {
            (_, Part::T(t)) if t.is_empty() => {}
            (Some(Part::T(a)), Part::T(t)) => a.push_str(&t),
            (_, p) => ps.push(p),
        }
    }
    if ps.iter().all(|p| matches!(p, Part::T(_))) {
        let t = match ps.pop() {
            Some(Part::T(t)) => t,
            _ => String::new(),
        };
        Op::Push(Val::of(t))
    } else {
        Op::Interp(ps)
    }
}

pub fn parse(src: &str, it: &mut Interner) -> Result<Code, String> {
    if lanes::has_marks(src) {
        return lanes::parse_beads(src, it);
    }
    parse_plain(src, it)
}

/// Text without bead marks.
pub fn parse_plain(src: &str, it: &mut Interner) -> Result<Code, String> {
    let cs: Vec<char> = src.chars().collect();
    let n = cs.len();
    let mut ops = Vec::new();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < n {
        if is_ws(cs[i]) {
            i += 1;
            continue;
        }
        let start = i as u32;
        if cs[i] == '⟨' {
            let mut j = i + 1;
            while j < n && cs[j] != '⟩' {
                j += 1;
            }
            if j >= n {
                return Err("unclosed ⟨".to_string());
            }
            let stem: String = cs[i + 1..j].iter().collect();
            let mut k = j + 1;
            while k < n && !is_ws(cs[k]) {
                k += 1;
            }
            let suf: String = cs[j + 1..k].iter().collect();
            ops.push(classify_angle(&stem, &suf, it)?);
            spans.push(start);
            i = k;
            continue;
        }
        let mut word = String::new();
        let mut done = false;
        while i < n && !is_ws(cs[i]) {
            match cs[i] {
                '{' => {
                    let (parts, j) = brace(&cs, i, it)?;
                    ops.push(lit_op(std::mem::take(&mut word), parts));
                    spans.push(start);
                    i = j;
                    done = true;
                    break;
                }
                '\'' => {
                    let mut j = i + 1;
                    while j < n && cs[j] != '\'' {
                        j += 1;
                    }
                    if j >= n {
                        return Err("unclosed '".to_string());
                    }
                    let t: String = cs[i + 1..j].iter().collect();
                    let mut w = std::mem::take(&mut word);
                    w.push_str(&t);
                    ops.push(Op::Push(Val::of(w)));
                    spans.push(start);
                    i = j + 1;
                    done = true;
                    break;
                }
                c => {
                    word.push(c);
                    i += 1;
                }
            }
        }
        if !done && !word.is_empty() {
            ops.push(classify(&word, it));
            spans.push(start);
        }
    }
    Ok(Code { ops, spans, unit: NO_UNIT, src: Rc::from(src) })
}

/// A compiled chunk runs from state `st` until it ends, tail-calls, or pushes a frame.
pub type Chunk = fn(&mut Rt, u32) -> Flow;

pub enum Flow {
    Done,
    Tail(Sym),
    /// Frames were pushed: resume at this state when they finish. `END` means nothing is left.
    Yield(u32),
    /// The op at this state must wait: run it again later.
    Wait(u32),
}

pub const END: u32 = u32::MAX;

/// A primitive written in Rust: it works on rt.args like a library word.
pub type NativeFn = fn(&mut Rt);

#[derive(Clone)]
pub enum Script {
    C(Chunk),
    U(Rc<Code>),
    N(NativeFn),
}

#[derive(Clone)]
pub struct Macro {
    pub val: Val,
    pub code: Option<Script>,
    pub gen: u64,
}

pub const UNBOUND: u64 = u64::MAX;

pub struct Program {
    pub syms: &'static [&'static str],
    pub chunks: &'static [(&'static str, Chunk)],
    pub files: &'static [(&'static str, usize)],
    pub lib: &'static [(&'static str, &'static str)],
    pub natives: &'static [(&'static str, NativeFn)],
    pub cache: OnceLock<FxMap<&'static str, Chunk>>,
}

impl Program {
    pub const fn new(
        syms: &'static [&'static str],
        chunks: &'static [(&'static str, Chunk)],
        files: &'static [(&'static str, usize)],
    ) -> Program {
        Program { syms, chunks, files, lib: &[], natives: &[], cache: OnceLock::new() }
    }
    pub const fn with_natives(
        syms: &'static [&'static str],
        chunks: &'static [(&'static str, Chunk)],
        files: &'static [(&'static str, usize)],
        natives: &'static [(&'static str, NativeFn)],
    ) -> Program {
        Program { syms, chunks, files, lib: &[], natives, cache: OnceLock::new() }
    }
    pub const fn with_lib(lib: &'static [(&'static str, &'static str)]) -> Program {
        Program { syms: &[], chunks: &[], files: &[], lib, natives: &[], cache: OnceLock::new() }
    }
    pub const fn full(
        syms: &'static [&'static str],
        chunks: &'static [(&'static str, Chunk)],
        files: &'static [(&'static str, usize)],
        lib: &'static [(&'static str, &'static str)],
        natives: &'static [(&'static str, NativeFn)],
    ) -> Program {
        Program { syms, chunks, files, lib, natives, cache: OnceLock::new() }
    }
    pub fn lib_src(&self, path: &str) -> Option<&'static str> {
        let p = path.strip_prefix("./").unwrap_or(path);
        self.lib.iter().find(|f| f.0 == p).map(|f| f.1)
    }
    pub fn find(&self, src: &str) -> Option<Chunk> {
        if self.chunks.is_empty() {
            return None;
        }
        self.cache
            .get_or_init(|| self.chunks.iter().map(|&(s, f)| (s, f)).collect())
            .get(src)
            .copied()
    }
    pub fn file(&self, path: &str) -> Option<Chunk> {
        self.files.iter().find(|f| f.0 == path).map(|f| self.chunks[f.1].1)
    }
}

/// Directory of the program's main file: code paths not found from the working
/// directory are looked up there.
pub static BASE: OnceLock<std::path::PathBuf> = OnceLock::new();

pub fn code_path(p: &str) -> String {
    let path = std::path::Path::new(p);
    if path.is_file() || path.is_absolute() {
        return p.to_string();
    }
    if let Some(b) = BASE.get() {
        let q = b.join(path);
        if q.is_file() {
            return q.to_string_lossy().into_owned();
        }
    }
    p.to_string()
}

pub static EMPTY: Program = Program::new(&[], &[], &[]);

pub enum Handle {
    File(File),
    Net(BufReader<TcpStream>),
    Listen(TcpListener),
}

pub struct FileH {
    pub h: Handle,
    pub eof: bool,
}

fn not_stream() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "a listener only accepts")
}

impl FileH {
    pub fn new(h: Handle) -> FileH {
        FileH { h, eof: false }
    }
    fn write_at(&mut self, off: u64, data: &[u8]) -> io::Result<()> {
        match &mut self.h {
            Handle::File(f) => f.seek(SeekFrom::Start(off)).and_then(|_| f.write_all(data)),
            Handle::Net(n) => n.get_mut().write_all(data).and_then(|_| n.get_mut().flush()),
            Handle::Listen(_) => Err(not_stream()),
        }
    }
    fn write(&mut self, data: &[u8]) -> io::Result<()> {
        match &mut self.h {
            Handle::File(f) => f.write_all(data),
            Handle::Net(n) => n.get_mut().write_all(data).and_then(|_| n.get_mut().flush()),
            Handle::Listen(_) => Err(not_stream()),
        }
    }
    fn read_at(&mut self, off: u64, len: usize) -> io::Result<Vec<u8>> {
        let mut b = Vec::with_capacity(len.min(1 << 20));
        match &mut self.h {
            Handle::File(f) => {
                f.seek(SeekFrom::Start(off))?;
                f.take(len as u64).read_to_end(&mut b)?;
                self.eof = b.len() < len;
            }
            Handle::Net(n) => {
                let avail = n.fill_buf()?;
                let k = avail.len().min(len);
                b.extend_from_slice(&avail[..k]);
                n.consume(k);
                self.eof = k == 0 && len > 0;
            }
            Handle::Listen(_) => return Err(not_stream()),
        }
        Ok(b)
    }
    fn read_line(&mut self) -> io::Result<String> {
        let mut out = Vec::new();
        match &mut self.h {
            Handle::File(f) => {
                let mut buf = [0u8; 4096];
                let mut got = false;
                loop {
                    let n = f.read(&mut buf)?;
                    if n == 0 {
                        break;
                    }
                    got = true;
                    if let Some(p) = buf[..n].iter().position(|&b| b == b'\n') {
                        out.extend_from_slice(&buf[..p]);
                        let back = (n - p - 1) as i64;
                        if back > 0 {
                            f.seek(SeekFrom::Current(-back))?;
                        }
                        break;
                    }
                    out.extend_from_slice(&buf[..n]);
                }
                self.eof = !got;
            }
            Handle::Net(n) => {
                let k = n.read_until(b'\n', &mut out)?;
                self.eof = k == 0;
                if out.last() == Some(&b'\n') {
                    out.pop();
                }
            }
            Handle::Listen(_) => return Err(not_stream()),
        }
        if out.last() == Some(&b'\r') {
            out.pop();
        }
        Ok(String::from_utf8_lossy(&out).into_owned())
    }
}

pub struct Worker {
    pub tx: Sender<String>,
    pub rx: Receiver<String>,
}

pub struct Rng(pub [u64; 4]);

fn splitmix(z: &mut u64) -> u64 {
    *z = z.wrapping_add(0x9E3779B97F4A7C15);
    let mut x = *z;
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
    x ^ (x >> 31)
}

impl Rng {
    pub fn seeded(a: u64, b: u64) -> Rng {
        let mut z = a ^ b.rotate_left(32);
        let mut s = [0u64; 4];
        for x in s.iter_mut() {
            *x = splitmix(&mut z);
        }
        Rng(s)
    }
    pub fn next(&mut self) -> u64 {
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
    pub fn f64(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

pub fn os_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static C: AtomicU64 = AtomicU64::new(0);
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(C.fetch_add(1, Ordering::Relaxed));
    h.finish()
}

fn grow<T>(v: &mut Vec<Option<T>>, i: usize) -> &mut Option<T> {
    if v.len() <= i {
        v.resize_with(i + 1, || None);
    }
    &mut v[i]
}

fn open_mode(path: &str, mode: &str) -> io::Result<File> {
    let m: String = mode.chars().filter(|&c| c != 'b').collect();
    let mut o = OpenOptions::new();
    match m.as_str() {
        "r" => o.read(true),
        "w" => o.write(true).create(true).truncate(true),
        "a" => o.append(true).create(true),
        "r+" => o.read(true).write(true),
        "w+" => o.read(true).write(true).create(true).truncate(true),
        "a+" => o.read(true).append(true).create(true),
        _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("bad mode '{}'", mode))),
    };
    o.open(path)
}

fn shell(c: &str) -> io::Result<std::process::Output> {
    let mut cmd = if cfg!(windows) {
        let mut k = Command::new("cmd");
        k.arg("/C").arg(c);
        k
    } else {
        let mut k = Command::new("sh");
        k.arg("-c").arg(c);
        k
    };
    cmd.stdin(Stdio::null()).stderr(Stdio::inherit()).output()
}

// errors

/// An s error: ends the run it happens in.
pub struct SErr(pub String);
/// `<--`: ends the program with a code.
pub struct SExit(pub i32);

pub fn quiet_panics() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let prev = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            let p = info.payload();
            if p.is::<SErr>() || p.is::<SExit>() {
                return;
            }
            prev(info)
        }));
    });
}

// output

/// Where `>` and `>_` write: standard output, or a buffer shared with whoever reads the run.
pub enum Out {
    Std(BufWriter<io::Stdout>),
    Buf(Arc<Mutex<Vec<u8>>>),
}

impl Out {
    pub fn std() -> Out {
        Out::Std(BufWriter::with_capacity(1 << 16, io::stdout()))
    }
    pub fn fork(&self) -> Out {
        match self {
            Out::Std(_) => Out::std(),
            Out::Buf(b) => Out::Buf(b.clone()),
        }
    }
    pub fn write_all(&mut self, b: &[u8]) {
        match self {
            Out::Std(w) => {
                let _ = w.write_all(b);
            }
            Out::Buf(v) => {
                if let Ok(mut v) = v.lock() {
                    v.extend_from_slice(b);
                }
            }
        }
    }
    pub fn flush(&mut self) {
        if let Out::Std(w) = self {
            let _ = w.flush();
        }
    }
}

// frames

pub enum Body {
    Ops(Rc<Code>),
    Chunk(Chunk),
    /// `n name.` and `&&&`: call `k` for i = 0 … n-1, clearing the line before each.
    Repeat { k: Sym, i: u64, n: u64, amp: bool },
    /// Waiting for something the host gives back (an rpc return, a join).
    Await(u32),
    /// A fork running its lanes one at a time.
    Lanes(Rc<RefCell<ForkRun>>),
    /// reduce and lanewise, one lane at a time.
    Each(Rc<RefCell<EachRun>>),
    /// try: an error above this frame runs h.
    Catch { h: Rc<str>, frames: usize },
}

pub struct Frame {
    pub body: Body,
    pub pc: u32,
    pub bound: u64,
}

pub enum Status {
    Done,
    /// The budget ran out; run again to continue.
    Ran,
    /// An op waits for something outside; run again later.
    Wait,
    /// A breakpoint: run again to continue past it.
    Break,
    Error(String),
    Exit(i32),
}

/// What an op that must wait is waiting for.
#[derive(Clone, Debug, PartialEq)]
pub enum Wait {
    Recv(usize),
    Host(u32),
}

/// Access rules a zone puts on a captured name.
pub const G_NONE: u8 = 0;
pub const G_MOVED: u8 = 1;
pub const G_LENT_RO: u8 = 2;
pub const G_BORROWED: u8 = 3;
pub const G_LENT: u8 = 4;
/// A `mut` capture whose zone was lost.
pub const G_LOST: u8 = 5;

pub struct Rt {
    pub prog: &'static Program,
    pub it: Interner,
    pub args: Vec<Val>,
    pub ow: bool,
    pub mem: Vec<Option<Val>>,
    pub nums: Vec<Option<f64>>,
    pub scripts: Vec<Option<(Script, u64)>>,
    pub lists: Vec<Option<List>>,
    pub repl: FxMap<Rc<str>, Vec<Macro>>,
    pub repl_num: bool,
    pub gen: u64,
    pub bound: u64,
    pub frames: Vec<Vec<Val>>,
    pub peer: String,
    pub quiet: Vec<u64>,
    pub quiet_cmd: Vec<u64>,
    pub quiet_site: Vec<u64>,
    pub files: HashMap<u32, FileH>,
    pub workers: Vec<Option<Worker>>,
    pub units: FxMap<String, Rc<Code>>,
    pub unit_ids: FxMap<String, u32>,
    pub rng: Rng,
    pub out: Out,
    // machine
    pub fs: Vec<Frame>,
    pub touched: Vec<Vec<Sym>>,
    pub lmap: FxMap<(u32, u32), Sym>,
    pub site: (u32, u32),
    pub breaks: FxSet<u64>,
    pub brk_skip: Option<u64>,
    pub wait: Option<Wait>,
    pub nonblock: bool,
    pub stdin: bool,
    /// Lines `<_` reads in place of the process's stdin, when set.
    pub input: Option<std::collections::VecDeque<String>>,
    pub guard: Vec<u8>,
    pub guards: bool,
    pub g: Option<Box<Graph>>,
    /// Ops run so far: budgets are counted against it.
    pub ops: u64,
    /// What is left of the budget while a chunk runs; self tail calls inside it spend it.
    pub fuel: u64,
    // lanes
    pub lanelog: Option<Box<LaneLog>>,
    pub lane_at: Option<usize>,
    pub joined: Option<Rc<Joined>>,
    pub view: Option<View>,
    /// When set, how each fork ran.
    pub fork_log: Option<Vec<ForkSeen>>,
    pub fork_cache: FxMap<usize, Rc<Fork>>,
    /// Counts writes to names, lists, scripts and definitions: a snapshot taken at one count
    /// holds while the count stands.
    pub epoch: u64,
    pub snap: Option<(u64, usize, Arc<lanes::Snapshot>)>,
    /// Folding at build time: an op that reaches outside the machine stops the run.
    pub fold: bool,
    /// What stopped a folding run.
    pub fold_hit: Option<String>,
    /// The op of the program's own text that a folding run is in.
    pub fold_top: u32,
    /// The texts of the files a compiled program carries, read in place of the disk.
    pub file_texts: Option<FxMap<String, String>>,
}

impl Rt {
    pub fn new(prog: &'static Program) -> Rt {
        Rt::with_out(prog, Out::std())
    }

    pub fn with_out(prog: &'static Program, out: Out) -> Rt {
        quiet_panics();
        let mut it = Interner::new();
        for s in prog.syms {
            it.sym(s);
        }
        let n = it.names.len();
        let mut rt = Rt {
            prog,
            it,
            args: Vec::with_capacity(16),
            ow: false,
            mem: vec![None; n],
            nums: vec![None; n],
            scripts: vec![None; n],
            lists: Vec::new(),
            repl: FxMap::default(),
            repl_num: false,
            gen: 1,
            bound: UNBOUND,
            frames: Vec::new(),
            peer: String::new(),
            quiet: Vec::new(),
            quiet_cmd: vec![0; CMD_WORDS.len()],
            quiet_site: Vec::new(),
            files: HashMap::new(),
            workers: Vec::new(),
            units: FxMap::default(),
            unit_ids: FxMap::default(),
            rng: Rng::seeded(os_u64(), os_u64()),
            out,
            fs: Vec::new(),
            touched: Vec::new(),
            lmap: FxMap::default(),
            site: (NO_UNIT, 0),
            breaks: FxSet::default(),
            brk_skip: None,
            wait: None,
            nonblock: false,
            stdin: true,
            input: None,
            guard: Vec::new(),
            guards: false,
            g: None,
            ops: 0,
            fuel: 0,
            lanelog: None,
            lane_at: None,
            joined: None,
            view: None,
            fork_log: None,
            fork_cache: FxMap::default(),
            epoch: 0,
            snap: None,
            fold: false,
            fold_hit: None,
            fold_top: 0,
            file_texts: None,
        };
        for &(name, f) in lanes::WORDS {
            rt.add_native(name, f);
        }
        rt.add_natives();
        rt
    }

    fn add_natives(&mut self) {
        for &(name, f) in self.prog.natives {
            let m = Macro { val: Val::L(name), code: Some(Script::N(f)), gen: 0 };
            self.repl.entry(Rc::from(name)).or_default().insert(0, m);
        }
    }

    /// Adds natives after creation (the studio's own natives).
    pub fn add_native(&mut self, name: &'static str, f: NativeFn) {
        let m = Macro { val: Val::L(name), code: Some(Script::N(f)), gen: 0 };
        self.repl.entry(Rc::from(name)).or_default().insert(0, m);
    }

    // helpers for natives

    /// Removes and returns the top value; an empty line is an error.
    pub fn pop(&mut self, who: &str) -> Val {
        match self.args.pop() {
            Some(v) => v,
            None => self.fail(format!("{}: stack is empty", who)),
        }
    }
    pub fn pop_num(&mut self, who: &str) -> f64 {
        self.pop(who).num()
    }
    pub fn pop_text(&mut self, who: &str) -> String {
        self.pop(who).text().into_owned()
    }
    pub fn push_num(&mut self, x: f64) {
        self.args.push(Val::N(x));
    }
    pub fn push_text(&mut self, s: String) {
        self.args.push(Val::of(s));
    }

    pub fn set_argv(&mut self, argv: Vec<String>) {
        let n = argv.len() as f64;
        let l: Vec<Val> = argv.into_iter().map(Val::of).collect();
        self.epoch += 1;
        *grow(&mut self.lists, S_ARGS.0 as usize) = Some(List::from_vals(l));
        self.nset(S_ARGC, n);
    }

    pub fn fail(&mut self, msg: String) -> ! {
        self.out.flush();
        let msg = match self.lane_at {
            Some(i) if !msg.starts_with("lane ") => format!("lane {}: {}", i, msg),
            _ => msg,
        };
        panic::panic_any(SErr(msg))
    }

    pub fn flush(&mut self) {
        self.out.flush();
    }

    pub fn name(&self, k: Sym) -> String {
        let n = self.it.name(k);
        match n.find('\u{1}') {
            Some(i) => n[..i].to_string(),
            None => n.to_string(),
        }
    }

    // locals

    /// The storage name a `:` name has in the running activation.
    #[inline(always)]
    pub fn l(&mut self, k: Sym) -> Sym {
        if self.fold && (k == S_ARGS || k == S_ARGC) {
            self.impure("the arguments")
        }
        if self.it.is_local(k) { self.local_sym(k) } else { k }
    }
    /// A folding run reached outside the machine: it stops here, and no try catches it.
    pub fn impure(&mut self, what: &str) -> ! {
        self.fold_hit = Some(what.to_string());
        self.fail(format!("fold stops at {}", what))
    }
    #[inline(never)]
    fn local_sym(&mut self, k: Sym) -> Sym {
        let d = self.fs.len().saturating_sub(1) as u32;
        if let Some(&s) = self.lmap.get(&(k.0, d)) {
            self.touch(d, s);
            return s;
        }
        let name = format!("{}\u{1}{}", self.it.name(k), d);
        let s = self.it.sym(&name);
        self.it.local[s.0 as usize] = false;
        self.lmap.insert((k.0, d), s);
        self.touch(d, s);
        s
    }
    fn touch(&mut self, d: u32, s: Sym) {
        let d = d as usize;
        if self.touched.len() <= d {
            self.touched.resize_with(d + 1, Vec::new);
        }
        if !self.touched[d].contains(&s) {
            self.touched[d].push(s);
        }
    }
    fn clear_locals(&mut self, d: usize) {
        if d < self.touched.len() && !self.touched[d].is_empty() {
            self.epoch += 1;
            let ss = std::mem::take(&mut self.touched[d]);
            for s in ss {
                let i = s.0 as usize;
                if let Some(v) = self.mem.get_mut(i) {
                    *v = None;
                }
                if let Some(v) = self.nums.get_mut(i) {
                    *v = None;
                }
                if let Some(v) = self.lists.get_mut(i) {
                    *v = None;
                }
                if let Some(v) = self.scripts.get_mut(i) {
                    *v = None;
                }
            }
        }
    }
    /// Sets a local of the frame at depth d (a sphere body's `:i`).
    pub fn set_local_num(&mut self, name: &str, d: usize, x: f64) {
        let k = self.it.sym(name);
        let key = (k.0, d as u32);
        let s = match self.lmap.get(&key) {
            Some(&s) => s,
            None => {
                let nm = format!("{}\u{1}{}", name, d);
                let s = self.it.sym(&nm);
                self.it.local[s.0 as usize] = false;
                self.lmap.insert(key, s);
                s
            }
        };
        self.touch(d as u32, s);
        self.epoch += 1;
        *grow(&mut self.nums, s.0 as usize) = Some(x);
    }

    // guards (zone captures)

    fn guard_of(&self, k: Sym) -> u8 {
        self.guard.get(k.0 as usize).copied().unwrap_or(0)
    }
    #[cold]
    #[inline(never)]
    fn guard_r(&mut self, k: Sym) {
        if self.guards {
            match self.guard_of(k) {
                G_MOVED => self.fail(format!("moved: {}", self.name(k))),
                G_LENT => self.fail(format!("lent: {}", self.name(k))),
                G_LOST => self.fail(format!("zone lost: {}", self.name(k))),
                _ => {}
            }
        }
    }
    #[cold]
    #[inline(never)]
    fn guard_w(&mut self, k: Sym) {
        if self.guards {
            match self.guard_of(k) {
                G_MOVED | G_LOST => self.guard[k.0 as usize] = G_NONE,
                G_LENT | G_LENT_RO => self.fail(format!("lent: {}", self.name(k))),
                G_BORROWED => self.fail(format!("borrowed: {}", self.name(k))),
                _ => {}
            }
        }
    }
    pub fn set_guard(&mut self, k: Sym, g: u8) {
        let i = k.0 as usize;
        if self.guard.len() <= i {
            self.guard.resize(i + 1, 0);
        }
        self.guard[i] = g;
        self.guards = self.guard.iter().any(|&x| x != 0);
    }

    // stack

    #[inline]
    pub fn push(&mut self, v: Val) {
        self.args.push(v);
    }
    pub fn push_string(&mut self, s: String) {
        self.args.push(Val::of(s));
    }
    #[inline]
    pub fn word(&mut self, t: &'static str, k: Sym) {
        if !self.quiet_sym(k, t) && self.try_expand(t) {
            return;
        }
        if self.g.is_some() && self.it.host[k.0 as usize] != 0 {
            let h = self.it.host[k.0 as usize] - 1;
            graph::host_word(self, h);
            return;
        }
        self.args.push(Val::W(t, k));
    }
    #[inline]
    pub fn word_n(&mut self, x: f64, t: &'static str) {
        if self.repl_num && self.try_expand(t) {
            return;
        }
        self.args.push(Val::N(x));
    }
    fn word_rc(&mut self, t: &Rc<str>, k: Sym, n: Option<f64>) {
        if n.is_some() {
            if self.repl_num && self.try_expand(t) {
                return;
            }
        } else {
            if !self.quiet_sym(k, t) && self.try_expand(t) {
                return;
            }
            if self.g.is_some() && self.it.host[k.0 as usize] != 0 {
                let h = self.it.host[k.0 as usize] - 1;
                graph::host_word(self, h);
                return;
            }
        }
        self.args.push(match n {
            Some(x) => Val::N(x),
            None => Val::K(t.clone(), k),
        });
    }

    // replacements: a word with a visible replacement runs the replacement's code

    /// The newest definition of t that the running code can see.
    #[inline]
    pub fn visible(&self, t: &str) -> Option<&Macro> {
        let b = self.bound;
        self.repl.get(t).and_then(|v| v.iter().rev().find(|m| m.gen < b))
    }
    #[inline]
    pub fn repl_active(&self, t: &str) -> bool {
        !self.repl.is_empty() && self.visible(t).is_some()
    }
    /// True when name t has no definitions at all; remembered until the next definition.
    #[inline]
    pub fn quiet_sym(&mut self, k: Sym, t: &str) -> bool {
        if self.repl.is_empty() {
            return true;
        }
        let i = k.0 as usize;
        if self.quiet.get(i) == Some(&self.gen) {
            return true;
        }
        if self.repl.contains_key(t) {
            return false;
        }
        if self.quiet.len() <= i {
            self.quiet.resize(i + 1, 0);
        }
        self.quiet[i] = self.gen;
        true
    }
    #[inline]
    pub fn quiet_cmd(&mut self, c: Cmd) -> bool {
        if self.repl.is_empty() {
            return true;
        }
        let i = c as usize;
        if self.quiet_cmd[i] == self.gen {
            return true;
        }
        if self.repl.contains_key(c.word()) {
            return false;
        }
        self.quiet_cmd[i] = self.gen;
        true
    }
    /// Guard for compiled fast path number `site`: none of its words or commands has a
    /// visible definition. A site whose names have no definitions at all is remembered.
    #[inline(always)]
    pub fn unreplaced(&mut self, site: usize, ws: &[(Sym, &str)], cs: &[Cmd]) -> bool {
        if self.guards {
            return false;
        }
        if self.repl.is_empty() || self.quiet_site.get(site) == Some(&self.gen) {
            return true;
        }
        self.unreplaced_slow(site, ws, cs)
    }
    #[inline(never)]
    fn unreplaced_slow(&mut self, site: usize, ws: &[(Sym, &str)], cs: &[Cmd]) -> bool {
        if ws.iter().all(|&(k, t)| self.quiet_sym(k, t)) && cs.iter().all(|&c| self.quiet_cmd(c)) {
            if self.quiet_site.len() <= site {
                self.quiet_site.resize(site + 1, 0);
            }
            self.quiet_site[site] = self.gen;
            return true;
        }
        for &(k, t) in ws {
            if !self.quiet_sym(k, t) && self.repl_active(t) {
                return false;
            }
        }
        for &c in cs {
            if !self.quiet_cmd(c) && self.repl_active(c.word()) {
                return false;
            }
        }
        true
    }
    /// Runs the visible definition of t: a value is pushed, code is pushed as a frame.
    pub fn try_expand(&mut self, t: &str) -> bool {
        let m = match self.visible(t) {
            Some(m) => m.clone(),
            None => return false,
        };
        match m.code {
            None => self.args.push(m.val),
            Some(Script::N(f)) => {
                let save = self.bound;
                self.bound = m.gen;
                f(self);
                self.bound = save;
            }
            Some(Script::C(f)) => self.fs.push(Frame { body: Body::Chunk(f), pc: 0, bound: m.gen }),
            Some(Script::U(u)) => self.fs.push(Frame { body: Body::Ops(u), pc: 0, bound: m.gen }),
        }
        true
    }
    #[inline]
    pub fn cmd_site(&mut self, c: Cmd) -> bool {
        if !self.quiet_cmd(c) && self.try_expand(c.word()) {
            return false;
        }
        self.cmd(c)
    }
    pub fn pop_to(&mut self, k: Sym) {
        let k = self.l(k);
        match self.args.pop() {
            Some(v) => self.mset(k, v),
            None => {
                let m = format!("{}@: stack is empty", self.name(k));
                self.fail(m)
            }
        }
    }

    fn need(&mut self, n: usize, c: Cmd) {
        if self.args.len() < n {
            let m = format!("{}: needs {} args, stack has {}", c.word(), n, self.args.len());
            self.fail(m)
        }
    }
    fn t(&self, i: usize) -> String {
        self.args[i].text().into_owned()
    }
    fn opt_t(&self, i: usize) -> Option<String> {
        self.args.get(i).map(|v| v.text().into_owned())
    }
    fn x(&self, i: usize) -> f64 {
        self.args[i].num()
    }
    pub fn arg_sym(&mut self, i: usize) -> Sym {
        let k = match &self.args[i] {
            Val::W(_, k) | Val::K(_, k) => *k,
            v => {
                let t = v.text();
                let t = t.into_owned();
                self.it.sym(&t)
            }
        };
        self.l(k)
    }
    fn dest(&mut self, i: usize, def: Sym) -> Sym {
        if self.args.len() > i { self.arg_sym(i) } else { def }
    }

    // variables

    #[inline(always)]
    pub fn nset(&mut self, s: Sym, x: f64) {
        if self.guards {
            self.guard_w(s);
        }
        self.note(s);
        let i = s.0 as usize;
        if i < self.nums.len() {
            self.nums[i] = Some(x);
        } else {
            *grow(&mut self.nums, i) = Some(x);
        }
    }
    pub fn mset(&mut self, s: Sym, v: Val) {
        if self.guards {
            self.guard_w(s);
        }
        self.note(s);
        *grow(&mut self.mem, s.0 as usize) = Some(v);
    }
    pub fn take_mem(&mut self, s: Sym) -> Option<String> {
        self.epoch += 1;
        self.mem.get_mut(s.0 as usize).and_then(|v| v.take()).map(|v| v.text().into_owned())
    }
    pub fn get_val(&mut self, s: Sym) -> Val {
        if self.guards {
            self.guard_r(s);
        }
        let i = s.0 as usize;
        if let Some(Some(v)) = self.mem.get(i) {
            return v.clone();
        }
        if let Some(Some(x)) = self.nums.get(i) {
            return Val::N(*x);
        }
        if s == S_Q {
            if self.fold {
                self.impure("?")
            }
            return Val::R(Rc::from(os_u64().to_string()));
        }
        Val::L("404")
    }
    /// A value that dispatch would push unchanged.
    pub fn inert(&self, v: &Val) -> bool {
        match v {
            Val::N(x) => !self.repl_num || !self.repl_active(&fmt_num(*x)),
            Val::W(t, k) => !self.repl_active(t) && (self.g.is_none() || self.it.host[k.0 as usize] == 0),
            Val::K(t, k) => !self.repl_active(t) && (self.g.is_none() || self.it.host[k.0 as usize] == 0),
            Val::L(t) => self.inert_text(t),
            Val::R(t) => self.inert_text(t),
        }
    }
    fn inert_text(&self, t: &str) -> bool {
        if t.is_empty() || t.chars().any(is_ws) {
            return true;
        }
        if self.g.is_some() && HOST_WORDS.contains(&t) {
            return false;
        }
        cmd_of(t).is_none() && !active_form(t) && !self.repl_active(t)
    }
    #[inline(always)]
    pub fn num_of(&mut self, k: Sym) -> Option<f64> {
        if self.guards {
            self.guard_r(k);
        }
        let i = k.0 as usize;
        match self.mem.get(i) {
            Some(Some(v)) => return if self.inert(v) { Some(v.num()) } else { None },
            _ => {}
        }
        if let Some(Some(x)) = self.nums.get(i) {
            let x = *x;
            return if !self.repl_num || !self.repl_active(&fmt_num(x)) { Some(x) } else { None };
        }
        let v = self.get_val(k);
        if self.inert(&v) { Some(v.num()) } else { None }
    }
    #[inline]
    pub fn val_of(&mut self, k: Sym) -> Option<Val> {
        let v = self.get_val(k);
        if self.inert(&v) { Some(v) } else { None }
    }
    #[inline]
    pub fn add_num(&mut self, k: Sym, d: f64) {
        let x = self.nums.get(k.0 as usize).copied().flatten().unwrap_or(0.0);
        self.nset(k, x + d);
    }
    pub fn interp(&mut self, s: &mut String, k: Sym) {
        let k = self.l(k);
        let v = self.get_val(k);
        s.push_str(&v.text());
    }
    #[inline]
    pub fn lookup(&mut self, k: Sym) -> bool {
        let k = self.l(k);
        if !self.guards && !self.repl_num {
            let i = k.0 as usize;
            if let (None | Some(None), Some(Some(x))) = (self.mem.get(i), self.nums.get(i)) {
                self.args.push(Val::N(*x));
                return false;
            }
        }
        let v = self.get_val(k);
        self.dispatch(v)
    }
    /// Runs a looked-up value as a word: a value that is one word is executed
    /// (command, suffix form, escape, replacement, host word); anything else is pushed.
    pub fn dispatch(&mut self, v: Val) -> bool {
        let t: Option<Rc<str>> = match &v {
            Val::N(x) => {
                if self.repl_num {
                    let t = fmt_num(*x);
                    if self.try_expand(&t) {
                        return false;
                    }
                }
                None
            }
            Val::W(t, k) => {
                let (t, k) = (*t, *k);
                if !self.quiet_sym(k, t) && self.try_expand(t) {
                    return false;
                }
                if self.g.is_some() && self.it.host[k.0 as usize] != 0 {
                    graph::host_word(self, self.it.host[k.0 as usize] - 1);
                    return false;
                }
                None
            }
            Val::K(t, k) => {
                let (t, k) = (t.clone(), *k);
                if !self.quiet_sym(k, &t) && self.try_expand(&t) {
                    return false;
                }
                if self.g.is_some() && self.it.host[k.0 as usize] != 0 {
                    graph::host_word(self, self.it.host[k.0 as usize] - 1);
                    return false;
                }
                None
            }
            Val::L(t) => Some(Rc::from(*t)),
            Val::R(t) => Some(t.clone()),
        };
        if let Some(t) = t {
            if !t.is_empty() && !t.chars().any(is_ws) {
                if let Some(c) = cmd_of(&t) {
                    return self.cmd_site(c);
                }
                if active_form(&t) || t.starts_with('⟨') {
                    let op = if t.starts_with('⟨') {
                        match parse(&t, &mut self.it) {
                            Ok(mut c) if c.ops.len() == 1 => c.ops.pop().unwrap(),
                            _ => {
                                self.args.push(v);
                                return false;
                            }
                        }
                    } else {
                        classify(&t, &mut self.it)
                    };
                    return matches!(self.step(&op, false), Some(Flow::Done));
                }
                if !self.repl.is_empty() && self.try_expand(&t) {
                    return false;
                }
                if self.g.is_some() {
                    if let Some(h) = HOST_WORDS.iter().position(|w| **w == *t) {
                        graph::host_word(self, h as u8);
                        return false;
                    }
                }
            }
        }
        self.args.push(v);
        false
    }
    fn joined(&self) -> String {
        let mut s = String::new();
        for (i, a) in self.args.iter().enumerate() {
            if i > 0 {
                s.push(' ');
            }
            s.push_str(&a.text());
        }
        s
    }
    pub fn line_text(&self) -> String {
        self.joined()
    }
    pub fn set_mem(&mut self, k: Sym) {
        let k = self.l(k);
        let v = if self.args.len() == 1 {
            self.args.pop().unwrap()
        } else {
            Val::of(self.joined())
        };
        self.args.clear();
        self.mset(k, v);
    }
    pub fn set_num(&mut self, k: Sym) {
        let k = self.l(k);
        let x = self.args.first().map(|v| v.num()).unwrap_or(0.0);
        self.args.clear();
        self.nset(k, x);
    }
    #[inline]
    pub fn inc(&mut self, k: Sym) {
        let k = self.l(k);
        let d = if self.args.is_empty() {
            1.0
        } else {
            let d = self.args[0].num();
            self.args.clear();
            d
        };
        let x = self.nums.get(k.0 as usize).copied().flatten().unwrap_or(0.0);
        self.nset(k, x + d);
    }
    #[inline]
    pub fn dec(&mut self, k: Sym) {
        let k = self.l(k);
        let d = if self.args.is_empty() {
            1.0
        } else {
            let d = self.args[0].num();
            self.args.clear();
            d
        };
        let x = self.nums.get(k.0 as usize).copied().flatten().unwrap_or(0.0);
        self.nset(k, x - d);
    }

    // scripts and units

    fn resolve(&mut self, body: &str, hint: Option<usize>) -> Result<Script, String> {
        let prog = self.prog;
        if let Some(h) = hint.filter(|&h| prog.chunks[h].0 == body) {
            return Ok(Script::C(prog.chunks[h].1));
        }
        if let Some(f) = prog.find(body) {
            return Ok(Script::C(f));
        }
        self.code_of(body).map(Script::U)
    }
    /// Parsed code for a text, cached; text the host registered carries its unit id.
    pub fn code_of(&mut self, src: &str) -> Result<Rc<Code>, String> {
        if let Some(u) = self.units.get(src) {
            return Ok(u.clone());
        }
        let mut c = parse(src, &mut self.it)?;
        c.unit = self.unit_ids.get(src).copied().unwrap_or(NO_UNIT);
        let u = Rc::new(c);
        if self.units.len() > 4096 {
            self.units.clear();
        }
        self.units.insert(src.to_string(), u.clone());
        Ok(u)
    }
    pub fn def_script(&mut self, k: Sym, hint: Option<usize>) {
        let k = self.l(k);
        if self.guards {
            self.guard_w(k);
        }
        let body = self.joined().replace('|', "~");
        self.args.clear();
        let sc = match self.resolve(&body, hint) {
            Ok(sc) => sc,
            Err(e) => {
                let m = format!("{}$: {}", self.name(k), e);
                self.fail(m)
            }
        };
        let b = self.gen;
        self.note(k);
        *grow(&mut self.scripts, k.0 as usize) = Some((sc, b));
    }
    /// The text of a script, when it has one.
    pub fn script_text(&self, k: Sym) -> Option<String> {
        match self.scripts.get(k.0 as usize).and_then(|x| x.as_ref()) {
            Some((Script::U(u), _)) => Some(u.src.to_string()),
            Some((Script::C(f), _)) => self.prog.chunks.iter().find(|c| c.1 as usize == *f as usize).map(|c| c.0.to_string()),
            _ => None,
        }
    }

    /// Definitions as (word, value text) in generation order, natives left out.
    pub fn defs_text(&self) -> Vec<(String, String)> {
        let mut v: Vec<(u64, String, String)> = Vec::new();
        for (k, ms) in &self.repl {
            for m in ms {
                if matches!(m.code, Some(Script::N(_))) {
                    continue;
                }
                v.push((m.gen, k.to_string(), m.val.text().into_owned()));
            }
        }
        v.sort();
        v.into_iter().map(|(_, k, t)| (k, t)).collect()
    }

    /// Replays a definition taken from another machine.
    pub fn add_def(&mut self, key: &str, val: &str) {
        let v = Val::of(val.to_string());
        let code = self.resolve(val, None).ok();
        if canon_num(key).is_some() {
            self.repl_num = true;
        }
        let gen = self.gen;
        self.gen += 1;
        self.note_repl(key);
        self.repl.entry(Rc::from(key)).or_default().push(Macro { val: v, code, gen });
    }

    /// Defines a script from text directly, bound at the current generation.
    pub fn def_script_text(&mut self, name: &str, body: &str) -> Result<(), String> {
        let k = self.it.sym(name);
        let sc = self.resolve(body, None)?;
        let b = self.gen;
        self.epoch += 1;
        *grow(&mut self.scripts, k.0 as usize) = Some((sc, b));
        Ok(())
    }

    /// The frame that calling script k runs, or None when it ran already (a native).
    fn frame_for(&mut self, k: Sym) -> Option<Frame> {
        if self.guards {
            self.guard_r(k);
        }
        let sc = self.scripts.get(k.0 as usize).and_then(|x| x.clone());
        match sc {
            Some((Script::N(f), _)) => {
                f(self);
                None
            }
            Some((Script::C(f), b)) => Some(Frame { body: Body::Chunk(f), pc: 0, bound: b }),
            Some((Script::U(u), b)) => Some(Frame { body: Body::Ops(u), pc: 0, bound: b }),
            None => {
                if self.g.is_some() {
                    if let Some(f) = graph::call_hook(self, k) {
                        return f;
                    }
                }
                let name = self.name(k);
                if self.prog.file(&name).is_some()
                    || std::path::Path::new(&code_path(&name)).is_file()
                    || self.prog.lib_src(&name).is_some()
                {
                    Some(self.file_frame(&name))
                } else {
                    self.fail(format!("{}.: no script or file named '{}'", name, name))
                }
            }
        }
    }
    pub fn call(&mut self, k: Sym) {
        if let Some(f) = self.frame_for(k) {
            self.fs.push(f);
        }
    }
    fn count(&mut self, i: usize, w: &str) -> u64 {
        let v = &self.args[i];
        let x = match v {
            Val::N(x) => Some(*x),
            _ => v.text().trim().parse::<f64>().ok(),
        };
        match x {
            Some(x) if x.is_finite() && x >= 0.0 && x.fract() == 0.0 => x as u64,
            _ => {
                let m = format!("{}: repeat count must be a whole number >= 0, got '{}'", w, v.text());
                self.fail(m)
            }
        }
    }
    fn repeat(&mut self, k: Sym) {
        let w = format!("{}.", self.name(k));
        let n = self.count(0, &w);
        self.fs.push(Frame { body: Body::Repeat { k, i: 0, n, amp: false }, pc: 0, bound: self.bound });
    }
    pub fn call_site(&mut self, k: Sym) {
        let k = self.l(k);
        if self.args.len() == 1 { self.repeat(k) } else { self.call(k) }
    }
    pub fn call_pass(&mut self, k: Sym) {
        let k = self.l(k);
        self.call(k)
    }
    /// A call as the last op: a repeat runs as a frame and ends the unit; else a tail call.
    pub fn tail_site(&mut self, k: Sym) -> Flow {
        let k = self.l(k);
        if self.args.len() == 1 {
            self.repeat(k);
            Flow::Yield(END)
        } else {
            Flow::Tail(k)
        }
    }
    /// A tail call from chunk `me` back into itself, done in place when nothing else needs the frame.
    #[inline]
    pub fn self_tail(&mut self, k: Sym, me: Chunk, base: usize) -> bool {
        if self.fuel <= 1 || self.args.len() == 1 || self.fs.len() != base || self.guards {
            return false;
        }
        let idx = base - 1;
        if !self.touched.get(idx).map_or(true, |t| t.is_empty()) {
            return false;
        }
        let b = match self.scripts.get(k.0 as usize) {
            Some(Some((Script::C(g), b))) if *g as usize == me as usize => *b,
            _ => return false,
        };
        self.fuel -= 1;
        let fr = &mut self.fs[idx];
        fr.bound = b;
        fr.pc = 0;
        self.bound = b;
        true
    }
    pub fn tail_pass(&mut self, k: Sym) -> Flow {
        Flow::Tail(self.l(k))
    }

    /// The frame that runs text as code; text made at run time sees every replacement.
    pub fn src_frame(&mut self, src: &str) -> Frame {
        if let Some(f) = self.prog.find(src) {
            return Frame { body: Body::Chunk(f), pc: 0, bound: UNBOUND };
        }
        match self.code_of(src) {
            Ok(u) => Frame { body: Body::Ops(u), pc: 0, bound: UNBOUND },
            Err(e) => self.fail(format!("parse: {}", e)),
        }
    }
    pub fn run_src(&mut self, src: &str) {
        let f = self.src_frame(src);
        self.fs.push(f);
    }
    pub fn file_frame(&mut self, path: &str) -> Frame {
        if let Some(f) = self.prog.file(path) {
            return Frame { body: Body::Chunk(f), pc: 0, bound: UNBOUND };
        }
        if let Some(t) = self.file_texts.as_ref().and_then(|m| m.get(path)).cloned() {
            return self.src_frame(&t);
        }
        if self.fold && self.prog.lib_src(path).is_none() {
            self.impure(&format!("the file {}", path))
        }
        match fs::read(code_path(path)) {
            Ok(b) => {
                let s = String::from_utf8_lossy(&b).into_owned();
                self.src_frame(&s)
            }
            Err(e) => match self.prog.lib_src(path) {
                Some(s) => self.src_frame(s),
                None => self.fail(format!("{}: {}", path, e)),
            },
        }
    }
    pub fn run_file(&mut self, path: &str) {
        let f = self.file_frame(path);
        self.fs.push(f);
    }

    // the machine

    /// Pushes a frame for `src` and runs until it and everything it started are done.
    pub fn run_text(&mut self, src: &str) -> Status {
        let r = self.guarded(|rt| rt.run_src(src));
        if let Some(st) = r {
            return st;
        }
        self.run(u64::MAX)
    }

    /// Runs f, turning an s error into a status.
    pub fn guarded<F: FnOnce(&mut Rt)>(&mut self, f: F) -> Option<Status> {
        match panic::catch_unwind(AssertUnwindSafe(|| f(self))) {
            Ok(()) => None,
            Err(p) => Some(self.status_of(p)),
        }
    }

    fn status_of(&mut self, p: Box<dyn std::any::Any + Send>) -> Status {
        let st = if let Some(e) = p.downcast_ref::<SErr>() {
            Status::Error(e.0.clone())
        } else if let Some(e) = p.downcast_ref::<SExit>() {
            Status::Exit(e.0)
        } else {
            panic::resume_unwind(p)
        };
        self.reset_frames();
        st
    }

    /// Drops every frame, as after an error.
    pub fn reset_frames(&mut self) {
        self.unwind_lanes(0);
        self.end_view();
        self.wait = None;
        self.brk_skip = None;
    }

    /// Runs at most `budget` ops (a compiled chunk counts as one per entry).
    pub fn run(&mut self, budget: u64) -> Status {
        self.wait = None;
        let mut left = budget;
        loop {
            let r = panic::catch_unwind(AssertUnwindSafe(|| self.run_inner(&mut left)));
            match r {
                Ok(st) => {
                    self.ops += budget - left;
                    return st;
                }
                Err(p) => {
                    let msg = p.downcast_ref::<SErr>().map(|e| e.0.clone());
                    if let Some(m) = msg {
                        if self.recover(&m) {
                            continue;
                        }
                    }
                    self.ops += budget - left;
                    return self.status_of(p);
                }
            }
        }
    }

    /// An error with a try below it: the frames above the try go, its handler runs.
    fn recover(&mut self, msg: &str) -> bool {
        if self.fold_hit.is_some() {
            return false;
        }
        let idx = match self.fs.iter().rposition(|f| matches!(f.body, Body::Catch { .. })) {
            Some(i) => i,
            None => return false,
        };
        self.unwind_lanes(idx + 1);
        let fr = self.fs.pop().unwrap();
        let d = self.fs.len();
        self.clear_locals(d);
        if let Body::Catch { h, frames } = fr.body {
            self.frames.truncate(frames);
            self.args.clear();
            self.wait = None;
            self.mset(S_ERR, Val::of(msg.to_string()));
            self.run_src(&h);
        }
        true
    }

    fn end_frame(&mut self, idx: usize) {
        self.clear_locals(idx);
        self.fs.remove(idx);
        if idx < self.touched.len() {
            self.touched.remove(idx);
        }
    }

    fn tail_into(&mut self, idx: usize, k: Sym) {
        self.clear_locals(idx);
        match self.frame_for(k) {
            Some(f) => {
                if self.fs.len() > idx + 1 {
                    // a native pushed frames before returning: they run, then the target
                    let rest: Vec<Frame> = self.fs.drain(idx + 1..).collect();
                    self.fs[idx] = f;
                    self.fs.extend(rest);
                } else {
                    self.fs[idx] = f;
                }
            }
            None => self.end_frame(idx),
        }
    }

    #[inline(always)]
    fn can_fast_tail(&self, idx: usize) -> bool {
        self.fs.len() == idx + 1 && !self.guards && self.touched.get(idx).map_or(true, |t| t.is_empty())
    }

    fn run_inner(&mut self, budget: &mut u64) -> Status {
        loop {
            let n = self.fs.len();
            if n == 0 {
                return Status::Done;
            }
            if *budget == 0 {
                return Status::Ran;
            }
            let idx = n - 1;
            self.bound = self.fs[idx].bound;
            let pc0 = self.fs[idx].pc;
            match &mut self.fs[idx].body {
                Body::Ops(code) => {
                    let mut code = code.clone();
                    let mut len = code.ops.len();
                    let mut pc = pc0 as usize;
                    enum Out {
                        End,
                        Pushed,
                        Tail(Sym),
                    }
                    loop {
                        let mut out = Out::End;
                        while pc < len {
                            if *budget == 0 {
                                self.fs[idx].pc = pc as u32;
                                return Status::Ran;
                            }
                            *budget -= 1;
                            self.site = (code.unit, pc as u32);
                            if self.fold && code.unit == 0 && idx == 0 {
                                self.fold_top = pc as u32;
                            }
                            if !self.breaks.is_empty() {
                                let key = ((code.unit as u64) << 32) | pc as u64;
                                if self.breaks.contains(&key) {
                                    if self.brk_skip == Some(key) {
                                        self.brk_skip = None;
                                    } else {
                                        self.brk_skip = Some(key);
                                        self.fs[idx].pc = pc as u32;
                                        return Status::Break;
                                    }
                                }
                            }
                            let op = &code.ops[pc];
                            pc += 1;
                            let f = self.step(op, pc == len);
                            if self.wait.is_some() {
                                // undo whatever frames the op pushed; it runs again from scratch
                                self.fs.truncate(idx + 1);
                                self.fs[idx].pc = (pc - 1) as u32;
                                return Status::Wait;
                            }
                            match f {
                                Some(Flow::Done) => {
                                    self.fs.truncate(idx + 1);
                                    out = Out::End;
                                    break;
                                }
                                Some(Flow::Tail(k)) => {
                                    out = Out::Tail(k);
                                    break;
                                }
                                _ => {}
                            }
                            if self.fs.len() != idx + 1 {
                                out = Out::Pushed;
                                break;
                            }
                        }
                        match out {
                            Out::End => self.end_frame(idx),
                            Out::Pushed => {
                                if pc == len {
                                    self.end_frame(idx);
                                } else {
                                    self.fs[idx].pc = pc as u32;
                                }
                            }
                            Out::Tail(k) => {
                                if self.can_fast_tail(idx) {
                                    if let Some(Some((Script::U(u), b))) = self.scripts.get(k.0 as usize) {
                                        let (u, b) = (u.clone(), *b);
                                        if !Rc::ptr_eq(&u, &code) {
                                            self.fs[idx].body = Body::Ops(u.clone());
                                        }
                                        self.fs[idx].bound = b;
                                        self.fs[idx].pc = 0;
                                        self.bound = b;
                                        code = u;
                                        len = code.ops.len();
                                        pc = 0;
                                        continue;
                                    }
                                }
                                self.tail_into(idx, k)
                            }
                        }
                        break;
                    }
                }
                Body::Chunk(f) => {
                    let mut f = *f;
                    let mut st = pc0;
                    loop {
                        *budget -= 1;
                        self.fuel = *budget;
                        let r = f(self, st);
                        *budget = self.fuel;
                        match r {
                            Flow::Done => {
                                self.fs.truncate(idx + 1);
                                self.end_frame(idx)
                            }
                            Flow::Tail(k) => {
                                if *budget > 0 && self.can_fast_tail(idx) {
                                    if let Some(Some((Script::C(g), b))) = self.scripts.get(k.0 as usize) {
                                        let (g, b) = (*g, *b);
                                        let fr = &mut self.fs[idx];
                                        if g as usize != f as usize {
                                            fr.body = Body::Chunk(g);
                                        }
                                        fr.bound = b;
                                        fr.pc = 0;
                                        self.bound = b;
                                        f = g;
                                        st = 0;
                                        continue;
                                    }
                                }
                                self.tail_into(idx, k)
                            }
                            Flow::Yield(END) => self.end_frame(idx),
                            Flow::Yield(s) => self.fs[idx].pc = s,
                            Flow::Wait(s) => {
                                self.fs.truncate(idx + 1);
                                self.fs[idx].pc = s;
                                return Status::Wait;
                            }
                        }
                        break;
                    }
                }
                Body::Repeat { k, i, n, amp } => {
                    let (k, cur, n, amp) = (*k, *i, *n, *amp);
                    *budget -= 1;
                    if cur < n {
                        if let Body::Repeat { i, .. } = &mut self.fs[idx].body {
                            *i += 1;
                        }
                        if amp {
                            self.nset(S_N, cur as f64);
                        }
                        self.args.clear();
                        self.call(k);
                    } else {
                        self.args.clear();
                        self.end_frame(idx);
                    }
                }
                Body::Await(w) => {
                    self.wait = Some(Wait::Host(*w));
                    return Status::Wait;
                }
                Body::Lanes(r) => {
                    let r = r.clone();
                    *budget -= 1;
                    self.lanes_step(idx, r);
                }
                Body::Each(r) => {
                    let r = r.clone();
                    *budget -= 1;
                    self.each_step(idx, r);
                }
                Body::Catch { .. } => {
                    *budget -= 1;
                    self.end_frame(idx);
                }
            }
        }
    }

    pub fn step(&mut self, op: &Op, tail: bool) -> Option<Flow> {
        match op {
            Op::Push(v) => self.args.push(v.clone()),
            Op::Interp(ps) => {
                let mut s = String::new();
                for p in ps {
                    match p {
                        Part::T(t) => s.push_str(t),
                        Part::V(k) => self.interp(&mut s, *k),
                    }
                }
                self.push_string(s);
            }
            Op::Word(t, k, n) => self.word_rc(t, *k, *n),
            Op::Cmd(c) => {
                if self.cmd_site(*c) {
                    return Some(Flow::Done);
                }
            }
            Op::Native(c) => {
                if self.cmd(*c) {
                    return Some(Flow::Done);
                }
            }
            Op::Pop(k) => self.pop_to(*k),
            Op::Lookup(k) => {
                if self.lookup(*k) {
                    return Some(Flow::Done);
                }
            }
            Op::Def(k) => self.def_script(*k, None),
            Op::SetMem(k) => self.set_mem(*k),
            Op::SetNum(k) => self.set_num(*k),
            Op::Inc(k) => self.inc(*k),
            Op::Dec(k) => self.dec(*k),
            Op::Call(k) => {
                if tail {
                    return match self.tail_site(*k) {
                        Flow::Tail(k) => Some(Flow::Tail(k)),
                        _ => None,
                    };
                }
                self.call_site(*k);
            }
            Op::CallPass(k) => {
                if tail {
                    return Some(self.tail_pass(*k));
                }
                self.call_pass(*k);
            }
            Op::Cond(t, inner) => {
                if self.ow == *t {
                    return self.step(inner, tail);
                }
            }
            Op::Lanes(f) => self.lanes_op(f.clone()),
        }
        None
    }

    fn on_err(&mut self, i: usize, msg: String) {
        if self.args.len() > i {
            let script = self.t(i);
            self.args.clear();
            self.mset(S_ERR, Val::of(msg));
            self.run_src(&script);
        } else {
            self.fail(msg)
        }
    }

    fn worker(&self, i: f64) -> Option<usize> {
        let k = i as i64;
        if k >= 0 && (k as usize) < self.workers.len() && self.workers[k as usize].is_some() {
            Some(k as usize)
        } else {
            None
        }
    }

    pub fn set_list(&mut self, k: Sym, l: List) {
        self.epoch += 1;
        *grow(&mut self.lists, k.0 as usize) = Some(l);
    }
    pub fn end_frame_pub(&mut self, idx: usize) {
        self.end_frame(idx)
    }
    pub fn clear_locals_pub(&mut self, d: usize) {
        self.clear_locals(d)
    }

    fn list(&mut self, k: Sym) -> &mut List {
        grow(&mut self.lists, k.0 as usize).get_or_insert_with(List::default)
    }

    fn arith(&mut self, c: Cmd) {
        self.need(2, c);
        let (a, b) = (self.x(0), self.x(1));
        let r = match c {
            Cmd::Add => a + b,
            Cmd::Sub => a - b,
            Cmd::Mul => a * b,
            Cmd::Div => a / b,
            Cmd::Pow => a.powf(b),
            Cmd::Mod => a.rem_euclid(b),
            _ => unreachable!(),
        };
        let d = self.dest(2, S_EQ);
        self.nset(d, r);
    }

    fn cmp(&mut self, c: Cmd) {
        self.need(2, c);
        let (a, b) = (&self.args[0], &self.args[1]);
        self.ow = match c {
            Cmd::Eq => val_eq(a, b),
            Cmd::Ne => !val_eq(a, b),
            Cmd::AndNe => self.ow && !val_eq(a, b),
            Cmd::AndEq => self.ow && val_eq(a, b),
            Cmd::NumEq => a.num() == b.num(),
            Cmd::Lt => a.num() < b.num(),
            Cmd::Gt => a.num() > b.num(),
            Cmd::Le => a.num() <= b.num(),
            Cmd::Ge => a.num() >= b.num(),
            _ => unreachable!(),
        };
    }

    fn seed_of(&self) -> Option<(u64, u64)> {
        match self.args.len() {
            0 => None,
            1 => Some((self.x(0) as i64 as u64, 0)),
            _ => Some((self.x(0) as i64 as u64, self.x(1) as i64 as u64)),
        }
    }

    /// Runs a file to the end in a fresh interpreter: the worker side of `t` and `***`.
    fn worker_run(prog: &'static Program, out: Out, path: &str) -> (Rt, Option<String>) {
        let mut w = Rt::with_out(prog, out);
        w.stdin = false;
        let st = match w.guarded(|w| w.run_file(path)) {
            Some(st) => st,
            None => w.run(u64::MAX),
        };
        w.flush();
        let err = match st {
            Status::Error(e) => Some(e),
            Status::Exit(c) => {
                if CLI.load(Ordering::Relaxed) {
                    std::process::exit(c)
                }
                None
            }
            _ => None,
        };
        (w, err)
    }

    pub fn cmd(&mut self, c: Cmd) -> bool {
        use Cmd::*;
        if self.fold && c.outside() {
            self.impure(c.word())
        }
        match c {
            Print => {
                let mut s = String::from("\n");
                for a in &self.args {
                    s.push(' ');
                    s.push_str(&a.text());
                }
                self.out.write_all(s.as_bytes());
            }
            PrintRaw => {
                let mut s = String::new();
                for a in &self.args {
                    s.push_str(&a.text());
                }
                self.out.write_all(s.as_bytes());
            }
            Shell => {
                self.need(1, c);
                let k = self.t(0);
                self.flush();
                match shell(&k) {
                    Ok(o) => {
                        let s = String::from_utf8_lossy(&o.stdout).into_owned();
                        self.mset(S_R, Val::of(s));
                        self.nset(S_RC, o.status.code().unwrap_or(-1) as f64);
                    }
                    Err(e) => self.fail(format!("<: {}: {}", k, e)),
                }
            }
            ReadIn => {
                let d = self.dest(0, S_UNDER);
                self.flush();
                let mut line = String::new();
                let n = match &mut self.input {
                    Some(q) => match q.pop_front() {
                        Some(l) => {
                            line = l;
                            1
                        }
                        None => 0,
                    },
                    None if self.stdin => io::stdin().lock().read_line(&mut line).unwrap_or(0),
                    None => 0,
                };
                while line.ends_with('\n') || line.ends_with('\r') {
                    line.pop();
                }
                self.mset(d, Val::of(line));
                self.mset(S_EOF, Val::N(if n == 0 { 1.0 } else { 0.0 }));
            }
            Write => {
                self.need(2, c);
                let (p, s) = (self.t(0), self.t(1));
                if let Err(e) = fs::write(&p, s) {
                    self.fail(format!("w: {}: {}", p, e))
                }
            }
            Append => {
                self.need(2, c);
                let (p, s) = (self.t(0), self.t(1));
                let r = OpenOptions::new().append(true).create(true).open(&p).and_then(|mut f| f.write_all(s.as_bytes()));
                if let Err(e) = r {
                    self.fail(format!("+w: {}: {}", p, e))
                }
            }
            Move => {
                self.need(2, c);
                let (a, b) = (self.t(0), self.t(1));
                if fs::rename(&a, &b).is_err() {
                    if let Err(e) = fs::copy(&a, &b).and_then(|_| fs::remove_file(&a)) {
                        self.fail(format!("mv: {} {}: {}", a, b, e))
                    }
                }
            }
            Read => {
                self.need(1, c);
                let p = self.t(0);
                let k = if self.args.len() > 1 { self.arg_sym(1) } else { self.arg_sym(0) };
                match fs::read(&p) {
                    Ok(b) => {
                        let s = String::from_utf8_lossy(&b).into_owned();
                        self.mset(k, Val::of(s));
                    }
                    Err(e) => self.fail(format!("r: {}: {}", p, e)),
                }
            }
            ReadAt => {
                self.need(4, c);
                let p = self.t(0);
                let (off, len) = (self.x(1) as u64, self.x(2) as usize);
                let d = self.arg_sym(3);
                let r = File::open(&p).and_then(|mut f| {
                    f.seek(SeekFrom::Start(off))?;
                    let mut b = Vec::with_capacity(len.min(1 << 20));
                    f.take(len as u64).read_to_end(&mut b)?;
                    Ok(b)
                });
                match r {
                    Ok(b) => {
                        let s = String::from_utf8_lossy(&b).into_owned();
                        self.mset(d, Val::of(s));
                    }
                    Err(e) => self.fail(format!("r@: {}: {}", p, e)),
                }
            }
            Match => {
                self.need(2, c);
                let (table, key, def) = (self.t(0), self.t(1), self.opt_t(2));
                self.args.clear();
                let mut ws = table.split_whitespace();
                let mut hit: Option<String> = None;
                while let (Some(k), Some(v)) = (ws.next(), ws.next()) {
                    if k == key {
                        hit = Some(v.to_string());
                        break;
                    }
                }
                if let Some(code) = hit.or(def) {
                    self.run_src(&code);
                }
                return false;
            }
            Repeat => {
                self.need(2, c);
                let n = self.count(0, "&&&");
                let k = self.arg_sym(1);
                self.fs.push(Frame { body: Body::Repeat { k, i: 0, n, amp: true }, pc: 0, bound: self.bound });
            }
            Par => {
                let paths: Vec<String> = self.args.iter().map(|a| a.text().into_owned()).collect();
                self.args.clear();
                self.flush();
                let prog = self.prog;
                let outs: Vec<Result<String, String>> = thread::scope(|sc| {
                    let hs: Vec<_> = paths
                        .iter()
                        .map(|p| {
                            let out = self.out.fork();
                            thread::Builder::new()
                                .stack_size(WORKER_STACK)
                                .spawn_scoped(sc, move || {
                                    let (mut w, err) = Rt::worker_run(prog, out, p);
                                    match err {
                                        Some(e) => Err(e),
                                        None => Ok(w.take_mem(S_OUT).unwrap_or_default()),
                                    }
                                })
                                .ok()
                        })
                        .collect();
                    hs.into_iter()
                        .map(|h| match h.and_then(|h| h.join().ok()) {
                            Some(r) => r,
                            None => Err("***: worker failed".to_string()),
                        })
                        .collect()
                });
                let mut codes = Vec::new();
                for o in outs {
                    match o {
                        Ok(o) => codes.push(o),
                        Err(e) => self.fail(e),
                    }
                }
                for o in codes.iter().rev() {
                    let f = self.src_frame(o);
                    self.fs.push(f);
                }
                return false;
            }
            SetN => {
                self.need(2, c);
                let x = self.x(0);
                let k = self.arg_sym(1);
                self.nset(k, x);
            }
            Add | Sub | Mul | Div | Pow | Mod => self.arith(c),
            Not => self.ow = !self.ow,
            Eq | Ne | AndNe | AndEq | NumEq | Lt | Gt | Le | Ge => self.cmp(c),
            LPush => {
                // every value before the list name is an entry, appended in order
                self.need(2, c);
                let n = self.args.len();
                let k = self.arg_sym(n - 1);
                self.guard_w(k);
                self.note(k);
                let vs: Vec<Val> = self.args[..n - 1].to_vec();
                let l = self.list(k);
                for v in vs {
                    l.push(v);
                }
            }
            LPop => {
                self.need(1, c);
                let k = self.arg_sym(0);
                self.guard_w(k);
                self.note(k);
                let v = self.list(k).pop().unwrap_or(Val::L("404"));
                self.mset(S_DOLLAR, v);
            }
            LGet => {
                self.need(3, c);
                let k = self.arg_sym(0);
                self.guard_r(k);
                let i = self.x(1) as i64;
                let d = self.arg_sym(2);
                let l = self.list(k);
                let i = if i < 0 { i + l.len() as i64 } else { i };
                if i >= 0 && (i as usize) < l.len() {
                    let v = l.get(i as usize).unwrap();
                    self.mset(d, v);
                }
            }
            LSet => {
                self.need(3, c);
                let v = self.args[0].clone();
                let k = self.arg_sym(1);
                self.guard_w(k);
                self.note(k);
                let i = self.x(2) as i64;
                let l = self.list(k);
                let i = if i < 0 { i + l.len() as i64 } else { i };
                if i >= 0 && (i as usize) < l.len() {
                    l.set(i as usize, v);
                }
            }
            LDel => {
                self.need(1, c);
                let k = self.arg_sym(0);
                self.guard_w(k);
                self.note(k);
                if let Some(l) = self.lists.get_mut(k.0 as usize) {
                    *l = None;
                }
            }
            LLen => {
                self.need(1, c);
                let k = self.arg_sym(0);
                self.guard_r(k);
                let n = self.lists.get(k.0 as usize).and_then(|l| l.as_ref()).map(|l| l.len()).unwrap_or(0);
                let d = self.dest(1, S_EQ);
                self.nset(d, n as f64);
            }
            Split => {
                self.need(2, c);
                let text = self.t(0);
                let k = self.arg_sym(1);
                self.guard_w(k);
                let parts: Vec<Val> = match self.opt_t(2) {
                    None => text.split_whitespace().map(|s| Val::of(s.to_string())).collect(),
                    Some(sep) if sep.is_empty() => text.chars().map(|ch| Val::of(ch.to_string())).collect(),
                    Some(sep) => text.split(sep.as_str()).map(|s| Val::of(s.to_string())).collect(),
                };
                self.note(k);
                *grow(&mut self.lists, k.0 as usize) = Some(List::from_vals(parts));
            }
            Join => {
                self.need(2, c);
                let k = self.arg_sym(0);
                self.guard_r(k);
                let d = self.arg_sym(1);
                let sep = self.opt_t(2).unwrap_or_else(|| " ".to_string());
                let s = match self.lists.get(k.0 as usize).and_then(|l| l.as_ref()) {
                    Some(l) => l.texts().join(&sep),
                    None => String::new(),
                };
                self.mset(d, Val::of(s));
            }
            SLen => {
                self.need(1, c);
                let n = self.args[0].text().chars().count();
                let d = self.dest(1, S_EQ);
                self.nset(d, n as f64);
            }
            Open => {
                self.need(3, c);
                let (p, mode) = (self.t(0), self.t(2));
                let k = self.arg_sym(1);
                match open_mode(&p, &mode) {
                    Ok(f) => {
                        self.files.insert(k.0, FileH::new(Handle::File(f)));
                    }
                    Err(e) => {
                        self.on_err(3, format!("f: {}: {}", p, e));
                        return false;
                    }
                }
            }
            FWriteAt => {
                self.need(3, c);
                let k = self.arg_sym(0);
                let off = self.x(1) as u64;
                let key = self.arg_sym(2);
                let data = match self.mem.get(key.0 as usize) {
                    Some(Some(v)) => v.text().into_owned(),
                    _ => String::new(),
                };
                let r = match self.files.get_mut(&k.0) {
                    Some(h) => h.write_at(off, data.as_bytes()),
                    None => Err(io::Error::new(io::ErrorKind::NotFound, "no open file")),
                };
                if let Err(e) = r {
                    let m = format!("+f: {}: {}", self.name(k), e);
                    self.on_err(3, m);
                    return false;
                }
            }
            FReadAt => {
                self.need(4, c);
                let k = self.arg_sym(0);
                let (off, len) = (self.x(1) as u64, self.x(2) as usize);
                let d = self.arg_sym(3);
                let r = match self.files.get_mut(&k.0) {
                    Some(h) => h.read_at(off, len),
                    None => Err(io::Error::new(io::ErrorKind::NotFound, "no open file")),
                };
                match r {
                    Ok(b) => {
                        let s = String::from_utf8_lossy(&b).into_owned();
                        self.mset(d, Val::of(s));
                    }
                    Err(e) => {
                        let m = format!("@f: {}: {}", self.name(k), e);
                        self.fail(m)
                    }
                }
            }
            Tcp => {
                self.need(3, c);
                let (addr, mode) = (self.t(0), self.t(2));
                let k = self.arg_sym(1);
                let r: io::Result<Handle> = match mode.as_str() {
                    "c" => TcpStream::connect(addr.as_str()).map(|s| Handle::Net(BufReader::new(s))),
                    "s" => TcpListener::bind(addr.as_str()).map(Handle::Listen),
                    "a" => {
                        let lk = self.arg_sym(0);
                        match self.files.get(&lk.0).map(|h| &h.h) {
                            Some(Handle::Listen(l)) => l
                                .accept()
                                .map(|(s, peer)| (Handle::Net(BufReader::new(s)), peer))
                                .map(|(h, peer)| {
                                    self.peer = peer.to_string();
                                    h
                                }),
                            _ => Err(io::Error::new(io::ErrorKind::NotFound, "no listener with that name")),
                        }
                    }
                    _ => Err(io::Error::new(io::ErrorKind::InvalidInput, format!("bad mode '{}', use c, s or a", mode))),
                };
                match r {
                    Ok(h) => {
                        let port = match &h {
                            Handle::Listen(l) => l.local_addr().map(|a| a.port()).ok(),
                            Handle::Net(n) => n.get_ref().local_addr().map(|a| a.port()).ok(),
                            _ => None,
                        };
                        if mode == "a" {
                            let peer = std::mem::take(&mut self.peer);
                            let ps = self.it.sym("peer");
                            self.mset(ps, Val::of(peer));
                        }
                        if let (Some(p), "s") = (port, mode.as_str()) {
                            let ps = self.it.sym("port");
                            self.nset(ps, p as f64);
                        }
                        self.files.insert(k.0, FileH::new(h));
                    }
                    Err(e) => {
                        self.on_err(3, format!("tcp: {}: {}", addr, e));
                        return false;
                    }
                }
            }
            FClose => {
                self.need(1, c);
                let k = self.arg_sym(0);
                self.files.remove(&k.0);
            }
            FWriteLn => {
                self.need(2, c);
                let k = self.arg_sym(0);
                let mut line = self.t(1);
                line.push('\n');
                let r = match self.files.get_mut(&k.0) {
                    Some(h) => h.write(line.as_bytes()),
                    None => Err(io::Error::new(io::ErrorKind::NotFound, "no open file")),
                };
                if let Err(e) = r {
                    let m = format!("f_: {}: {}", self.name(k), e);
                    self.on_err(2, m);
                    return false;
                }
            }
            FReadLn => {
                self.need(2, c);
                let k = self.arg_sym(0);
                let d = self.arg_sym(1);
                let r = match self.files.get_mut(&k.0) {
                    Some(h) => h.read_line(),
                    None => Err(io::Error::new(io::ErrorKind::NotFound, "no open file")),
                };
                match r {
                    Ok(s) => self.mset(d, Val::of(s)),
                    Err(e) => {
                        let m = format!("_f: {}: {}", self.name(k), e);
                        self.fail(m)
                    }
                }
            }
            Eof => {
                self.need(1, c);
                let k = self.arg_sym(0);
                let e = self.files.get(&k.0).map(|h| h.eof).unwrap_or(true);
                self.mset(S_EOF, Val::N(if e { 1.0 } else { 0.0 }));
            }
            Spawn => {
                let paths: Vec<String> = self.args.iter().map(|a| a.text().into_owned()).collect();
                for p in paths {
                    let (to_tx, to_rx) = channel::<String>();
                    let (from_tx, from_rx) = channel::<String>();
                    let prog = self.prog;
                    let out = self.out.fork();
                    let r = thread::Builder::new().stack_size(WORKER_STACK).spawn(move || {
                        let (mut w, err) = Rt::worker_run(prog, out, &p);
                        if let Some(e) = err {
                            let _ = from_tx.send(format!("\u{0}{}", e));
                            return;
                        }
                        while let Ok(m) = to_rx.recv() {
                            if m == "__stop__" {
                                break;
                            }
                            let st = w.run_text(&m);
                            w.flush();
                            match st {
                                Status::Error(e) => {
                                    let _ = from_tx.send(format!("\u{0}{}", e));
                                    return;
                                }
                                Status::Exit(c) => {
                                    if CLI.load(Ordering::Relaxed) {
                                        std::process::exit(c)
                                    }
                                    return;
                                }
                                _ => {}
                            }
                            if let Some(o) = w.take_mem(S_OUT) {
                                if from_tx.send(o).is_err() {
                                    break;
                                }
                            }
                        }
                        w.flush();
                    });
                    if let Err(e) = r {
                        self.fail(format!("t: {}", e))
                    }
                    self.workers.push(Some(Worker { tx: to_tx, rx: from_rx }));
                    let k = (self.workers.len() - 1) as f64;
                    self.nset(S_THREADS, k);
                }
            }
            Kill => {
                self.need(1, c);
                match self.worker(self.x(0)) {
                    Some(k) => self.workers[k] = None,
                    None => self.mset(S_ERR, Val::L("Invalid thread index")),
                }
            }
            Send => {
                self.need(2, c);
                let m = self.t(0);
                match self.worker(self.x(1)) {
                    Some(k) => {
                        if self.workers[k].as_ref().unwrap().tx.send(m).is_err() {
                            self.mset(S_ERR, Val::L("thread closed"));
                        }
                    }
                    None => self.mset(S_ERR, Val::L("Invalid thread index")),
                }
            }
            Recv => {
                self.need(1, c);
                let i = self.x(0);
                match self.worker(i) {
                    Some(k) => {
                        self.flush();
                        let m = if self.nonblock {
                            match self.workers[k].as_ref().unwrap().rx.try_recv() {
                                Ok(m) => Ok(m),
                                Err(TryRecvError::Empty) => {
                                    self.wait = Some(Wait::Recv(k));
                                    return false;
                                }
                                Err(TryRecvError::Disconnected) => Err(()),
                            }
                        } else {
                            self.workers[k].as_ref().unwrap().rx.recv().map_err(|_| ())
                        };
                        self.args.clear();
                        match m {
                            Ok(m) => {
                                if let Some(e) = m.strip_prefix('\u{0}') {
                                    self.fail(e.to_string())
                                }
                                self.run_src(&m)
                            }
                            Err(_) => self.mset(S_ERR, Val::L("thread closed")),
                        }
                    }
                    None => {
                        self.args.clear();
                        self.mset(S_ERR, Val::L("Invalid thread index"))
                    }
                }
                return false;
            }
            PrngN => {
                if let Some((a, b)) = self.seed_of() {
                    self.rng = Rng::seeded(a, b);
                }
                let d = self.dest(2, S_RN);
                let x = self.rng.f64();
                self.nset(d, x);
            }
            PrngS => {
                const AZ: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
                if let Some((a, b)) = self.seed_of() {
                    self.rng = Rng::seeded(a, b);
                }
                let d = self.dest(2, S_RN);
                let s: String = (0..32).map(|_| AZ[(self.rng.next() % 52) as usize] as char).collect();
                self.mset(d, Val::of(s));
            }
            Replace => {
                self.need(2, c);
                let key = self.t(1);
                let v = self.args[0].clone();
                let code = self.resolve(&v.text(), None).ok();
                if canon_num(&key).is_some() {
                    self.repl_num = true;
                }
                let gen = self.gen;
                self.gen += 1;
                self.note_repl(&key);
                self.repl.entry(Rc::from(key)).or_default().push(Macro { val: v, code, gen });
            }
            Open1 => {
                self.need(1, c);
                let n = self.args.len() - 1;
                let k = self.count(n, "(") as usize;
                self.args.pop();
                let keep = k.min(n);
                let rest: Vec<Val> = self.args.drain(..n - keep).collect();
                self.frames.push(rest);
                return false;
            }
            Close1 => {
                let mut saved = match self.frames.pop() {
                    Some(f) => f,
                    None => self.fail(String::from(")") + ": no open frame"),
                };
                saved.append(&mut self.args);
                self.args = saved;
                return false;
            }
            Eval => {
                let v = match self.args.pop() {
                    Some(v) => v,
                    None => self.fail("~: stack is empty".to_string()),
                };
                let t = v.text().into_owned();
                self.run_src(&t);
                return false;
            }
            Depth => {
                let n = self.args.len() as f64;
                self.args.push(Val::N(n));
                return false;
            }
            Comment => {}
            Ret => return true,
            Exit => {
                let code = self.args.first().map(|v| v.num() as i32).unwrap_or(0);
                self.flush();
                panic::panic_any(SExit(code))
            }
        }
        self.args.clear();
        false
    }
}

pub enum Entry {
    Chunk(usize),
    File(String),
    /// The state a build folded the program to, then chunk 0: the rest of the program.
    Folded(&'static Fold),
}

/// A value in a folded state.
pub enum FV {
    N(f64),
    T(&'static str),
    W(&'static str),
}
/// A list in a folded state: numbers, or values.
pub enum FL {
    N(&'static [f64]),
    V(&'static [FV]),
}
/// The code of a replacement layer: none (a value), a text, or the machine's own word.
pub enum FC {
    None,
    Src(&'static str),
    Native,
}
pub struct FM {
    pub val: FV,
    pub code: FC,
    pub gen: u64,
}
/// What a program did before it first reached outside the machine, worked out at build time:
/// what it printed, and the names, lists, scripts and replacements it left. `end` holds how
/// the program ended when it never reached outside: an exit code or an error.
pub struct Fold {
    pub out: &'static str,
    pub end: Option<Result<i32, &'static str>>,
    pub mem: &'static [(&'static str, FV)],
    pub nums: &'static [(&'static str, f64)],
    pub lists: &'static [(&'static str, FL)],
    pub scripts: &'static [(&'static str, &'static str, u64)],
    pub repl: &'static [(&'static str, &'static [FM])],
    pub gen: u64,
    pub ow: bool,
    pub repl_num: bool,
    pub frames: &'static [&'static [FV]],
    pub args: &'static [FV],
}

impl Rt {
    fn fold_val(&mut self, v: &FV) -> Val {
        match v {
            FV::N(x) => Val::N(*x),
            FV::T(t) => Val::L(t),
            FV::W(w) => {
                let k = self.it.sym(w);
                Val::W(w, k)
            }
        }
    }
    /// The machine as the build left it: the printed text written, the names in place.
    pub fn restore_fold(&mut self, f: &Fold) {
        self.out.write_all(f.out.as_bytes());
        for (n, v) in f.mem {
            let k = self.it.sym(n);
            let v = self.fold_val(v);
            *grow(&mut self.mem, k.0 as usize) = Some(v);
        }
        for (n, x) in f.nums {
            let k = self.it.sym(n);
            *grow(&mut self.nums, k.0 as usize) = Some(*x);
        }
        for (n, l) in f.lists {
            let k = self.it.sym(n);
            let l = match l {
                FL::N(xs) => List::nums_of(xs.to_vec()),
                FL::V(vs) => {
                    let vs: Vec<Val> = vs.iter().map(|v| self.fold_val(v)).collect();
                    List::from_vals(vs)
                }
            };
            *grow(&mut self.lists, k.0 as usize) = Some(l);
        }
        for (n, src, gen) in f.scripts {
            let k = self.it.sym(n);
            match self.resolve(src, None) {
                Ok(sc) => *grow(&mut self.scripts, k.0 as usize) = Some((sc, *gen)),
                Err(e) => self.fail(format!("{}$: {}", n, e)),
            }
        }
        for (key, layers) in f.repl {
            let natives: Vec<Script> = self.repl.get(*key).map(|ls| ls.iter().filter_map(|m| match &m.code { Some(Script::N(f)) => Some(Script::N(*f)), _ => None }).collect()).unwrap_or_default();
            let mut nat = natives.into_iter();
            let mut out = Vec::new();
            for m in layers.iter() {
                let val = self.fold_val(&m.val);
                let code = match m.code {
                    FC::None => None,
                    FC::Src(s) => match self.resolve(s, None) {
                        Ok(sc) => Some(sc),
                        Err(e) => self.fail(format!("{}: {}", key, e)),
                    },
                    FC::Native => nat.next(),
                };
                out.push(Macro { val, code, gen: m.gen });
            }
            self.repl.insert(Rc::from(*key), out);
        }
        self.gen = f.gen;
        self.ow = f.ow;
        self.repl_num = f.repl_num;
        self.frames = f.frames.iter().map(|fr| fr.iter().map(|v| self.fold_val(v)).collect::<Vec<Val>>()).collect();
        self.args = f.args.iter().map(|v| self.fold_val(v)).collect();
        self.epoch += 1;
    }
}

/// Runs a program to the end on a thread with a large stack and exits with its code.
pub fn s_main(prog: &'static Program, entry: Entry, argv: Vec<String>) -> ! {
    CLI.store(true, Ordering::Relaxed);
    let h = thread::Builder::new()
        .stack_size(MAIN_STACK)
        .spawn(move || {
            let mut rt = Rt::new(prog);
            rt.set_argv(argv);
            let report = std::env::var("S_FORKS").is_ok();
            if report {
                rt.fork_log = Some(Vec::new());
            }
            let mut ended = None;
            let st = rt.guarded(|rt| match entry {
                Entry::Chunk(i) => rt.fs.push(Frame { body: Body::Chunk(prog.chunks[i].1), pc: 0, bound: UNBOUND }),
                Entry::Folded(f) => {
                    rt.restore_fold(f);
                    match f.end {
                        Some(e) => ended = Some(e),
                        None => rt.fs.push(Frame { body: Body::Chunk(prog.chunks[0].1), pc: 0, bound: UNBOUND }),
                    }
                }
                Entry::File(p) => {
                    if let Some(d) = std::path::Path::new(&p).parent() {
                        let _ = BASE.set(d.to_path_buf());
                    }
                    rt.run_file(&p)
                }
            });
            let st = match (st, ended) {
                (Some(st), _) => st,
                (None, Some(Ok(c))) => Status::Exit(c),
                (None, Some(Err(e))) => Status::Error(e.to_string()),
                (None, None) => rt.run(u64::MAX),
            };
            rt.flush();
            if let Some(l) = &rt.fork_log {
                for f in l {
                    let unit = prog.chunks.iter().position(|c| *c.0 == *f.src).map(|i| i as i64).unwrap_or(-1);
                    // lanes split by \x1e, items by \x1f
                    let clean = |x: &str| x.replace(['\x1e', '\x1f', '\n', '\t'], " ");
                    let lines: Vec<String> = f.lines.iter().map(|l| l.iter().map(|v| clean(v)).collect::<Vec<_>>().join("\x1f")).collect();
                    eprintln!("s-fork\t{}\t{}\t{}\t{}\t{}", unit, f.at, f.how, f.n, lines.join("\x1e"));
                }
            }
            match st {
                Status::Error(e) => {
                    eprintln!("\ns: {}", e);
                    1
                }
                Status::Exit(c) => c,
                _ => 0,
            }
        })
        .expect("spawn main thread");
    let code = h.join().unwrap_or(101);
    std::process::exit(code)
}
