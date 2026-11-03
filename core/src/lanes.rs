// Bead lines in s (BEADS.md §2, §3): reading forks out of text, running lanes, the words on
// joined lanes, try, and the two ways a fork runs faster than lane by lane: a fork over
// numbers whose shared beads are arithmetic runs as a vector program over columns with SIMD,
// and a fork whose lane beads hold code runs its lanes on threads.
#![allow(dead_code, unused_imports, unused_variables, unused_mut, clippy::all)]

use crate::rt::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

// ---------- reading ----------

const FORK: u8 = 0;
const LANE: u8 = 1;
const MERGE: u8 = 2;
const JOIN: u8 = 3;

fn mark_of(c: char) -> Option<u8> {
    match c {
        '⟪' => Some(FORK),
        '¦' => Some(LANE),
        '⟫' => Some(MERGE),
        '⨝' => Some(JOIN),
        _ => None,
    }
}
fn ascii_mark(w: &str) -> Option<(u8, String)> {
    match w {
        "[[" => Some((FORK, String::new())),
        "||" => Some((LANE, String::new())),
        "]]" => Some((MERGE, String::new())),
        "||>" => Some((JOIN, String::new())),
        _ => {
            let l = w.strip_prefix("[[")?.strip_suffix(':')?;
            if is_label(l) { Some((FORK, l.to_string())) } else { None }
        }
    }
}
fn is_label(l: &str) -> bool {
    let mut cs = l.chars();
    matches!(cs.next(), Some(c) if c.is_ascii_alphabetic() || c == '_') && cs.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[derive(Clone, Debug)]
struct BTok {
    m: Option<u8>,
    label: String,
    text: String,
    at: u32,
    line: u32,
}

/// s tokens with the bead marks split out. A brace or a quote is one token, so marks inside
/// them belong to the text they will be when they run.
fn bead_tokens(cs: &[char]) -> Vec<BTok> {
    let n = cs.len();
    let mut out = Vec::new();
    let mut line = 0u32;
    let mut i = 0;
    while i < n {
        if is_ws(cs[i]) {
            if cs[i] == '\n' {
                line += 1;
            }
            i += 1;
            continue;
        }
        if let Some(m) = mark_of(cs[i]) {
            let at = i as u32;
            let mut label = String::new();
            let mut text = cs[i].to_string();
            i += 1;
            if m == FORK {
                let mut j = i;
                while j < n && (cs[j].is_ascii_alphanumeric() || cs[j] == '_' || cs[j] == '-') {
                    j += 1;
                }
                if j > i && j + 1 < n && cs[j] == ':' && !is_ws(cs[j + 1]) && mark_of(cs[j + 1]).is_none() {
                    let l: String = cs[i..j].iter().collect();
                    if is_label(&l) {
                        text.push_str(&l);
                        text.push(':');
                        label = l;
                        i = j + 1;
                    }
                }
            }
            out.push(BTok { m: Some(m), label, text, at, line });
            continue;
        }
        let start = i;
        if cs[i] == '⟨' {
            while i < n && !is_ws(cs[i]) {
                i += 1;
            }
        } else {
            while i < n && !is_ws(cs[i]) && mark_of(cs[i]).is_none() {
                if cs[i] == '{' {
                    let mut d = 0i32;
                    while i < n {
                        if cs[i] == '{' {
                            d += 1;
                        } else if cs[i] == '}' {
                            d -= 1;
                            if d == 0 {
                                i += 1;
                                break;
                            }
                        }
                        if cs[i] == '\n' {
                            line += 1;
                        }
                        i += 1;
                    }
                    break;
                }
                if cs[i] == '\'' {
                    i += 1;
                    while i < n && cs[i] != '\'' {
                        if cs[i] == '\n' {
                            line += 1;
                        }
                        i += 1;
                    }
                    i = (i + 1).min(n);
                    break;
                }
                i += 1;
            }
        }
        let text: String = cs[start..i].iter().collect();
        let tline = line;
        match ascii_mark(&text) {
            Some((m, label)) => out.push(BTok { m: Some(m), label, text, at: start as u32, line: tline }),
            None => out.push(BTok { m: None, label: String::new(), text, at: start as u32, line: tline }),
        }
    }
    out
}

/// Whether text has a bead mark outside its braces and quotes.
pub fn has_marks(src: &str) -> bool {
    if !src.contains(['⟪', '¦', '⟫', '⨝']) && !src.contains("[[") && !src.contains("||") && !src.contains("]]") {
        return false;
    }
    let cs: Vec<char> = src.chars().collect();
    bead_tokens(&cs).iter().any(|t| t.m.is_some())
}

#[derive(Clone, Debug)]
enum It {
    T(usize),
    F(ForkNode),
    R(RegionNode),
}
#[derive(Clone, Debug)]
struct ForkNode {
    lanes: Vec<Vec<It>>,
    label: String,
    at: u32,
}
#[derive(Clone, Debug)]
struct RegionNode {
    items: Vec<It>,
    at: u32,
    from: usize,
    to: usize,
}

struct Tree<'a> {
    t: &'a [BTok],
    p: usize,
}

fn mark_char(m: u8) -> char {
    ['⟪', '¦', '⟫', '⨝'][m as usize]
}

impl<'a> Tree<'a> {
    fn top(&mut self) -> Result<Vec<It>, String> {
        let mut out = Vec::new();
        while self.p < self.t.len() {
            let tk = &self.t[self.p];
            match tk.m {
                None => {
                    out.push(It::T(self.p));
                    self.p += 1;
                }
                Some(FORK) => out.push(It::R(self.region()?)),
                Some(m) => return Err(format!("at {}: {} outside a fork", tk.at, mark_char(m))),
            }
        }
        Ok(out)
    }
    fn region(&mut self) -> Result<RegionNode, String> {
        let from = self.p;
        let at = self.t[self.p].at;
        let mut line = self.t[self.p].line;
        let mut items = Vec::new();
        while self.p < self.t.len() {
            let tk = &self.t[self.p];
            match tk.m {
                Some(FORK) => {
                    let (f, end_line) = self.fork()?;
                    items.push(It::F(f));
                    line = end_line;
                }
                Some(JOIN) => {
                    self.p += 1;
                    break;
                }
                Some(m) => return Err(format!("at {}: {} outside a fork", tk.at, mark_char(m))),
                None => {
                    if tk.line != line {
                        break;
                    }
                    items.push(It::T(self.p));
                    self.p += 1;
                }
            }
        }
        Ok(RegionNode { items, at, from, to: self.p })
    }
    fn fork(&mut self) -> Result<(ForkNode, u32), String> {
        let open = self.p;
        let (at, label) = (self.t[open].at, self.t[open].label.clone());
        self.p += 1;
        let mut lanes: Vec<Vec<It>> = vec![Vec::new()];
        let mut first_open: Option<(usize, usize)> = None; // (lane index position, token index)
        loop {
            if self.p >= self.t.len() {
                return Err(format!("at {}: ⟪ without ⟫", at));
            }
            let tk = &self.t[self.p];
            match tk.m {
                Some(MERGE) => {
                    let l = tk.line;
                    self.p += 1;
                    return Ok((ForkNode { lanes, label, at }, l));
                }
                Some(LANE) => {
                    self.p += 1;
                    lanes.push(Vec::new());
                    first_open = None;
                }
                Some(FORK) => {
                    let tok = self.p;
                    let pos = lanes.last().unwrap().len();
                    if first_open.is_none() {
                        first_open = Some((pos, tok));
                    }
                    let (f, _) = self.fork()?;
                    lanes.last_mut().unwrap().push(It::F(f));
                }
                Some(JOIN) => {
                    let (pos, tok) = match first_open {
                        Some(x) => x,
                        None => return Err(format!("at {}: ⨝ with no open fork", tk.at)),
                    };
                    self.p += 1;
                    let lane = lanes.last_mut().unwrap();
                    let items: Vec<It> = lane.drain(pos..).collect();
                    let rat = self.t[tok].at;
                    lane.push(It::R(RegionNode { items, at: rat, from: tok, to: self.p }));
                    first_open = None;
                }
                None => {
                    lanes.last_mut().unwrap().push(It::T(self.p));
                    self.p += 1;
                }
                Some(_) => self.p += 1,
            }
        }
    }
}

/// The lanes of a region (§2.4, §2.7): forks in one set zip, sets multiply in the order
/// they first appear, and a lane's own unjoined forks multiply within the lane.
fn mint<'a>(items: &'a [It]) -> Result<Vec<Vec<&'a It>>, String> {
    let mut sets: Vec<(String, usize)> = Vec::new();
    for it in items {
        if let It::F(f) = it {
            let k = f.lanes.len();
            match sets.iter_mut().find(|s| s.0 == f.label) {
                None => sets.push((f.label.clone(), k)),
                Some(s) => {
                    if s.1 == 1 {
                        s.1 = k;
                    } else if k != 1 && k != s.1 {
                        return Err(format!("at {}: this ⟪ has {} lanes where its set has {}", f.at, k, s.1));
                    }
                }
            }
        }
    }
    let mut tuples: Vec<Vec<usize>> = vec![Vec::new()];
    for (_, k) in &sets {
        let mut next = Vec::new();
        for t in &tuples {
            for i in 0..*k {
                let mut u = t.clone();
                u.push(i);
                next.push(u);
            }
        }
        tuples = next;
    }
    let mut out = Vec::new();
    for tup in &tuples {
        let mut vs: Vec<Vec<&It>> = vec![Vec::new()];
        for it in items {
            match it {
                It::F(f) => {
                    let lane = if f.lanes.len() == 1 { &f.lanes[0] } else { &f.lanes[tup[sets.iter().position(|s| s.0 == f.label).unwrap()]] };
                    let lv = mint(lane)?;
                    let mut next = Vec::new();
                    for v in &vs {
                        for l in &lv {
                            let mut w = v.clone();
                            w.extend(l.iter().copied());
                            next.push(w);
                        }
                    }
                    vs = next;
                }
                _ => {
                    for v in vs.iter_mut() {
                        v.push(it);
                    }
                }
            }
        }
        out.extend(vs);
    }
    Ok(out)
}

fn leaf_text(t: &[BTok], it: &It) -> String {
    match it {
        It::T(i) => t[*i].text.clone(),
        It::R(r) => t[r.from..r.to].iter().map(|x| x.text.as_str()).collect::<Vec<_>>().join(" "),
        It::F(_) => unreachable!(),
    }
}
fn is_value(text: &str) -> bool {
    canon_num(text).is_some() || text.contains('{') || text.contains('\'')
}

/// Parses text that has bead marks: tokens outside regions parse as usual; a region from a
/// fork to its join (or the end of its text line) becomes one `Op::Lanes`.
pub fn parse_beads(src: &str, it: &mut Interner) -> Result<Code, String> {
    let cs: Vec<char> = src.chars().collect();
    let toks = bead_tokens(&cs);
    let tree = Tree { t: &toks, p: 0 }.top()?;
    let rc_src: Rc<str> = Rc::from(src);
    let mut ops = Vec::new();
    let mut spans = Vec::new();
    for item in &tree {
        match item {
            It::T(i) => {
                let c = parse_plain(&toks[*i].text, it)?;
                for (op, sp) in c.ops.into_iter().zip(c.spans) {
                    ops.push(op);
                    spans.push(sp + toks[*i].at);
                }
            }
            It::R(r) => {
                ops.push(Op::Lanes(Rc::new(fork_of(&toks, r, &rc_src, it)?)));
                spans.push(r.at);
            }
            It::F(_) => unreachable!(),
        }
    }
    Ok(Code { ops, spans, unit: NO_UNIT, src: rc_src })
}

fn fork_of(t: &[BTok], r: &RegionNode, src: &Rc<str>, it: &mut Interner) -> Result<Fork, String> {
    let variants = mint(&r.items)?;
    let texts: Vec<Rc<str>> = variants.iter().map(|v| Rc::from(v.iter().map(|x| leaf_text(t, x)).collect::<Vec<_>>().join(" "))).collect();
    for x in &texts {
        parse(x, it).map_err(|e| format!("at {}: in a lane: {}", r.at, e))?;
    }
    let forks: Vec<&ForkNode> = r.items.iter().filter_map(|x| if let It::F(f) = x { Some(f) } else { None }).collect();
    let code = forks.iter().any(|f| f.lanes.iter().any(|l| l.iter().any(|x| match x { It::T(i) => !is_value(&t[*i].text), _ => true })));
    let mut f = Fork { at: r.at, src: src.clone(), texts, spread: None, shared: Rc::from(""), beads: None, code, vprog: None, kernel: None };
    // one fork at the head of its region, and the beads after its merge
    if forks.len() == 1 {
        let shared: String = r.items[1..].iter().map(|x| leaf_text(t, x)).collect::<Vec<_>>().join(" ");
        let fk = forks[0];
        if fk.lanes.len() == 1 && fk.lanes[0].len() == 1 {
            if let It::T(i) = &fk.lanes[0][0] {
                let w = &t[*i].text;
                let stem = w.strip_suffix('…').or_else(|| w.strip_suffix("..."));
                if let Some(s) = stem.filter(|s| !s.is_empty() && !s.contains(|c: char| c == '{' || c == '\'')) {
                    f.spread = Some(it.sym(s));
                    f.texts = Vec::new();
                    f.code = false;
                }
            }
        }
        let nums: Option<Vec<Vec<f64>>> = if f.spread.is_some() {
            None
        } else {
            fk.lanes.iter().map(|l| l.iter().map(|x| match x { It::T(i) => canon_num(&t[*i].text), _ => None }).collect()).collect()
        };
        let m = if f.spread.is_some() {
            Some(1)
        } else {
            nums.as_ref().filter(|n| !n.is_empty() && n.iter().all(|l| l.len() == n[0].len())).map(|n| n[0].len())
        };
        f.beads = nums;
        f.shared = Rc::from(shared.as_str());
        if let Some(m) = m {
            if let Ok(c) = parse(&shared, it) {
                f.vprog = analyze(&c.ops, m, it).map(Rc::new);
            }
        }
    }
    Ok(f)
}

// ---------- forks ----------

/// A function the compiler writes for a vector program: input columns, scalars read at the
/// fork, and one output column per value the fork keeps.
pub type Kernel = fn(&[&[f64]], &[f64], &mut [Vec<f64>], usize);

pub struct Fork {
    /// Where the fork stands in `src`, in characters.
    pub at: u32,
    pub src: Rc<str>,
    /// The minted lanes as text (literal forks).
    pub texts: Vec<Rc<str>>,
    /// `⟪xs…⟫`: one lane per item of list xs.
    pub spread: Option<Sym>,
    /// The beads after the merge, for a fork that is the only fork of its region.
    pub shared: Rc<str>,
    /// The numbers each lane bead pushes, when every lane bead is numbers.
    pub beads: Option<Vec<Vec<f64>>>,
    /// Some lane bead holds code: the lanes run on threads.
    pub code: bool,
    /// The shared beads as a vector program, when they are arithmetic.
    pub vprog: Option<Rc<VProg>>,
    pub kernel: Option<Kernel>,
}

impl std::fmt::Debug for Fork {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Fork({} lanes at {})", self.texts.len(), self.at)
    }
}

/// A fork the compiler wrote out: everything static.
pub struct ForkSpec {
    pub at: u32,
    pub unit: u32,
    pub texts: &'static [&'static str],
    pub spread: Option<u32>,
    pub shared: &'static str,
    pub beads: Option<&'static [&'static [f64]]>,
    pub code: bool,
    pub kernel: Option<Kernel>,
}

// ---------- the vector program ----------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VS {
    /// The j-th value each lane bead pushed.
    In(u16),
    K(f64),
    /// A value read once at the fork.
    Sc(u16),
    Var(u16),
    /// A name given as an argument (a destination).
    Name(u32),
}

#[derive(Clone, Debug)]
pub enum VOp {
    /// out = k(a, b, c); c is used by Sel (a ? b : c).
    Op(vx::K, VS, VS, VS, u16),
    Copy(VS, u16),
}

/// How a value read at the fork is read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScKind {
    /// as a lookup reads it
    Look,
    /// the number the name holds, 0 when it holds none (`x;` on a name never set)
    Num0,
    /// the number the name holds, which must be there (a write in some lanes keeps it in the others)
    Num,
    /// the machine's flag `ow` at the fork, as a mask
    Ow,
}

#[derive(Debug)]
pub struct VProg {
    pub m: usize,
    pub ops: Vec<VOp>,
    pub nvars: usize,
    pub scalars: Vec<(Sym, ScKind)>,
    /// What the fork files under each lane: the last value of every name it wrote.
    pub writes: Vec<(Sym, u16)>,
    /// Each lane's line at the join.
    pub line: Vec<VS>,
    /// The values the program keeps, in the order a kernel writes them.
    pub outs: Vec<u16>,
    /// Words and commands that must have no definition for the program to stand.
    pub words: Vec<(Sym, String)>,
    pub cmds: Vec<Cmd>,
    pub numwords: bool,
}

fn arith(c: Cmd) -> Option<vx::K> {
    Some(match c {
        Cmd::Add => vx::K::Add,
        Cmd::Sub => vx::K::Sub,
        Cmd::Mul => vx::K::Mul,
        Cmd::Div => vx::K::Div,
        Cmd::Pow => vx::K::Pow,
        Cmd::Mod => vx::K::Rem,
        _ => return None,
    })
}
fn compare(c: Cmd) -> Option<vx::K> {
    Some(match c {
        Cmd::Eq | Cmd::AndEq => vx::K::Eq,
        Cmd::Ne | Cmd::AndNe => vx::K::Ne,
        Cmd::NumEq => vx::K::NumEq,
        Cmd::Lt => vx::K::Lt,
        Cmd::Gt => vx::K::Gt,
        Cmd::Le => vx::K::Le,
        Cmd::Ge => vx::K::Ge,
        _ => return None,
    })
}

/// Which lanes a value on the line is in: every lane, or the lanes where a mask is `pol`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Pres {
    All,
    Lit(VS, bool),
}

/// Builds a vector program, keeping track of the line lane set by lane set.
struct An<'a> {
    p: VProg,
    it: &'a mut Interner,
    line: Vec<(VS, Pres)>,
    written: Vec<(Sym, u16)>,
    /// `ow` as a literal: the lanes where mask is pol are the lanes where ow is true.
    ow: Option<(VS, bool)>,
}

impl<'a> An<'a> {
    fn var(&mut self) -> u16 {
        self.p.nvars += 1;
        (self.p.nvars - 1) as u16
    }
    fn scalar(&mut self, k: Sym, kind: ScKind) -> VS {
        match self.p.scalars.iter().position(|s| *s == (k, kind)) {
            Some(i) => VS::Sc(i as u16),
            None => {
                self.p.scalars.push((k, kind));
                VS::Sc((self.p.scalars.len() - 1) as u16)
            }
        }
    }
    fn ow(&mut self) -> (VS, bool) {
        if self.ow.is_none() {
            let s = self.scalar(S_EQ, ScKind::Ow);
            self.ow = Some((s, true));
        }
        self.ow.unwrap()
    }
    fn written(&self, k: Sym) -> Option<u16> {
        self.written.iter().find(|w| w.0 == k).map(|w| w.1)
    }
    /// The values present in every lane of e, in order; None when some value is in part of them.
    fn args(&self, e: Option<(VS, bool)>) -> Option<Vec<VS>> {
        let mut got = Vec::new();
        for (v, p) in &self.line {
            match (p, e) {
                (Pres::All, _) => got.push(*v),
                (Pres::Lit(m, pl), Some((em, ep))) if *m == em => {
                    if *pl == ep {
                        got.push(*v)
                    }
                }
                _ => return None,
            }
        }
        Some(got)
    }
    /// A command ran in the lanes of e: the line is empty there.
    fn clear(&mut self, e: Option<(VS, bool)>) -> Option<()> {
        let mut out = Vec::new();
        for (v, p) in &self.line {
            match (p, e) {
                (_, None) => {}
                (Pres::All, Some((em, ep))) => out.push((*v, Pres::Lit(em, !ep))),
                (Pres::Lit(m, pl), Some((em, ep))) if *m == em => {
                    if *pl != ep {
                        out.push((*v, *p))
                    }
                }
                _ => return None,
            }
        }
        self.line = out;
        Some(())
    }
    fn push(&mut self, v: VS, e: Option<(VS, bool)>) -> Option<()> {
        let p = match e {
            Some((m, pl)) => Pres::Lit(m, pl),
            None => Pres::All,
        };
        self.line.push((v, p));
        Some(())
    }
    /// Name k takes v in the lanes of e and keeps what it had in the others.
    fn write(&mut self, k: Sym, v: VS, e: Option<(VS, bool)>) {
        let out = self.var();
        match e {
            None => self.p.ops.push(VOp::Copy(v, out)),
            Some((m, pl)) => {
                let old = match self.written(k) {
                    Some(w) => VS::Var(w),
                    None => self.scalar(k, ScKind::Num),
                };
                let (a, b) = if pl { (v, old) } else { (old, v) };
                self.p.ops.push(VOp::Op(vx::K::Sel, m, a, b, out));
            }
        }
        match self.written.iter_mut().find(|w| w.0 == k) {
            Some(w) => w.1 = out,
            None => self.written.push((k, out)),
        }
    }
    fn op(&mut self, k: vx::K, a: VS, b: VS) -> VS {
        let out = self.var();
        self.p.ops.push(VOp::Op(k, a, b, VS::K(0.0), out));
        VS::Var(out)
    }
    fn local(&self, k: Sym) -> bool {
        self.it.is_local(k)
    }

    /// One op, run in the lanes of e (None: every lane).
    fn step(&mut self, op: &Op, e: Option<(VS, bool)>) -> Option<()> {
        let num = |v: &VS| !matches!(v, VS::Name(_));
        match op {
            Op::Push(Val::N(x)) => self.push(VS::K(*x), e),
            Op::Push(v) => {
                let t = v.text();
                if is_local_name(&t) {
                    return None;
                }
                let k = self.it.sym(&t);
                self.push(VS::Name(k.0), e)
            }
            Op::Word(_, _, Some(x)) => {
                self.p.numwords = true;
                self.push(VS::K(*x), e)
            }
            Op::Word(t, k, None) => {
                if self.local(*k) {
                    return None;
                }
                self.p.words.push((*k, t.to_string()));
                self.push(VS::Name(k.0), e)
            }
            Op::Lookup(k) => {
                if self.local(*k) {
                    return None;
                }
                let v = match self.written(*k) {
                    Some(w) => VS::Var(w),
                    None => self.scalar(*k, ScKind::Look),
                };
                self.push(v, e)
            }
            Op::Cmd(c) | Op::Native(c) => {
                if matches!(op, Op::Cmd(_)) {
                    self.p.cmds.push(*c);
                }
                // these take nothing from the line, and clear it
                if *c == Cmd::Comment {
                    return self.clear(e);
                }
                if *c == Cmd::Not && e.is_none() {
                    let (m, pl) = self.ow();
                    self.ow = Some((m, !pl));
                    return self.clear(None);
                }
                let a = self.args(e)?;
                if let Some(k) = arith(*c) {
                    if a.len() < 2 || a.len() > 3 || !num(&a[0]) || !num(&a[1]) {
                        return None;
                    }
                    let dest = if a.len() == 3 {
                        match a[2] {
                            VS::Name(k) => Sym(k),
                            _ => return None,
                        }
                    } else {
                        S_EQ
                    };
                    if self.local(dest) {
                        return None;
                    }
                    let r = self.op(k, a[0], a[1]);
                    self.write(dest, r, e);
                    return self.clear(e);
                }
                match c {
                    Cmd::SetN => {
                        if a.len() < 2 || !num(&a[0]) {
                            return None;
                        }
                        let dest = match a[1] {
                            VS::Name(k) => Sym(k),
                            _ => return None,
                        };
                        if self.local(dest) {
                            return None;
                        }
                        self.write(dest, a[0], e);
                        self.clear(e)
                    }
                    Cmd::Comment => self.clear(e),
                    Cmd::Not if e.is_none() => {
                        let (m, pl) = self.ow();
                        self.ow = Some((m, !pl));
                        self.clear(None)
                    }
                    _ => {
                        let k = compare(*c)?;
                        if e.is_some() || a.len() < 2 || !num(&a[0]) || !num(&a[1]) {
                            return None;
                        }
                        let r = self.op(k, a[0], a[1]);
                        let mask = if matches!(c, Cmd::AndEq | Cmd::AndNe) {
                            let (m, pl) = self.ow();
                            self.op(if pl { vx::K::And } else { vx::K::AndNot }, m, r)
                        } else {
                            r
                        };
                        self.ow = Some((mask, true));
                        self.clear(None)
                    }
                }
            }
            Op::SetNum(k) | Op::Inc(k) | Op::Dec(k) => {
                if self.local(*k) {
                    return None;
                }
                let a = self.args(e)?;
                if a.first().map_or(false, |v| !num(v)) {
                    return None;
                }
                match op {
                    Op::SetNum(_) => {
                        let x = a.first().copied().unwrap_or(VS::K(0.0));
                        self.write(*k, x, e);
                    }
                    _ => {
                        let d = a.first().copied().unwrap_or(VS::K(1.0));
                        let cur = match self.written(*k) {
                            Some(w) => VS::Var(w),
                            None => self.scalar(*k, if e.is_some() { ScKind::Num } else { ScKind::Num0 }),
                        };
                        let r = self.op(if matches!(op, Op::Inc(_)) { vx::K::Add } else { vx::K::Sub }, cur, d);
                        self.write(*k, r, e);
                    }
                }
                self.clear(e)
            }
            Op::Cond(t, inner) => {
                if e.is_some() || matches!(**inner, Op::Cond(..)) {
                    return None;
                }
                if let Op::Cmd(c) | Op::Native(c) = &**inner {
                    if arith(*c).is_none() && *c != Cmd::SetN {
                        return None;
                    }
                }
                let (m, pl) = self.ow();
                self.step(inner, Some((m, if *t { pl } else { !pl })))
            }
            _ => None,
        }
    }
}

/// The shared beads of a fork over m numbers as a vector program, when every op is one the
/// vector machine has (BEADS.md §3.4).
pub fn analyze(ops: &[Op], m: usize, it: &mut Interner) -> Option<VProg> {
    let p = VProg { m, ops: Vec::new(), nvars: 0, scalars: Vec::new(), writes: Vec::new(), line: Vec::new(), outs: Vec::new(), words: Vec::new(), cmds: Vec::new(), numwords: false };
    let mut a = An { p, it, line: (0..m as u16).map(|j| (VS::In(j), Pres::All)).collect(), written: Vec::new(), ow: None };
    for op in ops {
        a.step(op, None)?;
    }
    // every lane ends with the same line
    if !a.line.iter().all(|(v, p)| *p == Pres::All && !matches!(v, VS::Name(_))) {
        return None;
    }
    let mut p = a.p;
    p.line = a.line.into_iter().map(|(v, _)| v).collect();
    p.writes = a.written;
    let mut outs: Vec<u16> = p.writes.iter().map(|w| w.1).chain(p.line.iter().filter_map(|v| if let VS::Var(i) = v { Some(*i) } else { None })).collect();
    outs.sort();
    outs.dedup();
    p.outs = outs;
    Some(p)
}

// ---------- SIMD ----------

/// Vectors of f64 for the machine and for compiled kernels: AVX (4 lanes) or SSE2 (2) on
/// x86-64, chosen at run time, NEON (2) on aarch64, one lane elsewhere.
pub mod vx {
    //! Vectors of f64 for the machine and for compiled kernels: AVX-512 (8 lanes), AVX (4)
    //! or SSE2 (2) on x86-64, chosen when the program runs; NEON (2) on aarch64; one lane
    //! elsewhere. A mask is a vector whose lanes are all ones (true) or all zeros (false).

    pub trait V: Copy {
        const W: usize;
        unsafe fn ld(p: *const f64) -> Self;
        unsafe fn st(self, p: *mut f64);
        unsafe fn sp(x: f64) -> Self;
        unsafe fn add(self, o: Self) -> Self;
        unsafe fn sub(self, o: Self) -> Self;
        unsafe fn mul(self, o: Self) -> Self;
        unsafe fn div(self, o: Self) -> Self;
        unsafe fn and(self, o: Self) -> Self;
        unsafe fn or(self, o: Self) -> Self;
        /// !self & o
        unsafe fn andnot(self, o: Self) -> Self;
        unsafe fn ceq(self, o: Self) -> Self;
        unsafe fn clt(self, o: Self) -> Self;
        unsafe fn cle(self, o: Self) -> Self;
        unsafe fn cgt(self, o: Self) -> Self;
        unsafe fn cge(self, o: Self) -> Self;
        /// Either side is NaN.
        unsafe fn cun(self, o: Self) -> Self;
        #[inline(always)]
        unsafe fn ones() -> Self {
            Self::sp(f64::from_bits(u64::MAX))
        }
        #[inline(always)]
        unsafe fn pow(self, o: Self) -> Self {
            let (mut a, mut b) = ([0.0f64; 8], [0.0f64; 8]);
            self.st(a.as_mut_ptr());
            o.st(b.as_mut_ptr());
            for i in 0..Self::W {
                a[i] = a[i].powf(b[i]);
            }
            Self::ld(a.as_ptr())
        }
        #[inline(always)]
        unsafe fn rem(self, o: Self) -> Self {
            let (mut a, mut b) = ([0.0f64; 8], [0.0f64; 8]);
            self.st(a.as_mut_ptr());
            o.st(b.as_mut_ptr());
            for i in 0..Self::W {
                a[i] = a[i].rem_euclid(b[i]);
            }
            Self::ld(a.as_ptr())
        }
        /// `=` as the machine compares numbers: equal, or both NaN.
        #[inline(always)]
        unsafe fn veq(self, o: Self) -> Self {
            self.ceq(o).or(self.cun(self).and(o.cun(o)))
        }
        #[inline(always)]
        unsafe fn vne(self, o: Self) -> Self {
            self.veq(o).andnot(Self::ones())
        }
        /// m ? a : b, lane by lane.
        #[inline(always)]
        unsafe fn sel(m: Self, a: Self, b: Self) -> Self {
            m.and(a).or(m.andnot(b))
        }
    }

    #[derive(Clone, Copy)]
    pub struct S1(f64);
    #[inline(always)]
    fn bits(b: bool) -> f64 {
        f64::from_bits(if b { u64::MAX } else { 0 })
    }
    impl V for S1 {
        const W: usize = 1;
        #[inline(always)]
        unsafe fn ld(p: *const f64) -> Self { S1(*p) }
        #[inline(always)]
        unsafe fn st(self, p: *mut f64) { *p = self.0 }
        #[inline(always)]
        unsafe fn sp(x: f64) -> Self { S1(x) }
        #[inline(always)]
        unsafe fn add(self, o: Self) -> Self { S1(self.0 + o.0) }
        #[inline(always)]
        unsafe fn sub(self, o: Self) -> Self { S1(self.0 - o.0) }
        #[inline(always)]
        unsafe fn mul(self, o: Self) -> Self { S1(self.0 * o.0) }
        #[inline(always)]
        unsafe fn div(self, o: Self) -> Self { S1(self.0 / o.0) }
        #[inline(always)]
        unsafe fn pow(self, o: Self) -> Self { S1(self.0.powf(o.0)) }
        #[inline(always)]
        unsafe fn rem(self, o: Self) -> Self { S1(self.0.rem_euclid(o.0)) }
        #[inline(always)]
        unsafe fn and(self, o: Self) -> Self { S1(f64::from_bits(self.0.to_bits() & o.0.to_bits())) }
        #[inline(always)]
        unsafe fn or(self, o: Self) -> Self { S1(f64::from_bits(self.0.to_bits() | o.0.to_bits())) }
        #[inline(always)]
        unsafe fn andnot(self, o: Self) -> Self { S1(f64::from_bits(!self.0.to_bits() & o.0.to_bits())) }
        #[inline(always)]
        unsafe fn ceq(self, o: Self) -> Self { S1(bits(self.0 == o.0)) }
        #[inline(always)]
        unsafe fn clt(self, o: Self) -> Self { S1(bits(self.0 < o.0)) }
        #[inline(always)]
        unsafe fn cle(self, o: Self) -> Self { S1(bits(self.0 <= o.0)) }
        #[inline(always)]
        unsafe fn cgt(self, o: Self) -> Self { S1(bits(self.0 > o.0)) }
        #[inline(always)]
        unsafe fn cge(self, o: Self) -> Self { S1(bits(self.0 >= o.0)) }
        #[inline(always)]
        unsafe fn cun(self, o: Self) -> Self { S1(bits(self.0.is_nan() || o.0.is_nan())) }
    }

    #[cfg(target_arch = "x86_64")]
    pub use x86::*;
    #[cfg(target_arch = "x86_64")]
    mod x86 {
        use super::V;
        use std::arch::x86_64::*;
        #[derive(Clone, Copy)]
        pub struct Sse(__m128d);
        impl V for Sse {
            const W: usize = 2;
            #[inline(always)]
            unsafe fn ld(p: *const f64) -> Self { Sse(_mm_loadu_pd(p)) }
            #[inline(always)]
            unsafe fn st(self, p: *mut f64) { _mm_storeu_pd(p, self.0) }
            #[inline(always)]
            unsafe fn sp(x: f64) -> Self { Sse(_mm_set1_pd(x)) }
            #[inline(always)]
            unsafe fn add(self, o: Self) -> Self { Sse(_mm_add_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn sub(self, o: Self) -> Self { Sse(_mm_sub_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn mul(self, o: Self) -> Self { Sse(_mm_mul_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn div(self, o: Self) -> Self { Sse(_mm_div_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn and(self, o: Self) -> Self { Sse(_mm_and_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn or(self, o: Self) -> Self { Sse(_mm_or_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn andnot(self, o: Self) -> Self { Sse(_mm_andnot_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn ceq(self, o: Self) -> Self { Sse(_mm_cmpeq_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn clt(self, o: Self) -> Self { Sse(_mm_cmplt_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn cle(self, o: Self) -> Self { Sse(_mm_cmple_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn cgt(self, o: Self) -> Self { Sse(_mm_cmpgt_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn cge(self, o: Self) -> Self { Sse(_mm_cmpge_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn cun(self, o: Self) -> Self { Sse(_mm_cmpunord_pd(self.0, o.0)) }
        }
        #[derive(Clone, Copy)]
        pub struct Avx(__m256d);
        impl V for Avx {
            const W: usize = 4;
            #[inline(always)]
            unsafe fn ld(p: *const f64) -> Self { Avx(_mm256_loadu_pd(p)) }
            #[inline(always)]
            unsafe fn st(self, p: *mut f64) { _mm256_storeu_pd(p, self.0) }
            #[inline(always)]
            unsafe fn sp(x: f64) -> Self { Avx(_mm256_set1_pd(x)) }
            #[inline(always)]
            unsafe fn add(self, o: Self) -> Self { Avx(_mm256_add_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn sub(self, o: Self) -> Self { Avx(_mm256_sub_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn mul(self, o: Self) -> Self { Avx(_mm256_mul_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn div(self, o: Self) -> Self { Avx(_mm256_div_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn and(self, o: Self) -> Self { Avx(_mm256_and_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn or(self, o: Self) -> Self { Avx(_mm256_or_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn andnot(self, o: Self) -> Self { Avx(_mm256_andnot_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn ceq(self, o: Self) -> Self { Avx(_mm256_cmp_pd::<_CMP_EQ_OQ>(self.0, o.0)) }
            #[inline(always)]
            unsafe fn clt(self, o: Self) -> Self { Avx(_mm256_cmp_pd::<_CMP_LT_OQ>(self.0, o.0)) }
            #[inline(always)]
            unsafe fn cle(self, o: Self) -> Self { Avx(_mm256_cmp_pd::<_CMP_LE_OQ>(self.0, o.0)) }
            #[inline(always)]
            unsafe fn cgt(self, o: Self) -> Self { Avx(_mm256_cmp_pd::<_CMP_GT_OQ>(self.0, o.0)) }
            #[inline(always)]
            unsafe fn cge(self, o: Self) -> Self { Avx(_mm256_cmp_pd::<_CMP_GE_OQ>(self.0, o.0)) }
            #[inline(always)]
            unsafe fn cun(self, o: Self) -> Self { Avx(_mm256_cmp_pd::<_CMP_UNORD_Q>(self.0, o.0)) }
        }
        #[derive(Clone, Copy)]
        pub struct Avx512(__m512d);
        #[inline(always)]
        unsafe fn mask8(k: __mmask8) -> Avx512 {
            Avx512(_mm512_castsi512_pd(_mm512_maskz_set1_epi64(k, -1)))
        }
        #[inline(always)]
        unsafe fn si(a: __m512d) -> __m512i {
            _mm512_castpd_si512(a)
        }
        impl V for Avx512 {
            const W: usize = 8;
            #[inline(always)]
            unsafe fn ld(p: *const f64) -> Self { Avx512(_mm512_loadu_pd(p)) }
            #[inline(always)]
            unsafe fn st(self, p: *mut f64) { _mm512_storeu_pd(p, self.0) }
            #[inline(always)]
            unsafe fn sp(x: f64) -> Self { Avx512(_mm512_set1_pd(x)) }
            #[inline(always)]
            unsafe fn add(self, o: Self) -> Self { Avx512(_mm512_add_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn sub(self, o: Self) -> Self { Avx512(_mm512_sub_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn mul(self, o: Self) -> Self { Avx512(_mm512_mul_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn div(self, o: Self) -> Self { Avx512(_mm512_div_pd(self.0, o.0)) }
            #[inline(always)]
            unsafe fn and(self, o: Self) -> Self { Avx512(_mm512_castsi512_pd(_mm512_and_si512(si(self.0), si(o.0)))) }
            #[inline(always)]
            unsafe fn or(self, o: Self) -> Self { Avx512(_mm512_castsi512_pd(_mm512_or_si512(si(self.0), si(o.0)))) }
            #[inline(always)]
            unsafe fn andnot(self, o: Self) -> Self { Avx512(_mm512_castsi512_pd(_mm512_andnot_si512(si(self.0), si(o.0)))) }
            #[inline(always)]
            unsafe fn ceq(self, o: Self) -> Self { mask8(_mm512_cmp_pd_mask::<_CMP_EQ_OQ>(self.0, o.0)) }
            #[inline(always)]
            unsafe fn clt(self, o: Self) -> Self { mask8(_mm512_cmp_pd_mask::<_CMP_LT_OQ>(self.0, o.0)) }
            #[inline(always)]
            unsafe fn cle(self, o: Self) -> Self { mask8(_mm512_cmp_pd_mask::<_CMP_LE_OQ>(self.0, o.0)) }
            #[inline(always)]
            unsafe fn cgt(self, o: Self) -> Self { mask8(_mm512_cmp_pd_mask::<_CMP_GT_OQ>(self.0, o.0)) }
            #[inline(always)]
            unsafe fn cge(self, o: Self) -> Self { mask8(_mm512_cmp_pd_mask::<_CMP_GE_OQ>(self.0, o.0)) }
            #[inline(always)]
            unsafe fn cun(self, o: Self) -> Self { mask8(_mm512_cmp_pd_mask::<_CMP_UNORD_Q>(self.0, o.0)) }
        }
    }

    #[cfg(target_arch = "aarch64")]
    pub use arm::*;
    #[cfg(target_arch = "aarch64")]
    mod arm {
        use super::V;
        use std::arch::aarch64::*;
        #[derive(Clone, Copy)]
        pub struct Neon(float64x2_t);
        #[inline(always)]
        unsafe fn u(a: float64x2_t) -> uint64x2_t {
            vreinterpretq_u64_f64(a)
        }
        #[inline(always)]
        unsafe fn f(a: uint64x2_t) -> Neon {
            Neon(vreinterpretq_f64_u64(a))
        }
        impl V for Neon {
            const W: usize = 2;
            #[inline(always)]
            unsafe fn ld(p: *const f64) -> Self { Neon(vld1q_f64(p)) }
            #[inline(always)]
            unsafe fn st(self, p: *mut f64) { vst1q_f64(p, self.0) }
            #[inline(always)]
            unsafe fn sp(x: f64) -> Self { Neon(vdupq_n_f64(x)) }
            #[inline(always)]
            unsafe fn add(self, o: Self) -> Self { Neon(vaddq_f64(self.0, o.0)) }
            #[inline(always)]
            unsafe fn sub(self, o: Self) -> Self { Neon(vsubq_f64(self.0, o.0)) }
            #[inline(always)]
            unsafe fn mul(self, o: Self) -> Self { Neon(vmulq_f64(self.0, o.0)) }
            #[inline(always)]
            unsafe fn div(self, o: Self) -> Self { Neon(vdivq_f64(self.0, o.0)) }
            #[inline(always)]
            unsafe fn and(self, o: Self) -> Self { f(vandq_u64(u(self.0), u(o.0))) }
            #[inline(always)]
            unsafe fn or(self, o: Self) -> Self { f(vorrq_u64(u(self.0), u(o.0))) }
            #[inline(always)]
            unsafe fn andnot(self, o: Self) -> Self { f(vbicq_u64(u(o.0), u(self.0))) }
            #[inline(always)]
            unsafe fn ceq(self, o: Self) -> Self { f(vceqq_f64(self.0, o.0)) }
            #[inline(always)]
            unsafe fn clt(self, o: Self) -> Self { f(vcltq_f64(self.0, o.0)) }
            #[inline(always)]
            unsafe fn cle(self, o: Self) -> Self { f(vcleq_f64(self.0, o.0)) }
            #[inline(always)]
            unsafe fn cgt(self, o: Self) -> Self { f(vcgtq_f64(self.0, o.0)) }
            #[inline(always)]
            unsafe fn cge(self, o: Self) -> Self { f(vcgeq_f64(self.0, o.0)) }
            #[inline(always)]
            unsafe fn cun(self, o: Self) -> Self {
                let both = vandq_u64(vceqq_f64(self.0, self.0), vceqq_f64(o.0, o.0));
                f(vbicq_u64(vdupq_n_u64(u64::MAX), both))
            }
        }
    }

    /// The vector level this run uses: 4 AVX-512, 3 AVX, 2 SSE2, 1 NEON, 0 one lane. The
    /// machine takes the widest it has; `S_SIMD` (avx512, avx, sse2, neon, scalar) narrows it.
    pub fn level_code() -> u8 {
        use std::sync::atomic::{AtomicU8, Ordering};
        static L: AtomicU8 = AtomicU8::new(255);
        let l = L.load(Ordering::Relaxed);
        if l != 255 {
            return l;
        }
        #[allow(unused_mut, unused_assignments)]
        let mut best = 0u8;
        #[cfg(target_arch = "x86_64")]
        {
            best = if is_x86_feature_detected!("avx512f") { 4 } else if is_x86_feature_detected!("avx") { 3 } else { 2 };
        }
        #[cfg(target_arch = "aarch64")]
        {
            best = 1;
        }
        let want = match std::env::var("S_SIMD").as_deref() {
            Ok("avx512") => 4,
            Ok("avx") => 3,
            Ok("sse2") => 2,
            Ok("neon") => 1,
            Ok("scalar") => 0,
            _ => best,
        };
        // never above what the machine has, and on x86-64 never NEON
        let l = if want > best || (best >= 2 && want == 1) { best } else { want };
        L.store(l, Ordering::Relaxed);
        l
    }
    pub fn avx512() -> bool {
        level_code() == 4
    }
    pub fn avx() -> bool {
        level_code() >= 3
    }
    pub fn sse2() -> bool {
        level_code() >= 2
    }

    /// The vectors this machine uses, as a fork report names them.
    pub fn level() -> &'static str {
        match level_code() {
            4 => "f64×8 avx-512",
            3 => "f64×4 avx",
            2 => "f64×2 sse2",
            1 => "f64×2 neon",
            _ => "f64×1",
        }
    }

    /// One side of a column operation: a column, or one number for every lane.
    #[derive(Clone, Copy)]
    pub enum Arg<'a> {
        C(&'a [f64]),
        K(f64),
    }

    #[inline(always)]
    unsafe fn get<T: V>(a: Arg, i: usize) -> T {
        match a {
            Arg::C(s) => T::ld(s.as_ptr().add(i)),
            Arg::K(k) => T::sp(k),
        }
    }

    #[inline(always)]
    unsafe fn run3<T: V, F: Fn(T, T, T) -> T>(a: Arg, b: Arg, c: Arg, out: &mut [f64], from: usize, f: F) -> usize {
        let n = out.len();
        let mut i = from;
        while i + T::W <= n {
            let r = f(get::<T>(a, i), get::<T>(b, i), get::<T>(c, i));
            r.st(out.as_mut_ptr().add(i));
            i += T::W;
        }
        i
    }

    /// The column operations of the machine's vector programs.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum K {
        Add,
        Sub,
        Mul,
        Div,
        Pow,
        Rem,
        /// `=`: equal or both NaN; `!=`; `==`; `<<`; `>>`; `<<=`; `>>=`
        Eq,
        Ne,
        NumEq,
        Lt,
        Gt,
        Le,
        Ge,
        /// a & b; !a & b (masks)
        And,
        AndNot,
        /// a ? b : c
        Sel,
    }

    macro_rules! each_k {
        ($T:ty, $k:expr, $a:expr, $b:expr, $c:expr, $out:expr, $from:expr) => {
            match $k {
                K::Add => run3::<$T, _>($a, $b, $c, $out, $from, |x, y, _| x.add(y)),
                K::Sub => run3::<$T, _>($a, $b, $c, $out, $from, |x, y, _| x.sub(y)),
                K::Mul => run3::<$T, _>($a, $b, $c, $out, $from, |x, y, _| x.mul(y)),
                K::Div => run3::<$T, _>($a, $b, $c, $out, $from, |x, y, _| x.div(y)),
                K::Pow => run3::<$T, _>($a, $b, $c, $out, $from, |x, y, _| x.pow(y)),
                K::Rem => run3::<$T, _>($a, $b, $c, $out, $from, |x, y, _| x.rem(y)),
                K::Eq => run3::<$T, _>($a, $b, $c, $out, $from, |x, y, _| x.veq(y)),
                K::Ne => run3::<$T, _>($a, $b, $c, $out, $from, |x, y, _| x.vne(y)),
                K::NumEq => run3::<$T, _>($a, $b, $c, $out, $from, |x, y, _| x.ceq(y)),
                K::Lt => run3::<$T, _>($a, $b, $c, $out, $from, |x, y, _| x.clt(y)),
                K::Gt => run3::<$T, _>($a, $b, $c, $out, $from, |x, y, _| x.cgt(y)),
                K::Le => run3::<$T, _>($a, $b, $c, $out, $from, |x, y, _| x.cle(y)),
                K::Ge => run3::<$T, _>($a, $b, $c, $out, $from, |x, y, _| x.cge(y)),
                K::And => run3::<$T, _>($a, $b, $c, $out, $from, |x, y, _| x.and(y)),
                K::AndNot => run3::<$T, _>($a, $b, $c, $out, $from, |x, y, _| x.andnot(y)),
                K::Sel => run3::<$T, _>($a, $b, $c, $out, $from, |m, y, z| <$T>::sel(m, y, z)),
            }
        };
    }

    #[inline(always)]
    unsafe fn op_with<T: V>(k: K, a: Arg, b: Arg, c: Arg, out: &mut [f64]) {
        let i = each_k!(T, k, a, b, c, out, 0);
        each_k!(S1, k, a, b, c, out, i);
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx512f")]
    unsafe fn op_avx512(k: K, a: Arg, b: Arg, c: Arg, out: &mut [f64]) {
        op_with::<Avx512>(k, a, b, c, out)
    }
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx")]
    unsafe fn op_avx(k: K, a: Arg, b: Arg, c: Arg, out: &mut [f64]) {
        op_with::<Avx>(k, a, b, c, out)
    }

    /// out = k(a, b, c), lane by lane (c only for Sel).
    pub fn op(k: K, a: Arg, b: Arg, c: Arg, out: &mut [f64]) {
        for x in [a, b, c] {
            if let Arg::C(s) = x {
                assert!(s.len() >= out.len());
            }
        }
        unsafe {
            #[cfg(target_arch = "x86_64")]
            {
                return match level_code() {
                    4 => op_avx512(k, a, b, c, out),
                    3 => op_avx(k, a, b, c, out),
                    2 => op_with::<Sse>(k, a, b, c, out),
                    _ => op_with::<S1>(k, a, b, c, out),
                };
            }
            #[cfg(target_arch = "aarch64")]
            {
                return match level_code() {
                    1 => op_with::<Neon>(k, a, b, c, out),
                    _ => op_with::<S1>(k, a, b, c, out),
                };
            }
            #[allow(unreachable_code)]
            op_with::<S1>(k, a, b, c, out)
        }
    }

    /// out = a (c) b, for c in + - * / ^ % (0 … 5).
    pub fn bin(c: u8, a: Arg, b: Arg, out: &mut [f64]) {
        let k = [K::Add, K::Sub, K::Mul, K::Div, K::Pow, K::Rem][c.min(5) as usize];
        op(k, a, b, Arg::K(0.0), out)
    }
}

// ---------- joined lanes ----------

/// One name as a lane left it.
#[derive(Clone, Default)]
pub struct Slot {
    pub mem: Option<Val>,
    pub num: Option<f64>,
    pub list: Option<List>,
    pub script: Option<(Script, u64)>,
}

/// What a lane wrote: names, and replacements by word.
#[derive(Clone, Default)]
pub struct LaneW {
    pub names: Vec<(Sym, Slot)>,
    pub repl: Vec<(Rc<str>, Option<Vec<Macro>>)>,
}

#[derive(Clone)]
pub enum Col {
    K(f64),
    V(Arc<Vec<f64>>),
}
impl Col {
    fn at(&self, i: usize) -> f64 {
        match self {
            Col::K(x) => *x,
            Col::V(v) => v[i],
        }
    }
    fn arg(&self) -> vx::Arg<'_> {
        match self {
            Col::K(x) => vx::Arg::K(*x),
            Col::V(v) => vx::Arg::C(&v[..]),
        }
    }
}

/// The lanes after a join: one line and one set of writes per lane, or the same as columns
/// when the fork ran as a vector program.
pub enum Joined {
    Rows(Vec<(Vec<Val>, LaneW)>),
    Cols { n: usize, line: Vec<Col>, writes: Vec<(Sym, Col)> },
}

impl Joined {
    pub fn len(&self) -> usize {
        match self {
            Joined::Rows(r) => r.len(),
            Joined::Cols { n, .. } => *n,
        }
    }
    pub fn line(&self, i: usize) -> Vec<Val> {
        match self {
            Joined::Rows(r) => r[i].0.clone(),
            Joined::Cols { line, .. } => line.iter().map(|c| Val::N(c.at(i))).collect(),
        }
    }
    pub fn writes(&self, i: usize) -> LaneW {
        match self {
            Joined::Rows(r) => r[i].1.clone(),
            Joined::Cols { writes, .. } => LaneW { names: writes.iter().map(|(k, c)| (*k, Slot { num: Some(c.at(i)), ..Slot::default() })).collect(), repl: Vec::new() },
        }
    }
}

/// The record of one running lane: the first value of every name it writes.
#[derive(Default)]
pub struct LaneLog {
    pub seen: FxSet<u32>,
    pub old: Vec<(Sym, Slot)>,
    pub repl_seen: FxSet<Rc<str>>,
    pub repl_old: Vec<(Rc<str>, Option<Vec<Macro>>)>,
}

pub struct ForkRun {
    pub fork: Rc<Fork>,
    pub base: Vec<Val>,
    pub n: usize,
    pub next: usize,
    pub running: Option<usize>,
    pub items: Vec<Val>,
    pub rows: Vec<(Vec<Val>, LaneW)>,
    pub outer_log: Option<Box<LaneLog>>,
    pub outer_at: Option<usize>,
    pub started: std::time::Instant,
    /// `ow` at the fork: every lane starts with it, and the join puts it back.
    pub ow0: bool,
}

pub enum EachKind {
    Reduce,
    Lanewise,
}
pub struct EachRun {
    pub kind: EachKind,
    pub q: Rc<str>,
    pub i: usize,
    pub n: usize,
    pub running: bool,
    pub rows: Vec<(Vec<Val>, LaneW)>,
    pub base: Vec<Val>,
    pub outer_log: Option<Box<LaneLog>>,
    pub outer_at: Option<usize>,
    pub ow0: bool,
}

pub struct View {
    pub saved: Vec<(Sym, Slot)>,
    pub lane: LaneW,
}

pub struct ForkSeen {
    pub src: Rc<str>,
    pub at: u32,
    pub how: String,
    /// lanes at the join
    pub n: usize,
    /// the lines of the first 16 lanes at the join
    pub lines: Vec<Vec<String>>,
}

impl Rt {
    // ---------- names as slots ----------

    pub fn slot(&self, s: Sym) -> Slot {
        let i = s.0 as usize;
        Slot {
            mem: self.mem.get(i).cloned().flatten(),
            num: self.nums.get(i).copied().flatten(),
            list: self.lists.get(i).cloned().flatten(),
            script: self.scripts.get(i).cloned().flatten(),
        }
    }
    pub fn put_slot(&mut self, s: Sym, sl: Slot) {
        self.epoch = self.epoch.wrapping_add(1);
        let i = s.0 as usize;
        fn put<T>(v: &mut Vec<Option<T>>, i: usize, x: Option<T>) {
            if x.is_none() && v.len() <= i {
                return;
            }
            if v.len() <= i {
                v.resize_with(i + 1, || None);
            }
            v[i] = x;
        }
        put(&mut self.mem, i, sl.mem);
        put(&mut self.nums, i, sl.num);
        put(&mut self.lists, i, sl.list);
        put(&mut self.scripts, i, sl.script);
    }
    /// Before a running lane first writes name s.
    #[inline(never)]
    pub fn note_slow(&mut self, s: Sym) {
        let fresh = match &mut self.lanelog {
            Some(l) => l.seen.insert(s.0),
            None => false,
        };
        if fresh {
            let sl = self.slot(s);
            self.lanelog.as_mut().unwrap().old.push((s, sl));
        }
    }
    #[inline(always)]
    pub fn note(&mut self, s: Sym) {
        self.epoch = self.epoch.wrapping_add(1);
        if self.lanelog.is_some() {
            self.note_slow(s)
        }
    }
    pub fn note_repl(&mut self, key: &str) {
        self.epoch = self.epoch.wrapping_add(1);
        let fresh = match &mut self.lanelog {
            Some(l) => !l.repl_seen.contains(key),
            None => false,
        };
        if fresh {
            let k: Rc<str> = Rc::from(key);
            let old = self.repl.get(key).cloned();
            let l = self.lanelog.as_mut().unwrap();
            l.repl_seen.insert(k.clone());
            l.repl_old.push((k, old));
        }
    }
    /// Ends a lane's record: what the lane wrote, with every name put back as it was.
    fn close_log(&mut self, log: Box<LaneLog>) -> LaneW {
        let mut w = LaneW::default();
        let log = *log;
        for (s, old) in log.old.into_iter().rev() {
            let cur = self.slot(s);
            let empty = cur.mem.is_none() && cur.num.is_none() && cur.list.is_none() && cur.script.is_none();
            let was_empty = old.mem.is_none() && old.num.is_none() && old.list.is_none() && old.script.is_none();
            if !(empty && was_empty) {
                w.names.push((s, cur));
            }
            self.put_slot(s, old);
        }
        w.names.reverse();
        for (k, old) in log.repl_old.into_iter().rev() {
            let cur = self.repl.get(&*k).cloned();
            w.repl.push((k.clone(), cur));
            match old {
                Some(v) => {
                    self.repl.insert(k, v);
                }
                None => {
                    self.repl.remove(&*k);
                }
            }
        }
        self.gen += 1;
        w
    }
    /// Puts a lane's writes in place, recording in the running lane (if any) what they cover.
    fn apply_writes(&mut self, w: &LaneW) -> Vec<(Sym, Slot)> {
        let mut saved = Vec::new();
        for (s, sl) in &w.names {
            self.note(*s);
            saved.push((*s, self.slot(*s)));
            self.put_slot(*s, sl.clone());
        }
        for (k, v) in &w.repl {
            self.note_repl(k);
            match v {
                Some(v) => {
                    self.repl.insert(k.clone(), v.clone());
                }
                None => {
                    self.repl.remove(&**k);
                }
            }
        }
        self.gen += 1;
        saved
    }

    // ---------- running a fork ----------

    pub fn lanes_static(&mut self, spec: &'static ForkSpec) {
        let key = spec as *const ForkSpec as usize;
        let f = match self.fork_cache.get(&key) {
            Some(f) => f.clone(),
            None => {
                let shared = spec.shared;
                let m = if spec.spread.is_some() { Some(1) } else { spec.beads.filter(|b| !b.is_empty() && b.iter().all(|l| l.len() == b[0].len())).map(|b| b[0].len()) };
                let vprog = match m {
                    Some(m) => parse(shared, &mut self.it).ok().and_then(|c| analyze(&c.ops, m, &mut self.it)).map(Rc::new),
                    None => None,
                };
                let src: Rc<str> = self.prog.chunks.get(spec.unit as usize).map(|c| Rc::from(c.0)).unwrap_or_else(|| Rc::from(""));
                let f = Rc::new(Fork {
                    at: spec.at,
                    src,
                    texts: spec.texts.iter().map(|t| Rc::from(*t)).collect(),
                    spread: spec.spread.map(Sym),
                    shared: Rc::from(shared),
                    beads: spec.beads.map(|b| b.iter().map(|l| l.to_vec()).collect()),
                    code: spec.code,
                    vprog,
                    kernel: spec.kernel,
                });
                self.fork_cache.insert(key, f.clone());
                f
            }
        };
        self.lanes_op(f)
    }

    /// How a fork ran and what its lanes hold at the join, when the run keeps a fork log.
    fn seen(&mut self, f: &Fork, how: String, t: std::time::Instant) {
        if self.fork_log.is_none() {
            return;
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let how = format!("{} · {} ms", how, if ms < 1.0 { format!("{:.2}", ms) } else { format!("{:.1}", ms) });
        let (n, lines) = match &self.joined {
            Some(j) => (j.len(), (0..j.len().min(16)).map(|i| j.line(i).iter().map(|v| v.text().into_owned()).collect()).collect()),
            None => (0, Vec::new()),
        };
        if let Some(l) = &mut self.fork_log {
            l.push(ForkSeen { src: f.src.clone(), at: f.at, how, n, lines });
        }
    }

    /// FORK … JOIN: the lanes run and are held at the join.
    pub fn lanes_op(&mut self, f: Rc<Fork>) {
        let t0 = std::time::Instant::now();
        self.end_view();
        let base = std::mem::take(&mut self.args);
        let spread = f.spread.map(|k| self.l(k));
        let n = match spread {
            Some(k) => self.lists.get(k.0 as usize).and_then(|l| l.as_ref()).map_or(0, |l| l.len()),
            None => f.texts.len(),
        };
        // a vector program over numbers
        if let Some(p) = f.vprog.clone() {
            if base.is_empty() {
                // a list of numbers goes into the fork as it is: the column is shared
                let ins: Option<Vec<Arc<Vec<f64>>>> = if let Some(k) = spread {
                    match self.lists.get(k.0 as usize).and_then(|l| l.as_ref()) {
                        Some(List::N(v)) => Some(vec![v.clone()]),
                        Some(List::V(l)) => l.iter().map(|v| canon_num(&v.text())).collect::<Option<Vec<f64>>>().map(|c| vec![Arc::new(c)]),
                        None => Some(vec![Arc::new(Vec::new())]),
                    }
                } else {
                    f.beads.as_ref().map(|b| (0..p.m).map(|j| Arc::new(b.iter().map(|l| l[j]).collect())).collect())
                };
                if let Some(ins) = ins {
                    let r = self.run_vector(&f, &p, ins, n);
                    if let Some(j) = r {
                        let how = format!("{} · {} lanes · {}", vx::level(), n, if f.kernel.is_some() { "compiled kernel" } else { "machine kernels" });
                        self.joined = Some(Rc::new(j));
                        self.seen(&f, how, t0);
                        self.args = base;
                        return;
                    }
                }
            }
        }
        // lanes holding code on threads
        if f.code && n > 1 && self.g.is_none() && !self.guards && !self.fold {
            let lines: Vec<Vec<Val>> = (0..n).map(|_| base.clone()).collect();
            let texts: Vec<Rc<str>> = f.texts.clone();
            let rows = self.lanes_threads(&texts, &lines);
            self.joined = Some(Rc::new(Joined::Rows(rows)));
            self.seen(&f, format!("{} threads", n), t0);
            self.args = base;
            return;
        }
        let items: Vec<Val> = match spread {
            Some(k) => self.lists.get(k.0 as usize).and_then(|l| l.as_ref()).map(|l| l.to_vals()).unwrap_or_default(),
            None => Vec::new(),
        };
        let run = ForkRun { fork: f, base, n, next: 0, running: None, items, rows: Vec::with_capacity(n), outer_log: self.lanelog.take(), outer_at: self.lane_at, started: t0, ow0: self.ow };
        let bound = self.bound;
        self.fs.push(Frame { body: Body::Lanes(Rc::new(RefCell::new(run))), pc: 0, bound });
    }

    /// One step of a fork running lane by lane, each time its frame is on top.
    pub fn lanes_step(&mut self, idx: usize, run: Rc<RefCell<ForkRun>>) {
        let mut r = run.borrow_mut();
        if let Some(i) = r.running.take() {
            let log = self.lanelog.take().unwrap_or_default();
            let w = self.close_log(log);
            let args = std::mem::take(&mut self.args);
            r.rows.push((args, w));
            let _ = i;
        }
        if r.next < r.n {
            let i = r.next;
            r.next += 1;
            r.running = Some(i);
            let mut line = r.base.clone();
            self.lanelog = Some(Box::new(LaneLog::default()));
            self.lane_at = Some(i);
            self.ow = r.ow0;
            let text = if r.fork.spread.is_some() {
                line.push(r.items[i].clone());
                r.fork.shared.clone()
            } else {
                r.fork.texts[i].clone()
            };
            self.args = line;
            drop(r);
            if !text.trim().is_empty() {
                let f = self.src_frame(&text);
                self.fs.push(f);
            }
            return;
        }
        // the join
        self.lanelog = r.outer_log.take();
        self.lane_at = r.outer_at;
        self.ow = r.ow0;
        self.args = std::mem::take(&mut r.base);
        let rows = std::mem::take(&mut r.rows);
        let (f, n, t0) = (r.fork.clone(), r.n, r.started);
        drop(r);
        self.joined = Some(Rc::new(Joined::Rows(rows)));
        self.seen(&f, format!("one lane at a time · {} lanes", n), t0);
        self.end_frame_pub(idx);
    }

    fn run_vector(&mut self, f: &Fork, p: &VProg, ins: Vec<Arc<Vec<f64>>>, n: usize) -> Option<Joined> {
        if self.guards || (p.numwords && self.repl_num) {
            return None;
        }
        for (_, t) in &p.words {
            if self.repl_active(t) {
                return None;
            }
        }
        for c in &p.cmds {
            if self.repl_active(c.word()) {
                return None;
            }
        }
        // a name the fork writes must hold no text: a lookup reads text first
        for (k, _) in &p.writes {
            if self.mem.get(k.0 as usize).map_or(false, |m| m.is_some()) {
                return None;
            }
        }
        let mut sc = Vec::with_capacity(p.scalars.len());
        for &(k, kind) in &p.scalars {
            let x = match kind {
                ScKind::Look => self.num_of(k)?,
                ScKind::Num0 => self.nums.get(k.0 as usize).copied().flatten().unwrap_or(0.0),
                ScKind::Num => self.nums.get(k.0 as usize).copied().flatten()?,
                ScKind::Ow => f64::from_bits(if self.ow { u64::MAX } else { 0 }),
            };
            sc.push(x);
        }
        let mut vars: Vec<Option<Col>> = vec![None; p.nvars];
        match f.kernel {
            Some(k) => {
                let refs: Vec<&[f64]> = ins.iter().map(|c| &c[..]).collect();
                let mut outs: Vec<Vec<f64>> = p.outs.iter().map(|_| vec![0.0; n]).collect();
                k(&refs, &sc, &mut outs, n);
                for (o, v) in p.outs.iter().zip(outs) {
                    vars[*o as usize] = Some(Col::V(Arc::new(v)));
                }
            }
            None => {
                // in blocks of lanes: a value the program does not keep never leaves the cache
                const B: usize = 2048;
                let mut outs: Vec<Vec<f64>> = p.outs.iter().map(|_| vec![0.0; n]).collect();
                let mut tmp: Vec<Vec<f64>> = (0..p.nvars).map(|_| vec![0.0; B]).collect();
                let mut from = 0;
                while from < n {
                    let len = B.min(n - from);
                    for op in &p.ops {
                        let (k, a, b, c, d) = match op {
                            VOp::Op(k, a, b, c, d) => (Some(*k), *a, *b, *c, *d),
                            VOp::Copy(a, d) => (None, *a, VS::K(0.0), VS::K(0.0), *d),
                        };
                        let mut out = std::mem::take(&mut tmp[d as usize]);
                        {
                            let arg = |v: VS| -> vx::Arg {
                                match v {
                                    VS::In(j) => vx::Arg::C(&ins[j as usize][from..from + len]),
                                    VS::K(x) => vx::Arg::K(x),
                                    VS::Sc(j) => vx::Arg::K(sc[j as usize]),
                                    VS::Var(j) => vx::Arg::C(&tmp[j as usize][..len]),
                                    VS::Name(_) => vx::Arg::K(0.0),
                                }
                            };
                            match k {
                                Some(k) => vx::op(k, arg(a), arg(b), arg(c), &mut out[..len]),
                                None => match arg(a) {
                                    vx::Arg::C(s) => out[..len].copy_from_slice(s),
                                    vx::Arg::K(x) => out[..len].fill(x),
                                },
                            }
                        }
                        tmp[d as usize] = out;
                    }
                    for (o, v) in p.outs.iter().enumerate() {
                        outs[o][from..from + len].copy_from_slice(&tmp[*v as usize][..len]);
                    }
                    from += len;
                }
                for (o, v) in p.outs.iter().zip(outs) {
                    vars[*o as usize] = Some(Col::V(Arc::new(v)));
                }
            }
        }
        let col = |v: &VS| -> Col {
            match v {
                VS::In(j) => Col::V(ins[*j as usize].clone()),
                VS::K(x) => Col::K(*x),
                VS::Sc(j) => Col::K(sc[*j as usize]),
                VS::Var(j) => vars[*j as usize].clone().unwrap_or(Col::K(0.0)),
                VS::Name(_) => Col::K(0.0),
            }
        };
        let line = p.line.iter().map(|v| col(v)).collect();
        let writes = p.writes.iter().map(|(k, v)| (*k, col(&VS::Var(*v)))).collect();
        Some(Joined::Cols { n, line, writes })
    }

    // ---------- threads ----------

    fn lanes_threads(&mut self, texts: &[Rc<str>], lines: &[Vec<Val>]) -> Vec<(Vec<Val>, LaneW)> {
        self.flush();
        // the state at the fork, made again only when something was written since
        let n = self.it.names.len();
        let snap = match &self.snap {
            Some((e, k, s)) if *e == self.epoch && *k == n => s.clone(),
            _ => {
                let s = Arc::new(self.snapshot());
                self.snap = Some((self.epoch, n, s.clone()));
                s
            }
        };
        let ow = self.ow;
        let prog = self.prog;
        let jobs: Vec<(String, Vec<SV>)> = texts.iter().zip(lines).map(|(t, l)| (t.to_string(), l.iter().map(sv).collect())).collect();
        let outs: Vec<Option<LaneOut>> = std::thread::scope(|sc| {
            let hs: Vec<_> = jobs
                .into_iter()
                .map(|(text, line)| {
                    let snap = snap.clone();
                    std::thread::Builder::new().stack_size(WORKER_STACK).spawn_scoped(sc, move || worker_lane(prog, &snap, ow, line, &text)).ok()
                })
                .collect();
            hs.into_iter().map(|h| h.and_then(|h| h.join().ok())).collect()
        });
        let mut rows = Vec::with_capacity(outs.len());
        for (i, o) in outs.into_iter().enumerate() {
            let o = match o {
                Some(o) => o,
                None => self.fail(format!("lane {}: the thread failed", i)),
            };
            self.out.write_all(&o.out);
            if let Some(e) = o.err {
                self.fail(format!("lane {}: {}", i, e));
            }
            let args = o.args.iter().map(|v| self.val_of_sv(v)).collect();
            let mut w = LaneW::default();
            for (name, sl) in &o.names {
                let k = self.it.sym(name);
                let sl = self.slot_of_ss(sl);
                w.names.push((k, sl));
            }
            for (key, ms) in &o.repl {
                let ms = ms.as_ref().map(|ms| ms.iter().map(|m| self.macro_of_sm(m)).collect());
                w.repl.push((Rc::from(key.as_str()), ms));
            }
            rows.push((args, w));
        }
        rows
    }

    pub fn snapshot(&self) -> Snapshot {
        let names = self.it.names.iter().map(|n| n.to_string()).collect();
        let local = self.it.local.clone();
        let mut mem = Vec::new();
        for (i, v) in self.mem.iter().enumerate() {
            if let Some(v) = v {
                mem.push((i as u32, sv(v)));
            }
        }
        let mut nums = Vec::new();
        for (i, v) in self.nums.iter().enumerate() {
            if let Some(x) = v {
                nums.push((i as u32, *x));
            }
        }
        let mut lists = Vec::new();
        for (i, v) in self.lists.iter().enumerate() {
            if let Some(l) = v {
                lists.push((i as u32, sl(l)));
            }
        }
        let mut scripts = Vec::new();
        for (i, v) in self.scripts.iter().enumerate() {
            if let Some((s, b)) = v {
                scripts.push((i as u32, ss_of(s), *b));
            }
        }
        let repl = self.repl.iter().map(|(k, ms)| (k.to_string(), ms.iter().map(sm_of).collect())).collect();
        Snapshot { names, local, mem, nums, lists, scripts, repl, gen: self.gen, repl_num: self.repl_num, ow: self.ow }
    }
    pub fn restore(&mut self, s: &Snapshot) {
        self.epoch = self.epoch.wrapping_add(1);
        for n in &s.names {
            self.it.sym(n);
        }
        for (i, l) in s.local.iter().enumerate() {
            if i < self.it.local.len() {
                self.it.local[i] = *l;
            }
        }
        let n = self.it.names.len();
        self.mem = vec![None; n];
        self.nums = vec![None; n];
        self.lists = vec![None; n];
        self.scripts = vec![None; n];
        for (i, v) in &s.mem {
            self.mem[*i as usize] = Some(self.val_of_sv(v));
        }
        for (i, x) in &s.nums {
            self.nums[*i as usize] = Some(*x);
        }
        for (i, l) in &s.lists {
            let l = self.list_of_sl(l);
            self.lists[*i as usize] = Some(l);
        }
        for (i, sc, b) in &s.scripts {
            let sc = self.script_of_ss(sc);
            self.scripts[*i as usize] = Some((sc, *b));
        }
        self.repl.clear();
        for (k, ms) in &s.repl {
            let ms: Vec<Macro> = ms.iter().map(|m| self.macro_of_sm(m)).collect();
            self.repl.insert(Rc::from(k.as_str()), ms);
        }
        self.gen = s.gen;
        self.repl_num = s.repl_num;
        self.ow = s.ow;
    }
    fn val_of_sv(&mut self, v: &SV) -> Val {
        match v {
            SV::N(x) => Val::N(*x),
            SV::T(t) => Val::R(Rc::from(t.as_str())),
            SV::W(t) => {
                let k = self.it.sym(t);
                Val::K(Rc::from(t.as_str()), k)
            }
        }
    }
    fn list_of_sl(&mut self, l: &SL) -> List {
        match l {
            SL::N(v) => List::N(v.clone()),
            SL::V(v) => List::V(v.iter().map(|x| self.val_of_sv(x)).collect()),
        }
    }
    fn script_of_ss(&mut self, s: &SS) -> Script {
        match s {
            SS::C(f) => Script::C(*f),
            SS::N(f) => Script::N(*f),
            SS::T(t) => match self.prog.find(t) {
                Some(f) => Script::C(f),
                None => match self.code_of(t) {
                    Ok(u) => Script::U(u),
                    Err(_) => Script::U(Rc::new(Code { ops: Vec::new(), spans: Vec::new(), unit: NO_UNIT, src: Rc::from(t.as_str()) })),
                },
            },
        }
    }
    fn macro_of_sm(&mut self, m: &SM) -> Macro {
        let val = self.val_of_sv(&m.val);
        let code = m.code.as_ref().map(|c| self.script_of_ss(c));
        Macro { val, code, gen: m.gen }
    }
    fn slot_of_ss(&mut self, s: &SSlot) -> Slot {
        Slot {
            mem: s.mem.as_ref().map(|v| self.val_of_sv(v)),
            num: s.num,
            list: s.list.as_ref().map(|l| self.list_of_sl(l)),
            script: s.script.as_ref().map(|(sc, b)| (self.script_of_ss(sc), *b)),
        }
    }

    // ---------- after the join ----------

    fn joined_or(&mut self, who: &str) -> Rc<Joined> {
        match &self.joined {
            Some(j) => j.clone(),
            None => self.fail(format!("{}: no joined lanes", who)),
        }
    }
    pub fn end_view(&mut self) {
        if let Some(v) = self.view.take() {
            for (s, old) in v.saved.into_iter().rev() {
                self.put_slot(s, old);
            }
            for (k, _) in &v.lane.repl {
                let _ = k;
            }
            self.gen += 1;
        }
    }
    pub fn view_lane(&mut self, i: usize) {
        self.end_view();
        let j = self.joined_or("lane");
        if i >= j.len() {
            let m = format!("lane: no lane {}, there are {}", i, j.len());
            self.fail(m)
        }
        let w = j.writes(i);
        let saved = self.apply_writes(&w);
        self.view = Some(View { saved, lane: w });
    }

    pub fn each_step(&mut self, idx: usize, run: Rc<RefCell<EachRun>>) {
        let mut r = run.borrow_mut();
        match r.kind {
            EachKind::Reduce => {
                if r.i < r.n {
                    let i = r.i;
                    r.i += 1;
                    let q = r.q.clone();
                    drop(r);
                    let j = self.joined_or("reduce");
                    let line = j.line(i);
                    self.args.extend(line);
                    let f = self.src_frame(&q);
                    self.fs.push(f);
                } else {
                    drop(r);
                    self.end_frame_pub(idx);
                }
            }
            EachKind::Lanewise => {
                if r.running {
                    r.running = false;
                    let log = self.lanelog.take().unwrap_or_default();
                    let w = self.close_log(log);
                    let args = std::mem::take(&mut self.args);
                    r.rows.push((args, w));
                }
                if r.i < r.n {
                    let i = r.i;
                    r.i += 1;
                    r.running = true;
                    let q = r.q.clone();
                    let r_ow0 = r.ow0;
                    drop(r);
                    let j = self.joined_or("lanewise");
                    self.lanelog = Some(Box::new(LaneLog::default()));
                    self.lane_at = Some(i);
                    self.ow = r_ow0;
                    let w = j.writes(i);
                    self.apply_writes(&w);
                    self.args = j.line(i);
                    let f = self.src_frame(&q);
                    self.fs.push(f);
                } else {
                    self.lanelog = r.outer_log.take();
                    self.lane_at = r.outer_at;
                    self.ow = r.ow0;
                    self.args = std::mem::take(&mut r.base);
                    let rows = std::mem::take(&mut r.rows);
                    drop(r);
                    self.joined = Some(Rc::new(Joined::Rows(rows)));
                    self.end_frame_pub(idx);
                }
            }
        }
    }

    /// After an error: every fork the error left running is undone, down to `depth`.
    pub fn unwind_lanes(&mut self, keep: usize) {
        while self.fs.len() > keep {
            let fr = self.fs.pop().unwrap();
            let d = self.fs.len();
            self.clear_locals_pub(d);
            match fr.body {
                Body::Lanes(r) => {
                    if let Some(log) = self.lanelog.take() {
                        self.close_log(log);
                    }
                    let mut r = r.borrow_mut();
                    self.lanelog = r.outer_log.take();
                    self.lane_at = r.outer_at;
                }
                Body::Each(r) => {
                    let mut r = r.borrow_mut();
                    if r.running {
                        if let Some(log) = self.lanelog.take() {
                            self.close_log(log);
                        }
                    }
                    self.lanelog = r.outer_log.take();
                    self.lane_at = r.outer_at;
                }
                _ => {}
            }
        }
    }
}

// ---------- the words ----------

pub fn w_lanes(rt: &mut Rt) {
    let j = rt.joined_or("lanes");
    rt.args.push(Val::N(j.len() as f64));
}
pub fn w_lane(rt: &mut Rt) {
    let x = rt.pop_num("lane");
    if !(x >= 0.0 && x.fract() == 0.0) {
        rt.fail(format!("lane: a lane is a whole number >= 0, got {}", fmt_num(x)))
    }
    let i = x as usize;
    rt.view_lane(i);
    let j = rt.joined_or("lane");
    let line = j.line(i);
    rt.args.extend(line);
}
pub fn w_endlane(rt: &mut Rt) {
    rt.end_view();
}
pub fn w_collect(rt: &mut Rt) {
    let j = rt.joined_or("collect");
    for i in 0..j.len() {
        let l = j.line(i);
        rt.args.extend(l);
    }
}
pub fn w_into(rt: &mut Rt) {
    let v = rt.pop("into");
    rt.args.push(v);
    let k = rt.arg_sym(rt.args.len() - 1);
    rt.args.pop();
    let j = rt.joined_or("into");
    let out = match &*j {
        // columns go into the list as numbers, lane by lane
        Joined::Cols { n, line, .. } => {
            if line.len() == 1 {
                match &line[0] {
                    Col::V(v) => List::N(v.clone()),
                    Col::K(x) => List::nums_of(vec![*x; *n]),
                }
            } else {
                let mut o = Vec::with_capacity(n * line.len());
                for i in 0..*n {
                    for c in line {
                        o.push(c.at(i));
                    }
                }
                List::nums_of(o)
            }
        }
        Joined::Rows(r) => {
            let mut o: Vec<Val> = Vec::with_capacity(r.len());
            for (l, _) in r {
                o.extend(l.iter().cloned());
            }
            List::from_vals(o)
        }
    };
    rt.note(k);
    rt.set_list(k, out);
}
pub fn w_reduce(rt: &mut Rt) {
    let q = rt.pop_text("reduce");
    let j = rt.joined_or("reduce");
    if j.len() == 0 {
        return;
    }
    let first = j.line(0);
    rt.args.extend(first);
    let run = EachRun { kind: EachKind::Reduce, q: Rc::from(q.as_str()), i: 1, n: j.len(), running: false, rows: Vec::new(), base: Vec::new(), outer_log: None, outer_at: None, ow0: rt.ow };
    let bound = rt.bound;
    rt.fs.push(Frame { body: Body::Each(Rc::new(RefCell::new(run))), pc: 0, bound });
}
pub fn w_lanewise(rt: &mut Rt) {
    let q = rt.pop_text("lanewise");
    let j = rt.joined_or("lanewise");
    rt.end_view();
    let base = std::mem::take(&mut rt.args);
    let run = EachRun { kind: EachKind::Lanewise, q: Rc::from(q.as_str()), i: 0, n: j.len(), running: false, rows: Vec::new(), base, outer_log: rt.lanelog.take(), outer_at: rt.lane_at, ow0: rt.ow };
    let bound = rt.bound;
    rt.fs.push(Frame { body: Body::Each(Rc::new(RefCell::new(run))), pc: 0, bound });
}
pub fn w_try(rt: &mut Rt) {
    let h = rt.pop_text("try");
    let body = rt.pop_text("try");
    rt.args.clear();
    let bound = rt.bound;
    let frames = rt.frames.len();
    rt.fs.push(Frame { body: Body::Catch { h: Rc::from(h.as_str()), frames }, pc: 0, bound });
    rt.run_src(&body);
}

pub static WORDS: &[(&str, NativeFn)] = &[
    ("lanes", w_lanes),
    ("lane", w_lane),
    ("endlane", w_endlane),
    ("collect", w_collect),
    ("into", w_into),
    ("reduce", w_reduce),
    ("lanewise", w_lanewise),
    ("try", w_try),
];

// ---------- what crosses to a thread ----------

#[derive(Clone)]
pub enum SV {
    N(f64),
    T(String),
    W(String),
}
#[derive(Clone)]
pub enum SS {
    C(Chunk),
    N(NativeFn),
    T(String),
}
#[derive(Clone)]
pub struct SM {
    pub val: SV,
    pub code: Option<SS>,
    pub gen: u64,
}
#[derive(Clone, Default)]
pub struct SSlot {
    pub mem: Option<SV>,
    pub num: Option<f64>,
    pub list: Option<SL>,
    pub script: Option<(SS, u64)>,
}
pub struct Snapshot {
    pub names: Vec<String>,
    pub local: Vec<bool>,
    pub mem: Vec<(u32, SV)>,
    pub nums: Vec<(u32, f64)>,
    pub lists: Vec<(u32, SL)>,
    pub scripts: Vec<(u32, SS, u64)>,
    pub repl: Vec<(String, Vec<SM>)>,
    pub gen: u64,
    pub repl_num: bool,
    pub ow: bool,
}
pub struct LaneOut {
    pub args: Vec<SV>,
    pub names: Vec<(String, SSlot)>,
    pub repl: Vec<(String, Option<Vec<SM>>)>,
    pub out: Vec<u8>,
    pub err: Option<String>,
}

fn sv(v: &Val) -> SV {
    match v {
        Val::N(x) => SV::N(*x),
        Val::L(s) => SV::T(s.to_string()),
        Val::R(s) => SV::T(s.to_string()),
        Val::W(s, _) => SV::W(s.to_string()),
        Val::K(s, _) => SV::W(s.to_string()),
    }
}
/// A list crossing to a thread: numbers stay numbers.
#[derive(Clone)]
pub enum SL {
    N(Arc<Vec<f64>>),
    V(Vec<SV>),
}
fn sl(l: &List) -> SL {
    match l {
        List::N(v) => SL::N(v.clone()),
        List::V(v) => SL::V(v.iter().map(sv).collect()),
    }
}
fn ss_of(s: &Script) -> SS {
    match s {
        Script::C(f) => SS::C(*f),
        Script::N(f) => SS::N(*f),
        Script::U(u) => SS::T(u.src.to_string()),
    }
}
fn sm_of(m: &Macro) -> SM {
    SM { val: sv(&m.val), code: m.code.as_ref().map(ss_of), gen: m.gen }
}
fn sslot(s: &Slot) -> SSlot {
    SSlot {
        mem: s.mem.as_ref().map(sv),
        num: s.num,
        list: s.list.as_ref().map(sl),
        script: s.script.as_ref().map(|(sc, b)| (ss_of(sc), *b)),
    }
}

/// A lane on its own thread: a machine with the state of the fork, the lane's line, the
/// lane's text; what it printed, wrote and left on its line come back.
fn worker_lane(prog: &'static Program, snap: &Snapshot, ow: bool, line: Vec<SV>, text: &str) -> LaneOut {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let mut w = Rt::with_out(prog, Out::Buf(buf.clone()));
    w.stdin = false;
    w.restore(snap);
    w.ow = ow;
    w.args = line.iter().map(|v| w.val_of_sv(v)).collect();
    w.lanelog = Some(Box::new(LaneLog::default()));
    let st = w.run_text(text);
    w.flush();
    let err = match st {
        Status::Error(e) => Some(e),
        Status::Exit(c) => Some(format!("<--: {}", c)),
        _ => None,
    };
    let args = w.args.iter().map(sv).collect();
    let log = w.lanelog.take().unwrap_or_default();
    let mut names = Vec::new();
    for (s, _) in &log.old {
        let sl = w.slot(*s);
        names.push((w.it.name(*s).to_string(), sslot(&sl)));
    }
    let repl = log.repl_old.iter().map(|(k, _)| (k.to_string(), w.repl.get(&**k).map(|ms| ms.iter().map(sm_of).collect()))).collect();
    let out = buf.lock().map(|b| b.clone()).unwrap_or_default();
    LaneOut { args, names, repl, out, err }
}

/// The compiler's side: Rust for a vector program, a fused loop over SIMD registers in an
/// AVX version and a baseline version, chosen when it runs.
pub fn kernel_rust(p: &VProg, i: usize) -> String {
    use std::fmt::Write as _;
    let mut b = String::new();
    let lit = |x: f64| -> String {
        if x.is_nan() {
            "f64::NAN".into()
        } else if x == f64::INFINITY {
            "f64::INFINITY".into()
        } else if x == f64::NEG_INFINITY {
            "f64::NEG_INFINITY".into()
        } else {
            format!("{:?}", x)
        }
    };
    let src = |v: &VS| -> String {
        match v {
            VS::In(j) => format!("T::ld(i{}.add(i))", j),
            VS::K(x) => format!("T::sp({})", lit(*x)),
            VS::Sc(j) => format!("s{}", j),
            VS::Var(j) => format!("v{}", j),
            VS::Name(_) => "T::sp(0.0)".into(),
        }
    };
    let _ = writeln!(b, "#[inline(always)]\nunsafe fn kb{}<T: vx::V>(ins: &[&[f64]], sc: &[f64], outs: &mut [Vec<f64>], from: usize, to: usize) {{", i);
    for j in 0..p.m {
        let _ = writeln!(b, "    let i{0} = ins[{0}].as_ptr();", j);
    }
    for j in 0..p.scalars.len() {
        let _ = writeln!(b, "    let s{0} = T::sp(sc[{0}]);", j);
    }
    for (o, _) in p.outs.iter().enumerate() {
        let _ = writeln!(b, "    let o{0} = outs[{0}].as_mut_ptr();", o);
    }
    let _ = writeln!(b, "    let mut i = from;\n    while i + T::W <= to {{");
    for op in &p.ops {
        match op {
            VOp::Copy(a, d) => {
                let _ = writeln!(b, "        let v{} = {};", d, src(a));
            }
            VOp::Op(k, x, y, z, d) => {
                use vx::K;
                let e = match k {
                    K::Sel => format!("T::sel({}, {}, {})", src(x), src(y), src(z)),
                    _ => {
                        let m = match k {
                            K::Add => "add",
                            K::Sub => "sub",
                            K::Mul => "mul",
                            K::Div => "div",
                            K::Pow => "pow",
                            K::Rem => "rem",
                            K::Eq => "veq",
                            K::Ne => "vne",
                            K::NumEq => "ceq",
                            K::Lt => "clt",
                            K::Gt => "cgt",
                            K::Le => "cle",
                            K::Ge => "cge",
                            K::And => "and",
                            K::AndNot => "andnot",
                            K::Sel => unreachable!(),
                        };
                        format!("{}.{}({})", src(x), m, src(y))
                    }
                };
                let _ = writeln!(b, "        let v{} = {};", d, e);
            }
        }
    }
    for (o, v) in p.outs.iter().enumerate() {
        let _ = writeln!(b, "        v{}.st(o{}.add(i));", v, o);
    }
    let _ = writeln!(b, "        i += T::W;\n    }}\n}}");
    for (name, feat, ty, w) in [("kz", "avx512f", "Avx512", 8), ("kx", "avx", "Avx", 4)] {
        let _ = writeln!(
            b,
            "#[cfg(target_arch = \"x86_64\")]\n#[target_feature(enable = \"{feat}\")]\nunsafe fn {name}{i}(ins: &[&[f64]], sc: &[f64], outs: &mut [Vec<f64>], n: usize) {{\n    let to = n - n % {w};\n    kb{i}::<vx::{ty}>(ins, sc, outs, 0, to);\n    kb{i}::<vx::S1>(ins, sc, outs, to, n);\n}}"
        );
    }
    let _ = writeln!(
        b,
        "fn k{0}(ins: &[&[f64]], sc: &[f64], outs: &mut [Vec<f64>], n: usize) {{\n    for c in ins.iter() {{ assert!(c.len() >= n); }}\n    for c in outs.iter() {{ assert!(c.len() >= n); }}\n    unsafe {{\n        #[cfg(target_arch = \"x86_64\")]\n        {{\n            match vx::level_code() {{\n                4 => return kz{0}(ins, sc, outs, n),\n                3 => return kx{0}(ins, sc, outs, n),\n                2 => {{ let to = n - n % 2; kb{0}::<vx::Sse>(ins, sc, outs, 0, to); return kb{0}::<vx::S1>(ins, sc, outs, to, n); }}\n                _ => return kb{0}::<vx::S1>(ins, sc, outs, 0, n),\n            }}\n        }}\n        #[cfg(target_arch = \"aarch64\")]\n        {{\n            if vx::level_code() == 1 {{ let to = n - n % 2; kb{0}::<vx::Neon>(ins, sc, outs, 0, to); return kb{0}::<vx::S1>(ins, sc, outs, to, n); }}\n            return kb{0}::<vx::S1>(ins, sc, outs, 0, n);\n        }}\n        #[allow(unreachable_code)]\n        kb{0}::<vx::S1>(ins, sc, outs, 0, n)\n    }}\n}}",
        i
    );
    b
}
