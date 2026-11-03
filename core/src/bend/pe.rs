// The s machine run at build time over values that are known, or that come from outside and
// are not known until the program runs. Known work is done here; every op on an unknown value
// is written out as a statement. A choice on an unknown is both paths, joined again where they
// meet. A script entered again after it wrote statements is a function, made general over the
// names that differ between the two entries.
use super::ir::*;
use crate::rt::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Num,
    Str,
    Any,
}

impl Kind {
    pub fn ty(self) -> Ty {
        match self {
            Kind::Num => Ty::F,
            Kind::Str => Ty::S,
            Kind::Any => Ty::V,
        }
    }
    fn join(a: Kind, b: Kind) -> Kind {
        if a == b { a } else { Kind::Any }
    }
    fn within(a: Kind, b: Kind) -> bool {
        a == b || b == Kind::Any
    }
}

#[derive(Clone, Copy, Debug)]
pub struct D {
    pub v: u32,
    pub k: Kind,
    /// the values it can hold, when they are known: an index into Pe::doms, 0 for none
    pub m: u32,
}

/// A value: known at build time, or held in a variable.
#[derive(Clone, Debug)]
pub enum A {
    K(Val),
    D(D),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AB {
    K(bool),
    D(u32),
}

#[derive(Clone, Debug)]
pub enum AL {
    K(Rc<Vec<A>>),
    D(u32),
    /// a list held in a variable, then known items: the line over a lower part from input
    C(u32, Rc<Vec<A>>),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Nat {
    Lanes,
    Lane,
    EndLane,
    Collect,
    Into,
    Reduce,
    Lanewise,
    Try,
}

#[derive(Clone)]
struct Mac {
    val: Val,
    code: Option<Rc<Code>>,
    nat: Option<Nat>,
    gen: u64,
}

/// A name's storage: the word, and the frame depth for a `:` name.
type Nm = (Sym, u32);
const GL: u32 = u32::MAX;
/// What a name holds on a path that did not set it, where another path did.
const ABSENT: &str = "\u{1}";

#[derive(Clone, Default)]
struct Slot {
    mem: Option<A>,
    num: Option<A>,
    list: Option<AL>,
    script: Option<(Rc<Code>, u64)>,
}

#[derive(Clone, Default)]
struct LW {
    names: Vec<(Nm, Slot)>,
    repl: Vec<(Rc<str>, Option<Rc<Vec<Mac>>>)>,
}

#[derive(Clone)]
struct Row {
    line: Vec<A>,
    w: LW,
}

#[derive(Clone)]
struct JD {
    n: A,
    /// one list per value of a lane's line
    lines: Vec<u32>,
    /// one list per name a lane writes: the name, whether it is text (else a number)
    writes: Vec<(Nm, bool, u32)>,
}

#[derive(Clone, Default)]
struct Log {
    seen: Vec<Nm>,
    old: Vec<(Nm, Slot)>,
    repl_seen: Vec<Rc<str>>,
    repl_old: Vec<(Rc<str>, Option<Rc<Vec<Mac>>>)>,
}

/// Statements, newest last, shared between copies of a path until one of them adds more.
#[derive(Clone, Default)]
struct Buf(Option<Rc<BufNode>>);
struct BufNode {
    s: Stmt,
    prev: Buf,
}
impl Buf {
    fn push(&mut self, s: Stmt) {
        let prev = std::mem::take(self);
        *self = Buf(Some(Rc::new(BufNode { s, prev })));
    }
    fn to_vec(&self) -> Vec<Stmt> {
        let mut v = Vec::new();
        let mut cur = &self.0;
        while let Some(n) = cur {
            v.push(n.s.clone());
            cur = &n.prev.0;
        }
        v.reverse();
        v
    }
}

/// A reply a worker gives back: code main runs on `>t`, or the error it stopped with.
#[derive(Clone)]
enum Reply {
    Code(A),
    Err(String),
}

/// A worker of `t`: its own machine, the replies main has not taken yet, and whether it
/// stopped with an error.
#[derive(Clone)]
struct Wk {
    st: St,
    replies: std::collections::VecDeque<Reply>,
    dead: bool,
    /// the thread running its last message, and what its block gives back
    pending: Option<(u32, Vec<(u32, Ty)>)>,
}

/// One path of the machine.
#[derive(Clone)]
pub struct St {
    args: Vec<A>,
    /// the line's lower part when its length comes from input: a list before args
    low: Option<u32>,
    ow: AB,
    mem: Rc<BTreeMap<Nm, A>>,
    nums: Rc<BTreeMap<Nm, A>>,
    lists: Rc<BTreeMap<Nm, AL>>,
    scripts: Rc<BTreeMap<Nm, (Rc<Code>, u64)>>,
    repl: Rc<BTreeMap<Rc<str>, Rc<Vec<Mac>>>>,
    repl_num: bool,
    gen: u64,
    /// the lines `(` put aside: each one's lower part from input, and its values
    frames: Vec<(Option<u32>, Vec<A>)>,
    /// frames on the machine's stack
    fs: u32,
    /// names whose text (0) or number (1) a path may not have set: they hold ABSENT there
    maybe: Rc<std::collections::BTreeSet<(Nm, u8)>>,
    joined: Option<Rc<Vec<Row>>>,
    /// lanes over a list from input, held at the join as lists across the lanes
    jdyn: Option<Rc<JD>>,
    view: Option<Rc<Vec<(Nm, Slot)>>>,
    log: Option<Rc<Log>>,
    lane_at: Option<usize>,
    /// the counter and the count of each running repeat
    reps: Vec<(A, A)>,
    code: Buf,
    end: Option<End>,
    /// the workers of `t`, by index; a closed one is None
    workers: Rc<Vec<Option<Rc<Wk>>>>,
    /// lanes a join held were let go at a loop head: reading them before the next fork does
    /// not build
    gone: bool,
}

pub enum Stop {
    /// The program has no Bend form here.
    Build(String),
    /// Make the head a function over this state.
    Restart(u64, Box<St>),
    /// The path ended: its `end` says how.
    Dead(Box<St>),
}
type R<T> = Result<T, Stop>;

enum Fl {
    End,
    Done,
    Tail(Rc<Code>, u64),
    Rep(Nm, A),
    /// a choice ran both paths to the end of the frame and joined them
    Exited,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum Key {
    Code(usize),
    Rep(Nm),
}

#[derive(Clone)]
enum HB {
    Code(Rc<Code>, u64),
    Rep(Nm, bool),
}

enum HK {
    Inline,
    Fun(usize),
}

struct Head {
    id: u64,
    key: Key,
    kind: HK,
    saved: Option<St>,
    body: HB,
    dyn0: u64,
    steps0: u64,
    /// the part of the state the head owns
    b: Base,
    /// rounds run here because a round set names the one before had not
    peel: u32,
    rounds: u64,
    /// `(` frames open at the head: the same code run inside more frames is another loop
    nf: usize,
}

pub struct FunInfo {
    pub key: String,
    g: St,
    gb: Base,
    exit: Ex0,
    pub fun: Option<Fun>,
    body: HB,
    used: bool,
    provisional: bool,
}

/// Static work a loop may do before it is made a function.
const LOOP_STEPS: u64 = 2_000_000;
/// Calls of one script nested at build time before it is made a function.
const MAX_NEST: usize = 1000;
/// Lanes a fork over a known list runs at build time; a longer list runs as a map.
const FORK_MAX: usize = 64;
/// Items a known list holds before it is held in a variable.
const KLIST: usize = 256;
/// Values a variable's known values hold at most.
const DOM_MAX: usize = 64;

pub struct Pe {
    pub it: Interner,
    units: FxMap<String, Rc<Code>>,
    pub tys: Vec<Ty>,
    pub funs: Vec<FunInfo>,
    memo: FxMap<String, usize>,
    /// the parts of each text built by `~name~` from input: a reply run as code is run from them
    texts: FxMap<u32, Vec<A>>,
    heads: Vec<Head>,
    fun_stack: Vec<usize>,
    next_head: u64,
    dyn_ev: u64,
    steps: u64,
    pruned: bool,
    pub lib: Vec<(String, String)>,
    pub site: (Rc<str>, u32),
    pub main_src: Rc<str>,
    pub keysets: Vec<Vec<String>>,
    pub lmaps: Vec<(String, Vec<(String, Ex)>)>,
    doms: Vec<Rc<Vec<Val>>>,
}

fn same_val(a: &Val, b: &Val) -> bool {
    match (a, b) {
        (Val::N(x), Val::N(y)) => x.to_bits() == y.to_bits(),
        (Val::N(_), _) | (_, Val::N(_)) => false,
        _ => a.text() == b.text(),
    }
}

fn kind_of(a: &A) -> Kind {
    match a {
        A::K(Val::N(_)) => Kind::Num,
        A::K(_) => Kind::Str,
        A::D(d) => d.k,
    }
}

/// Characters a number written as s writes it can hold.
fn numberish(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '.' | '-' | 'i' | 'n' | 'f' | 'N' | 'a')
}

/// A line as one list: its lower part from input, if any, then its values.
fn line_al(low: Option<u32>, args: &[A]) -> AL {
    match low {
        None => AL::K(Rc::new(args.to_vec())),
        Some(v) if args.is_empty() => AL::D(v),
        Some(v) => AL::C(v, Rc::new(args.to_vec())),
    }
}
fn same_lines(a: &St, b: &St) -> bool {
    a.args.len() == b.args.len() && a.low == b.low && a.frames.len() == b.frames.len() && a.frames.iter().zip(b.frames.iter()).all(|(x, y)| x.0 == y.0 && x.1.len() == y.1.len())
}
fn al_line(l: AL) -> (Option<u32>, Vec<A>) {
    match l {
        AL::K(xs) => (None, (*xs).clone()),
        AL::D(v) => (Some(v), Vec::new()),
        AL::C(v, xs) => (Some(v), (*xs).clone()),
    }
}

impl St {
    fn emit(&mut self, s: Stmt) {
        self.code.push(s)
    }
    fn depth(&self) -> u32 {
        self.fs.saturating_sub(1)
    }
}

pub fn build_err<T>(m: String) -> R<T> {
    Err(Stop::Build(m))
}

impl Pe {
    pub fn new(lib: Vec<(String, String)>, main_src: &str) -> Pe {
        Pe {
            it: Interner::new(),
            units: FxMap::default(),
            tys: Vec::new(),
            funs: Vec::new(),
            memo: FxMap::default(),
            texts: FxMap::default(),
            heads: Vec::new(),
            fun_stack: Vec::new(),
            next_head: 1,
            dyn_ev: 0,
            steps: 0,
            pruned: false,
            lib,
            site: (Rc::from(main_src), 0),
            main_src: Rc::from(main_src),
            keysets: Vec::new(),
            lmaps: Vec::new(),
            doms: vec![Rc::new(Vec::new())],
        }
    }

    pub fn var(&mut self, t: Ty) -> u32 {
        self.tys.push(t);
        (self.tys.len() - 1) as u32
    }
    fn bind(&mut self, st: &mut St, t: Ty, e: Ex) -> u32 {
        let v = self.var(t);
        st.emit(Stmt::Let(v, t, e));
        self.dyn_ev += 1;
        v
    }
    fn bind_a(&mut self, st: &mut St, k: Kind, e: Ex) -> A {
        let v = self.bind(st, k.ty(), e);
        A::D(D { v, k, m: 0 })
    }

    fn here(&self) -> String {
        let (src, at) = &self.site;
        let pre: String = src.chars().take(*at as usize).collect();
        let line = pre.matches('\n').count() + 1;
        let word: String = src.chars().skip(*at as usize).take_while(|c| !c.is_whitespace()).collect();
        if Rc::ptr_eq(src, &self.main_src) || **src == *self.main_src {
            format!("line {}, `{}`", line, word)
        } else {
            let first: String = src.chars().take(40).collect::<String>().replace('\n', " ");
            format!("`{}` in the text `{}…`", word, first)
        }
    }
    fn no<T>(&self, what: &str) -> R<T> {
        build_err(format!("{} ({})", what, self.here()))
    }

    // ---------- values ----------

    /// The values a value can be, when they are known.
    fn dom_of(&self, a: &A) -> Option<Vec<Val>> {
        match a {
            A::K(v) => Some(vec![v.clone()]),
            A::D(d) if d.m > 0 => Some((*self.doms[d.m as usize]).clone()),
            _ => None,
        }
    }
    fn mk_dom(&mut self, vals: Vec<Val>) -> u32 {
        let mut d: Vec<Val> = Vec::new();
        for v in vals {
            if !d.iter().any(|x| same_val(x, &v)) {
                d.push(v);
            }
        }
        if d.is_empty() || d.len() > DOM_MAX {
            return 0;
        }
        d.sort_by(|a, b| a.text().cmp(&b.text()));
        if let Some(i) = self.doms.iter().position(|x| x.len() == d.len() && x.iter().zip(d.iter()).all(|(a, b)| same_val(a, b))) {
            return i as u32;
        }
        self.doms.push(Rc::new(d));
        (self.doms.len() - 1) as u32
    }

    pub fn ex_num(&self, a: &A) -> Ex {
        match a {
            A::K(v) => Ex::Num(v.num()),
            A::D(D { v, k: Kind::Num, .. }) => Ex::Var(*v),
            A::D(D { v, k: Kind::Str, .. }) => Ex::App(P::Parse, vec![Ex::Var(*v)]),
            A::D(D { v, k: Kind::Any, .. }) => Ex::App(P::VNum, vec![Ex::Var(*v)]),
        }
    }
    pub fn ex_text(&self, a: &A) -> Ex {
        match a {
            A::K(v) => Ex::Str(Rc::from(v.text().as_ref())),
            A::D(D { v, k: Kind::Num, .. }) => Ex::App(P::Fmt, vec![Ex::Var(*v)]),
            A::D(D { v, k: Kind::Str, .. }) => Ex::Var(*v),
            A::D(D { v, k: Kind::Any, .. }) => Ex::App(P::VText, vec![Ex::Var(*v)]),
        }
    }
    pub fn ex_v(&self, a: &A) -> Ex {
        match a {
            A::K(Val::N(x)) => Ex::App(P::VN, vec![Ex::Num(*x)]),
            A::K(Val::L(t)) if *t == ABSENT => Ex::App(P::VX, vec![]),
            A::K(v) => Ex::App(P::VT, vec![Ex::Str(Rc::from(v.text().as_ref()))]),
            A::D(D { v, k: Kind::Num, .. }) => Ex::App(P::VN, vec![Ex::Var(*v)]),
            A::D(D { v, k: Kind::Str, .. }) => Ex::App(P::VT, vec![Ex::Var(*v)]),
            A::D(D { v, k: Kind::Any, .. }) => Ex::Var(*v),
        }
    }
    fn ex_as(&self, a: &A, k: Kind) -> Ex {
        match k {
            Kind::Num => self.ex_num(a),
            Kind::Str => self.ex_text(a),
            Kind::Any => self.ex_v(a),
        }
    }
    fn ex_b(&self, b: AB) -> Ex {
        match b {
            AB::K(x) => Ex::Bool(x),
            AB::D(v) => Ex::Var(v),
        }
    }
    fn ex_list(&self, l: &AL) -> Ex {
        match l {
            AL::D(v) => Ex::Var(*v),
            AL::C(v, xs) => {
                let mut e = Ex::Var(*v);
                for x in xs.iter() {
                    e = Ex::App(P::LPush, vec![e, self.ex_v(x)]);
                }
                e
            }
            AL::K(xs) => {
                let mut e = Ex::App(P::LNew, vec![]);
                for x in xs.iter() {
                    e = Ex::App(P::LPush, vec![e, self.ex_v(x)]);
                }
                e
            }
        }
    }
    /// The text of values joined with spaces, as one value: known, or a variable.
    fn joined_val(&mut self, st: &mut St, xs: &[A]) -> A {
        if xs.iter().all(|x| matches!(x, A::K(_))) {
            let mut s = String::new();
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    s.push(' ');
                }
                if let A::K(v) = x {
                    s.push_str(&v.text());
                }
            }
            return A::K(Val::of(s));
        }
        let mut parts = Vec::new();
        let mut text_only = false;
        for (i, x) in xs.iter().enumerate() {
            if i > 0 {
                parts.push(Ex::Str(Rc::from(" ")));
                text_only = true;
            }
            if let A::K(v) = x {
                if v.text().chars().any(|c| !numberish(c)) {
                    text_only = true;
                }
            }
            parts.push(self.ex_text(x));
        }
        let e = Ex::App(P::Cat, parts);
        if text_only {
            self.bind_a(st, Kind::Str, e)
        } else {
            self.bind_a(st, Kind::Any, Ex::App(P::VOf, vec![e]))
        }
    }

    // ---------- names ----------

    fn l(&mut self, st: &St, k: Sym) -> R<Nm> {
        if self.it.is_local(k) {
            Ok((k, st.depth()))
        } else {
            Ok((k, GL))
        }
    }
    fn word_sym(&mut self, a: &A) -> R<Sym> {
        match a {
            A::K(Val::W(_, k)) | A::K(Val::K(_, k)) => Ok(*k),
            A::K(v) => {
                let t = v.text().into_owned();
                Ok(self.it.sym(&t))
            }
            A::D(_) => self.no("a name that comes from input"),
        }
    }
    fn arg_sym(&mut self, st: &St, i: usize) -> R<Nm> {
        let k = self.word_sym(&st.args[i])?;
        self.l(st, k)
    }
    fn dest(&mut self, st: &St, i: usize, def: Sym) -> R<Nm> {
        if st.args.len() > i {
            self.arg_sym(st, i)
        } else {
            Ok((def, GL))
        }
    }
    fn note(&mut self, st: &mut St, s: Nm) {
        if let Some(log) = &st.log {
            if !log.seen.contains(&s) {
                let old = self.slot(st, s);
                let l = Rc::make_mut(st.log.as_mut().unwrap());
                l.seen.push(s);
                l.old.push((s, old));
            }
        }
    }
    fn note_repl(&mut self, st: &mut St, key: &str) {
        if let Some(log) = &st.log {
            if !log.repl_seen.iter().any(|k| &**k == key) {
                let k: Rc<str> = Rc::from(key);
                let old = st.repl.get(key).cloned();
                let l = Rc::make_mut(st.log.as_mut().unwrap());
                l.repl_seen.push(k.clone());
                l.repl_old.push((k, old));
            }
        }
    }
    fn slot(&self, st: &St, s: Nm) -> Slot {
        Slot { mem: st.mem.get(&s).cloned(), num: st.nums.get(&s).cloned(), list: st.lists.get(&s).cloned(), script: st.scripts.get(&s).cloned() }
    }
    fn put_slot(&self, st: &mut St, s: Nm, sl: Slot) {
        fn put<T: Clone>(m: &mut Rc<BTreeMap<Nm, T>>, s: Nm, x: Option<T>) {
            match x {
                Some(x) => {
                    Rc::make_mut(m).insert(s, x);
                }
                None => {
                    if m.contains_key(&s) {
                        Rc::make_mut(m).remove(&s);
                    }
                }
            }
        }
        put(&mut st.mem, s, sl.mem);
        put(&mut st.nums, s, sl.num);
        put(&mut st.lists, s, sl.list);
        put(&mut st.scripts, s, sl.script);
    }
    fn nset(&mut self, st: &mut St, s: Nm, x: A) {
        self.note(st, s);
        Rc::make_mut(&mut st.nums).insert(s, x);
    }
    fn mset(&mut self, st: &mut St, s: Nm, v: A) {
        self.note(st, s);
        Rc::make_mut(&mut st.mem).insert(s, v);
    }
    fn get_val(&mut self, st: &mut St, s: Nm) -> R<A> {
        if let Some(v) = st.mem.get(&s).cloned() {
            if !st.maybe.contains(&(s, 0)) {
                return Ok(v);
            }
            match &v {
                A::K(Val::L(t)) if *t == ABSENT => {}
                A::K(_) => return Ok(v),
                A::D(_) => {
                    let fb = self.nums_or_404(st, s)?;
                    return Ok(self.or_absent(st, &v, &fb));
                }
            }
        }
        self.nums_or_404(st, s)
    }
    fn nums_or_404(&mut self, st: &mut St, s: Nm) -> R<A> {
        let none = A::K(Val::L("404"));
        if let Some(v) = st.nums.get(&s).cloned() {
            if !st.maybe.contains(&(s, 1)) {
                return Ok(v);
            }
            return Ok(match &v {
                A::K(Val::L(t)) if *t == ABSENT => none,
                A::K(_) => v,
                A::D(_) => self.or_absent(st, &v, &none),
            });
        }
        if s == (S_Q, GL) {
            return self.no("`?~`, a random number, is not built to Bend yet");
        }
        Ok(none)
    }
    /// v, or fb where v holds ABSENT.
    fn or_absent(&mut self, st: &mut St, v: &A, fb: &A) -> A {
        if kind_of(v) == Kind::Num {
            return v.clone();
        }
        if let Some(d) = self.dom_of(v) {
            let has = d.iter().any(|x| matches!(x, Val::L(t) if *t == ABSENT));
            if !has {
                return v.clone();
            }
            if d.len() == 1 {
                return fb.clone();
            }
        }
        let c = match kind_of(v) {
            Kind::Any => Ex::App(P::IsX, vec![self.ex_v(v)]),
            _ => Ex::App(P::SEq, vec![self.ex_text(v), Ex::Str(Rc::from(ABSENT))]),
        };
        let e = Ex::App(P::If, vec![c, self.ex_v(fb), self.ex_v(v)]);
        self.bind_a(st, Kind::Any, e)
    }
    /// The number a name holds for `;` and `:`: 0 when it holds none.
    fn num_slot(&mut self, st: &mut St, s: Nm) -> A {
        let zero = A::K(Val::N(0.0));
        match st.nums.get(&s).cloned() {
            None => zero,
            Some(v) if !st.maybe.contains(&(s, 1)) => v,
            Some(A::K(Val::L(t))) if t == ABSENT => zero,
            Some(v @ A::K(_)) => v,
            Some(v) => {
                let a = self.or_absent(st, &v, &zero);
                self.as_num(st, &a)
            }
        }
    }
    fn list_of(&self, st: &St, s: Nm) -> AL {
        st.lists.get(&s).cloned().unwrap_or_else(|| AL::K(Rc::new(Vec::new())))
    }
    fn set_list(&mut self, st: &mut St, s: Nm, l: AL) {
        Rc::make_mut(&mut st.lists).insert(s, l);
    }
    /// A list as a variable, written out from its known items when it has them.
    fn dyn_list(&mut self, st: &mut St, s: Nm) -> u32 {
        match self.list_of(st, s) {
            AL::D(v) => v,
            l => {
                let e = self.ex_list(&l);
                let v = self.bind(st, Ty::L, e);
                self.set_list(st, s, AL::D(v));
                v
            }
        }
    }
    fn clear_depth(&self, st: &mut St, d: u32) {
        fn clr<T: Clone>(m: &mut Rc<BTreeMap<Nm, T>>, d: u32) {
            if m.keys().any(|k| k.1 == d) {
                Rc::make_mut(m).retain(|k, _| k.1 != d);
            }
        }
        clr(&mut st.mem, d);
        clr(&mut st.nums, d);
        clr(&mut st.lists, d);
        clr(&mut st.scripts, d);
        if st.maybe.iter().any(|(k, _)| k.1 == d) {
            Rc::make_mut(&mut st.maybe).retain(|(k, _)| k.1 != d);
        }
    }

    // ---------- code ----------

    pub fn code_of(&mut self, src: &str) -> R<Rc<Code>> {
        if let Some(u) = self.units.get(src) {
            return Ok(u.clone());
        }
        let c = match parse(src, &mut self.it) {
            Ok(c) => c,
            Err(e) => return Err(Stop::Build(format!("parse: {}", e))),
        };
        let u = Rc::new(c);
        self.units.insert(src.to_string(), u.clone());
        Ok(u)
    }
    fn file_src(&self, path: &str) -> Option<String> {
        if let Ok(b) = std::fs::read(code_path(path)) {
            return Some(String::from_utf8_lossy(&b).into_owned());
        }
        self.lib.iter().find(|(p, _)| p == path).map(|(_, s)| s.clone())
    }
    fn visible(&self, st: &St, t: &str, bound: u64) -> Option<Mac> {
        st.repl.get(t).and_then(|v| v.iter().rev().find(|m| m.gen < bound).cloned())
    }

    // ---------- the frames ----------

    /// Runs code as a new frame to its end.
    fn frame_code(&mut self, mut st: St, code: Rc<Code>, bound: u64) -> R<St> {
        st.fs += 1;
        let d = st.depth();
        let mut st = self.chain(st, code, 0, bound, false)?;
        self.clear_depth(&mut st, d);
        st.fs -= 1;
        Ok(st)
    }
    fn frame_src(&mut self, st: St, src: &str) -> R<St> {
        let code = self.code_of(src)?;
        self.frame_code(st, code, UNBOUND)
    }
    fn script_of(&mut self, st: &St, k: Nm) -> R<(Rc<Code>, u64)> {
        if let Some(s) = st.scripts.get(&k) {
            return Ok(s.clone());
        }
        let name = self.it.name(k.0).to_string();
        match self.file_src(&name) {
            Some(src) => Ok((self.code_of(&src)?, UNBOUND)),
            None => self.no(&format!("{}.: no script or file named '{}'", name, name)),
        }
    }
    fn call(&mut self, st: St, k: Nm) -> R<St> {
        let (code, b) = self.script_of(&st, k)?;
        self.frame_code(st, code, b)
    }

    /// Runs from op `pc` of `code` to the end of the frame, through its tail calls.
    fn chain(&mut self, st: St, code: Rc<Code>, pc: usize, bound: u64, skip: bool) -> R<St> {
        let base = self.heads.len();
        let r = self.chain_in(st, code, pc, bound, skip, base);
        self.close_heads(base, r)
    }
    fn close_heads(&mut self, base: usize, r: R<St>) -> R<St> {
        match r {
            Err(Stop::Restart(h, g)) if self.heads[base..].iter().any(|x| x.id == h) => {
                let i = self.heads.iter().position(|x| x.id == h).unwrap();
                let saved = self.heads[i].saved.take().unwrap();
                let body = self.heads[i].body.clone();
                let b = self.heads[i].b;
                self.heads.truncate(base);
                self.residual(saved, body, *g, b)
            }
            r => {
                self.heads.truncate(base);
                r
            }
        }
    }
    fn chain_in(&mut self, mut st: St, mut code: Rc<Code>, mut pc: usize, mut bound: u64, mut skip: bool, base: usize) -> R<St> {
        loop {
            if pc == 0 && !skip {
                let key = Key::Code(Rc::as_ptr(&code) as usize);
                if let Some(exit) = self.head(&st, key, HB::Code(code.clone(), bound), base)? {
                    return Ok(exit);
                }
            }
            skip = false;
            let (s2, fl) = self.run_ops(st, &code, pc, bound)?;
            st = s2;
            match fl {
                Fl::End | Fl::Done | Fl::Exited => return Ok(st),
                Fl::Tail(c, b) => {
                    let d = st.depth();
                    self.clear_depth(&mut st, d);
                    code = c;
                    pc = 0;
                    bound = b;
                }
                Fl::Rep(k, n) => {
                    let d = st.depth();
                    self.clear_depth(&mut st, d);
                    st.reps.push((A::K(Val::N(0.0)), n));
                    let mut st = self.rep_loop(st, k, false, false)?;
                    st.reps.pop();
                    return Ok(st);
                }
            }
        }
    }

    /// An entry into code or into a repeat's next round. Returns the frame's end when the
    /// entry became a call to a function.
    fn head(&mut self, st: &St, key: Key, body: HB, base: usize) -> R<Option<St>> {
        if let Some(i) = self.heads.iter().rposition(|h| h.key == key && h.nf == st.frames.len()) {
            match self.heads[i].kind {
                HK::Fun(f) => {
                    let g = self.funs[f].g.clone();
                    let gb = self.funs[f].gb;
                    let cb = self.base_of(st, &key);
                    if self.fits(st, cb, &g, gb) {
                        let exit = self.call_fun(st.clone(), f)?;
                        return Ok(Some(exit));
                    }
                    let g2 = self.generalize(&g, gb, st, cb)?;
                    return Err(Stop::Restart(self.heads[i].id, Box::new(g2)));
                }
                HK::Inline => {
                    let (hdyn, hsteps, hid) = (self.heads[i].dyn0, self.heads[i].steps0, self.heads[i].id);
                    let deep = i < base && self.heads.iter().filter(|h| h.key == key).count() >= MAX_NEST;
                    if self.dyn_ev != hdyn || self.steps - hsteps > LOOP_STEPS || deep {
                        let saved = self.heads[i].saved.as_ref().unwrap().clone();
                        let sb = self.heads[i].b;
                        let cb = self.base_of(st, &key);
                        match self.generalize(&saved, sb, st, cb) {
                            Ok(g) => return Err(Stop::Restart(hid, Box::new(g))),
                            // a round that sets names the round before it did not: one more
                            // round runs here, and the loop is made general from the next
                            Err(Stop::Build(m)) => {
                                let peel = self.heads[i].peel;
                                if peel >= 3 || !m.contains("names set on one path") {
                                    return Err(Stop::Build(m));
                                }
                                let id = self.next_head;
                                self.next_head += 1;
                                let b = self.base_of(st, &key);
                                let h = Head { id, key, kind: HK::Inline, saved: Some(st.clone()), body, dyn0: self.dyn_ev, steps0: self.steps, b, peel: peel + 1, rounds: 0, nf: st.frames.len() };
                                if i >= base {
                                    self.heads[i] = h;
                                } else {
                                    self.heads.push(h);
                                }
                                return Ok(None);
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    if i >= base {
                        // the same loop again, with nothing written out: the head moves on,
                        // keeping the work it allows counted from the first round
                        // the state is kept at rounds 1, 2, 4, 8 …: a long loop copies it a
                        // few times, and a loop made general starts from the last copy
                        self.heads[i].rounds += 1;
                        let r = self.heads[i].rounds;
                        if r & (r - 1) == 0 {
                            self.heads[i].saved = Some(st.clone());
                            self.heads[i].b = self.base_of(st, &key);
                            self.heads[i].dyn0 = self.dyn_ev;
                        }
                        return Ok(None);
                    }
                }
            }
        }
        let id = self.next_head;
        self.next_head += 1;
        let b = self.base_of(st, &key);
        if self.heads.iter().filter(|h| h.key == key).count() >= MAX_NEST {
            return self.no("code that runs inside ever more `(` frames");
        }
        self.heads.push(Head { id, key, kind: HK::Inline, saved: Some(st.clone()), body, dyn0: self.dyn_ev, steps0: self.steps, b, peel: 0, rounds: 0, nf: st.frames.len() });
        Ok(None)
    }

    // ---------- repeats ----------

    /// `n name.` and `&&&`: the repeat's frame is on the stack, its counter on top of reps.
    fn rep_loop(&mut self, st: St, k: Nm, amp: bool, skip: bool) -> R<St> {
        let base = self.heads.len();
        let r = self.rep_in(st, k, amp, skip, base);
        self.close_heads(base, r)
    }
    fn rep_in(&mut self, mut st: St, k: Nm, amp: bool, mut skip: bool, base: usize) -> R<St> {
        loop {
            // every round starts on an empty line, as the machine's does
            st.args.clear();
            st.low = None;
            if !skip {
                if let Some(exit) = self.head(&st, Key::Rep(k), HB::Rep(k, amp), base)? {
                    return Ok(exit);
                }
            }
            skip = false;
            let (i, n) = st.reps.last().unwrap().clone();
            match (&i, &n) {
                (A::K(i), A::K(n)) => {
                    let (i, n) = (i.num(), n.num());
                    st.args.clear();
                    if i >= n {
                        return Ok(st);
                    }
                    st.reps.last_mut().unwrap().0 = A::K(Val::N(i + 1.0));
                    if amp {
                        self.nset(&mut st, (S_N, GL), A::K(Val::N(i)));
                    }
                    self.steps += 1;
                    st = self.call(st, k)?;
                }
                _ => {
                    let c = Ex::App(P::Lt, vec![self.ex_num(&i), self.ex_num(&n)]);
                    let cv = self.bind(&mut st, Ty::B, c);
                    let ie = self.ex_num(&i);
                    return self.branch_exits(
                        st,
                        cv,
                        move |pe: &mut Pe, mut s: St| {
                            let i1 = pe.bind_a(&mut s, Kind::Num, Ex::App(P::Add, vec![ie.clone(), Ex::Num(1.0)]));
                            s.reps.last_mut().unwrap().0 = i1;
                            s.args.clear();
                            if amp {
                                let iv = pe.bind_a(&mut s, Kind::Num, ie);
                                pe.nset(&mut s, (S_N, GL), iv);
                            }
                            let s = pe.call(s, k)?;
                            pe.rep_loop(s, k, amp, false)
                        },
                        |_pe: &mut Pe, mut s: St| {
                            s.args.clear();
                            Ok(s)
                        },
                    );
                }
            }
        }
    }

    // ---------- choices ----------

    /// Both paths of a choice on c run to the end of the frame; the frame ends joined.
    fn branch_exits(
        &mut self,
        st: St,
        c: u32,
        t: impl FnOnce(&mut Pe, St) -> R<St>,
        e: impl FnOnce(&mut Pe, St) -> R<St>,
    ) -> R<St> {
        self.dyn_ev += 1;
        let base = st.code.clone();
        let mut a = st.clone();
        a.code = Buf::default();
        let mut b = st;
        b.code = Buf::default();
        let ra = t(self, a);
        let ra = self.path_result(ra)?;
        let rb = e(self, b);
        let rb = self.path_result(rb)?;
        self.join(base, c, ra, rb)
    }
    /// A path's end: alive (Ok) or dead (its block ends with how).
    fn path_result(&mut self, r: R<St>) -> R<Result<St, St>> {
        match r {
            Ok(s) => Ok(Ok(s)),
            Err(Stop::Dead(s)) => Ok(Err(*s)),
            Err(e) => Err(e),
        }
    }
    /// The two paths of a choice joined into one, the choice written out as a branch.
    fn join(&mut self, base: Buf, c: u32, a: Result<St, St>, b: Result<St, St>) -> R<St> {
        let dead_block = |s: &St| Block { stmts: s.code.to_vec(), end: s.end.clone().unwrap() };
        let (t, e, z) = match (&a, &b) {
            (Ok(a), Ok(b)) => {
                let z = match self.zip(&[a, b], None, Mode::Merge) {
                    Ok(z) => z,
                    Err(m) => return self.no(&format!("the paths of a choice on input end differently: {}", m)),
                };
                let t = Block { stmts: a.code.to_vec(), end: End::Yield(z.vals[0].clone()) };
                let e = Block { stmts: b.code.to_vec(), end: End::Yield(z.vals[1].clone()) };
                (t, e, z)
            }
            (Ok(a), Err(b)) => {
                let z = self.zip(&[a], None, Mode::Merge).map_err(Stop::Build)?;
                let t = Block { stmts: a.code.to_vec(), end: End::Yield(z.vals[0].clone()) };
                (t, dead_block(b), z)
            }
            (Err(a), Ok(b)) => {
                let z = self.zip(&[b], None, Mode::Merge).map_err(Stop::Build)?;
                let e = Block { stmts: b.code.to_vec(), end: End::Yield(z.vals[0].clone()) };
                (dead_block(a), e, z)
            }
            (Err(a), Err(b)) => {
                let (t, e) = (dead_block(a), dead_block(b));
                let mut m = a.clone();
                m.code = base;
                m.emit(Stmt::Branch { c: Ex::Var(c), t, e, outs: vec![] });
                m.end = Some(End::Never);
                return Err(Stop::Dead(Box::new(m)));
            }
        };
        let mut m = z.st;
        m.code = base;
        m.emit(Stmt::Branch { c: Ex::Var(c), t, e, outs: z.outs });
        Ok(m)
    }
}

// ---------- the ops ----------

fn cat(parts: Vec<Ex>) -> Ex {
    let mut out: Vec<Ex> = Vec::new();
    for p in parts {
        match (out.last_mut(), &p) {
            (Some(Ex::Str(a)), Ex::Str(b)) => {
                let s = format!("{}{}", a, b);
                *a = Rc::from(s.as_str());
            }
            (_, Ex::Str(b)) if b.is_empty() => {}
            _ => out.push(p),
        }
    }
    if out.len() == 1 {
        out.pop().unwrap()
    } else if out.is_empty() {
        Ex::Str(Rc::from(""))
    } else {
        Ex::App(P::Cat, out)
    }
}

impl Pe {
    fn fail<T>(&mut self, mut st: St, msg: String) -> R<T> {
        let msg = match st.lane_at {
            Some(i) if !msg.starts_with("lane ") => format!("lane {}: {}", i, msg),
            _ => msg,
        };
        st.end = Some(End::Fail(Ex::Str(Rc::from(msg.as_str()))));
        Err(Stop::Dead(Box::new(st)))
    }
    /// Values from the line's lower part moved up until args holds n: what input made.
    fn line_take(&mut self, st: &mut St, n: usize) {
        while st.args.len() < n {
            let lv = match st.low {
                Some(v) => v,
                None => return,
            };
            let x = self.bind_a(st, Kind::Any, Ex::App(P::LLast, vec![Ex::Var(lv)]));
            let l = self.bind(st, Ty::L, Ex::App(P::LPop, vec![Ex::Var(lv)]));
            st.args.insert(0, x);
            st.low = Some(l);
        }
    }
    fn need(&mut self, st: &St, n: usize, c: Cmd) -> R<()> {
        if st.args.len() < n {
            let m = format!("{}: needs {} args, stack has {}", c.word(), n, st.args.len());
            return self.fail(st.clone(), m);
        }
        Ok(())
    }
    fn static_text(&mut self, a: &A, what: &str) -> R<String> {
        match a {
            A::K(v) => Ok(v.text().into_owned()),
            A::D(_) => self.no(&format!("{} built from input", what)),
        }
    }
    /// A repeat count: a whole number >= 0.
    fn count(&mut self, st: &St, i: usize, w: &str) -> R<A> {
        match &st.args[i] {
            A::K(v) => {
                let x = match v {
                    Val::N(x) => Some(*x),
                    _ => v.text().trim().parse::<f64>().ok(),
                };
                match x {
                    Some(x) if x.is_finite() && x >= 0.0 && x.fract() == 0.0 => Ok(A::K(Val::N(x))),
                    _ => {
                        let m = format!("{}: repeat count must be a whole number >= 0, got '{}'", w, v.text());
                        self.fail(st.clone(), m)
                    }
                }
            }
            a => Ok(a.clone()),
        }
    }

    /// Runs ops from pc to the end of the code, or until the frame is left.
    fn run_ops(&mut self, mut st: St, code: &Rc<Code>, mut pc: usize, bound: u64) -> R<(St, Fl)> {
        let n = code.ops.len();
        while pc < n {
            self.site = (code.src.clone(), code.spans[pc]);
            self.steps += 1;
            let op = &code.ops[pc];
            pc += 1;
            let tail = pc == n;
            if let Op::Cond(t, inner) = op {
                if let AB::D(c) = st.ow {
                    match self.cond_split(st, code, pc, bound, *t, inner, c, tail)? {
                        (s, None) => {
                            st = s;
                            continue;
                        }
                        (s, Some(fl)) => return Ok((s, fl)),
                    }
                }
            }
            let (s, fl) = self.step(st, op, tail, bound)?;
            st = s;
            match fl {
                Fl::End => {}
                fl => return Ok((st, fl)),
            }
        }
        Ok((st, Fl::End))
    }

    /// A conditional op on an unknown yes-flag: the op runs on the path where the flag says
    /// so. The paths join after it when they can, or else at the end of the frame.
    fn cond_split(&mut self, st: St, code: &Rc<Code>, pc: usize, bound: u64, t: bool, inner: &Op, c: u32, tail: bool) -> R<(St, Option<Fl>)> {
        self.dyn_ev += 1;
        let base = st.code.clone();
        let mut on = st.clone();
        on.ow = AB::K(t);
        on.code = Buf::default();
        let mut off = st;
        off.ow = AB::K(!t);
        off.code = Buf::default();
        let r_on = self.step(on, inner, tail, bound);
        let (on, fl) = match r_on {
            Ok((s, fl)) => (Ok(s), fl),
            Err(Stop::Dead(s)) => (Err(*s), Fl::End),
            Err(e) => return Err(e),
        };
        let order = |a: Result<St, St>, b: Result<St, St>| if t { (a, b) } else { (b, a) };
        if let (Ok(a), Fl::End) = (&on, &fl) {
            // the paths join right after only when their lines have one shape: else a value
            // one path put on the line is still to be used by that path
            if same_lines(a, &off) && self.zip(&[a, &off], None, Mode::Merge).is_ok() {
                let (x, y) = order(on, Ok(off));
                let m = self.join(base, c, x, y)?;
                return Ok((m, None));
            }
        }
        if let Err(_) = &on {
            let (x, y) = order(on, Ok(off));
            let m = self.join(base, c, x, y)?;
            return Ok((m, None));
        }
        // each path to the end of the frame
        let on = on.ok().unwrap();
        let ra = self.finish(on, fl, code, pc, bound);
        let ra = self.path_result(ra)?;
        let rb = self.chain(off, code.clone(), pc, bound, true);
        let rb = self.path_result(rb)?;
        let (x, y) = order(ra, rb);
        let m = self.join(base, c, x, y)?;
        Ok((m, Some(Fl::Exited)))
    }
    /// The rest of a frame after an op that left it as fl says.
    fn finish(&mut self, mut st: St, fl: Fl, code: &Rc<Code>, pc: usize, bound: u64) -> R<St> {
        match fl {
            Fl::End => self.chain(st, code.clone(), pc, bound, true),
            Fl::Done | Fl::Exited => Ok(st),
            Fl::Tail(c, b) => {
                let d = st.depth();
                self.clear_depth(&mut st, d);
                self.chain(st, c, 0, b, false)
            }
            Fl::Rep(k, n) => {
                let d = st.depth();
                self.clear_depth(&mut st, d);
                st.reps.push((A::K(Val::N(0.0)), n));
                let mut st = self.rep_loop(st, k, false, false)?;
                st.reps.pop();
                Ok(st)
            }
        }
    }

    fn step(&mut self, mut st: St, op: &Op, tail: bool, bound: u64) -> R<(St, Fl)> {
        match op {
            Op::Push(v) => st.args.push(A::K(v.clone())),
            Op::Interp(ps) => {
                let mut parts: Vec<A> = Vec::new();
                for p in ps {
                    match p {
                        Part::T(t) => parts.push(A::K(Val::of(t.clone()))),
                        Part::V(k) => {
                            let k = self.l(&st, *k)?;
                            parts.push(self.get_val(&mut st, k)?);
                        }
                    }
                }
                let v = self.interp(&mut st, ps, parts);
                st.args.push(v);
            }
            Op::Word(t, k, n) => {
                let st = self.word(st, t, *k, *n, bound)?;
                return Ok((st, Fl::End));
            }
            Op::Cmd(c) => {
                if let Some(m) = self.visible(&st, c.word(), bound) {
                    let st = self.expand(st, m)?;
                    return Ok((st, Fl::End));
                }
                return self.cmd(st, *c, bound);
            }
            Op::Native(c) => return self.cmd(st, *c, bound),
            Op::Pop(k) => {
                let k = self.l(&st, *k)?;
                if st.args.is_empty() && st.low.is_some() {
                    self.line_take(&mut st, 1);
                }
                match st.args.pop() {
                    Some(v) => self.mset(&mut st, k, v),
                    None => {
                        let m = format!("{}@: stack is empty", self.it.name(k.0));
                        return self.fail(st, m);
                    }
                }
            }
            Op::Lookup(k) => {
                let k = self.l(&st, *k)?;
                let v = self.get_val(&mut st, k)?;
                return self.dispatch(st, v, bound);
            }
            Op::Def(k) => {
                let k = self.l(&st, *k)?;
                let parts = std::mem::take(&mut st.args);
                let mut body = String::new();
                for (i, p) in parts.iter().enumerate() {
                    if i > 0 {
                        body.push(' ');
                    }
                    body.push_str(&self.static_text(p, "a script")?);
                }
                let body = body.replace('|', "~");
                let code = match self.code_of(&body) {
                    Ok(c) => c,
                    Err(Stop::Build(e)) => {
                        let m = format!("{}$: {}", self.it.name(k.0), e.trim_start_matches("parse: "));
                        return self.fail(st, m);
                    }
                    Err(e) => return Err(e),
                };
                self.note(&mut st, k);
                let g = st.gen;
                Rc::make_mut(&mut st.scripts).insert(k, (code, g));
            }
            Op::SetMem(k) => {
                let k = self.l(&st, *k)?;
                let v = if st.args.len() == 1 {
                    st.args.pop().unwrap()
                } else {
                    let xs = std::mem::take(&mut st.args);
                    self.joined_val(&mut st, &xs)
                };
                st.args.clear();
                self.mset(&mut st, k, v);
            }
            Op::SetNum(k) => {
                let k = self.l(&st, *k)?;
                let x = match st.args.first().cloned() {
                    None => A::K(Val::N(0.0)),
                    Some(a) => self.as_num(&mut st, &a),
                };
                st.args.clear();
                self.nset(&mut st, k, x);
            }
            Op::Inc(k) | Op::Dec(k) => {
                let up = matches!(op, Op::Inc(_));
                let k = self.l(&st, *k)?;
                let d = if st.args.is_empty() {
                    A::K(Val::N(1.0))
                } else {
                    let a = st.args[0].clone();
                    st.args.clear();
                    a
                };
                let x = self.num_slot(&mut st, k);
                let r = self.arith2(&mut st, if up { Cmd::Add } else { Cmd::Sub }, &x, &d);
                self.nset(&mut st, k, r);
            }
            Op::Call(k) | Op::CallPass(k) => {
                let pass = matches!(op, Op::CallPass(_));
                let k = self.l(&st, *k)?;
                if !pass && st.args.len() == 1 && st.low.is_none() {
                    let n = self.count(&st, 0, &format!("{}.", self.it.name(k.0)))?;
                    if tail {
                        return Ok((st, Fl::Rep(k, n)));
                    }
                    st.fs += 1;
                    let d = st.depth();
                    st.reps.push((A::K(Val::N(0.0)), n));
                    let _ = k;
                    let mut s = self.rep_loop_nm(st, k, false)?;
                    s.reps.pop();
                    self.clear_depth(&mut s, d);
                    s.fs -= 1;
                    return Ok((s, Fl::End));
                }
                if tail {
                    let (code, b) = self.script_of(&st, k)?;
                    return Ok((st, Fl::Tail(code, b)));
                }
                let st = self.call(st, k)?;
                return Ok((st, Fl::End));
            }
            Op::Cond(t, inner) => {
                if let AB::K(ow) = st.ow {
                    if ow == *t {
                        return self.step(st, inner, tail, bound);
                    }
                }
            }
            Op::Lanes(f) => {
                let st = self.lanes_op(st, f.clone())?;
                return Ok((st, Fl::End));
            }
        }
        Ok((st, Fl::End))
    }

    fn interp(&mut self, st: &mut St, _ps: &[Part], parts: Vec<A>) -> A {
        if parts.iter().all(|p| matches!(p, A::K(_))) {
            let s: String = parts.iter().map(|p| if let A::K(v) = p { v.text().into_owned() } else { String::new() }).collect();
            return A::K(Val::of(s));
        }
        let nonempty: Vec<&A> = parts.iter().filter(|p| !matches!(p, A::K(v) if v.text().is_empty())).collect();
        if nonempty.len() == 1 {
            if let A::D(d) = nonempty[0] {
                return A::D(*d);
            }
        }
        let mut text_only = false;
        let mut es = Vec::new();
        for p in &parts {
            if let A::K(v) = p {
                if v.text().chars().any(|c| !numberish(c)) {
                    text_only = true;
                }
            }
            es.push(self.ex_text(p));
        }
        let e = cat(es);
        let a = if text_only { self.bind_a(st, Kind::Str, e) } else { self.bind_a(st, Kind::Any, Ex::App(P::VOf, vec![e])) };
        if let A::D(d) = &a {
            self.texts.insert(d.v, parts);
        }
        a
    }
    /// Code built by `~name~` from input, run: the text read with each value from input held
    /// apart in a hidden name, the quoted words that hold one read from it. A value holding
    /// a quote would end the quote early in the machine: it stops the program here.
    fn run_template(&mut self, mut st: St, parts: Vec<A>) -> R<St> {
        let mut tpl = String::new();
        let mut holes: Vec<Sym> = Vec::new();
        for p in &parts {
            match p {
                A::K(v) => tpl.push_str(&v.text()),
                A::D(d) => {
                    let i = holes.len();
                    let k = self.it.sym(&format!("\u{2}r{}", i));
                    holes.push(k);
                    let _ = write!(tpl, "\u{2}{}\u{2}", i);
                    self.mset(&mut st, (k, GL), p.clone());
                    // a number holds no quote, space or suffix: it reads the same wherever it is
                    if d.k == Kind::Num {
                        continue;
                    }
                    let n = Ex::App(P::LLen, vec![Ex::App(P::SplitBy, vec![self.ex_text(p), Ex::Str(Rc::from("'"))])]);
                    let c = self.bind(&mut st, Ty::B, Ex::App(P::Gt, vec![n, Ex::Num(1.0)]));
                    let msg = "a reply from a worker holds a quote, which the machine would read as the end of one";
                    st.emit(Stmt::Branch { c: Ex::Var(c), t: Block { stmts: vec![], end: End::Fail(Ex::Str(Rc::from(msg))) }, e: Block::default(), outs: vec![] });
                    self.mset(&mut st, (k, GL), p.clone());
                }
            }
        }
        let code = self.code_of(&tpl)?;
        let mut ops = Vec::new();
        for op in code.ops.iter() {
            // a plain word with a value from input in it: pushed as it reads, when no
            // replacement could have that name
            let word = match op {
                Op::Word(t, _, _) if t.contains('\u{2}') => {
                    let pre = &t[..t.find('\u{2}').unwrap()];
                    let suf = &t[t.rfind('\u{2}').unwrap() + 1..];
                    if st.repl.keys().any(|k| k.len() > pre.len() + suf.len() && k.starts_with(pre) && k.ends_with(suf)) {
                        return self.no("a word from a worker's code that a replacement could have the name of");
                    }
                    Some(t.to_string())
                }
                _ => None,
            };
            let text = match (op, word) {
                (_, Some(t)) => Some(t),
                (Op::Push(v), None) if v.text().contains('\u{2}') => Some(v.text().into_owned()),
                _ => None,
            };
            match op {
                _ if text.is_some() => {
                    let t = text.unwrap();
                    let mut ps = Vec::new();
                    for (j, piece) in t.split('\u{2}').enumerate() {
                        if j % 2 == 0 {
                            if !piece.is_empty() {
                                ps.push(Part::T(piece.to_string()));
                            }
                        } else {
                            let i: usize = piece.parse().unwrap_or(0);
                            ps.push(Part::V(holes[i]));
                        }
                    }
                    ops.push(Op::Interp(ps));
                }
                op if format!("{:?}", op).contains("\\u{2}") => return self.no("a worker's reply whose code, not only its quoted words, comes from input"),
                op => ops.push(op.clone()),
            }
        }
        let c2 = Rc::new(Code { ops, spans: code.spans.clone(), unit: code.unit, src: code.src.clone() });
        self.frame_code(st, c2, UNBOUND)
    }
    fn as_num(&mut self, st: &mut St, a: &A) -> A {
        match a {
            A::K(v) => A::K(Val::N(v.num())),
            A::D(D { k: Kind::Num, .. }) => a.clone(),
            _ => {
                let e = self.ex_num(a);
                self.bind_a(st, Kind::Num, e)
            }
        }
    }

    fn word(&mut self, mut st: St, t: &Rc<str>, k: Sym, n: Option<f64>, bound: u64) -> R<St> {
        if let Some(x) = n {
            if st.repl_num {
                if let Some(m) = self.visible(&st, t, bound) {
                    return self.expand(st, m);
                }
            }
            st.args.push(A::K(Val::N(x)));
            return Ok(st);
        }
        if let Some(m) = self.visible(&st, t, bound) {
            return self.expand(st, m);
        }
        st.args.push(A::K(Val::K(t.clone(), k)));
        Ok(st)
    }
    /// Runs a visible definition: a value is pushed, code runs as a frame.
    fn expand(&mut self, mut st: St, m: Mac) -> R<St> {
        if let Some(nat) = m.nat {
            return self.native(st, nat, m.gen);
        }
        match m.code {
            None => {
                st.args.push(A::K(m.val));
                Ok(st)
            }
            Some(c) => self.frame_code(st, c, m.gen),
        }
    }

    /// A looked-up value run as a word: a value that is one word is executed; anything else
    /// is pushed.
    fn dispatch(&mut self, mut st: St, v: A, bound: u64) -> R<(St, Fl)> {
        let v = match v {
            A::K(v) => v,
            A::D(d) => {
                match d.k {
                    Kind::Num => {
                        if st.repl_num && st.repl.keys().any(|k| canon_num(k).is_some()) {
                            return self.no("a number from input looked up where numbers are replaced");
                        }
                    }
                    _ if d.m > 0 => {
                        let dom = (*self.doms[d.m as usize]).clone();
                        let running: Vec<Val> = dom.iter().filter(|v| self.runs_static(&st, v, bound)).cloned().collect();
                        if running.is_empty() {
                            st.args.push(A::D(d));
                            return Ok((st, Fl::End));
                        }
                        let rest = dom.len() > running.len();
                        let st = self.dispatch_each(st, d, &running, 0, rest, bound)?;
                        return Ok((st, Fl::End));
                    }
                    _ => {
                        // text from input runs as code when it is one word the machine runs
                        let keys: Vec<String> = st.repl.iter().filter(|(_, ms)| ms.iter().any(|m| m.gen < bound)).map(|(k, _)| k.to_string()).collect();
                        let ki = match self.keysets.iter().position(|s| *s == keys) {
                            Some(i) => i,
                            None => {
                                self.keysets.push(keys);
                                self.keysets.len() - 1
                            }
                        };
                        let te = self.ex_text(&A::D(d));
                        let c = self.bind(&mut st, Ty::B, Ex::App(P::Runs, vec![te.clone(), Ex::Num(ki as f64)]));
                        let st2 = st.clone();
                        let msg = Ex::App(P::Cat, vec![Ex::Str(Rc::from("'")), te, Ex::Str(Rc::from("' runs as code here; a build without the machine cannot run a word it learns as the program runs, s3 build can"))]);
                        let base = st.code.clone();
                        let mut dead = st2;
                        dead.code = Buf::default();
                        dead.end = Some(End::Fail(msg));
                        let mut live = st;
                        live.code = Buf::default();
                        let mut m = self.join(base, c, Err(dead), Ok(live))?;
                        m.args.push(A::D(d));
                        return Ok((m, Fl::End));
                    }
                }
                st.args.push(A::D(d));
                return Ok((st, Fl::End));
            }
        };
        let t: Option<Rc<str>> = match &v {
            Val::N(x) => {
                if st.repl_num {
                    if let Some(m) = self.visible(&st, &fmt_num(*x), bound) {
                        return Ok((self.expand(st, m)?, Fl::End));
                    }
                }
                None
            }
            Val::W(t, _) => {
                if let Some(m) = self.visible(&st, t, bound) {
                    return Ok((self.expand(st, m)?, Fl::End));
                }
                None
            }
            Val::K(t, _) => {
                if let Some(m) = self.visible(&st, t, bound) {
                    return Ok((self.expand(st, m)?, Fl::End));
                }
                None
            }
            Val::L(t) => Some(Rc::from(*t)),
            Val::R(t) => Some(t.clone()),
        };
        if let Some(t) = t {
            if !t.is_empty() && !t.chars().any(is_ws) {
                if let Some(c) = cmd_of(&t) {
                    if let Some(m) = self.visible(&st, c.word(), bound) {
                        return Ok((self.expand(st, m)?, Fl::End));
                    }
                    return self.cmd(st, c, bound);
                }
                if active_form(&t) || t.starts_with('⟨') {
                    let op = if t.starts_with('⟨') {
                        match parse(&t, &mut self.it) {
                            Ok(mut c) if c.ops.len() == 1 => c.ops.pop().unwrap(),
                            _ => {
                                st.args.push(A::K(v));
                                return Ok((st, Fl::End));
                            }
                        }
                    } else {
                        classify(&t, &mut self.it)
                    };
                    let (st, fl) = self.step(st, &op, false, bound)?;
                    return Ok((st, if matches!(fl, Fl::Done) { Fl::Done } else { Fl::End }));
                }
                if let Some(m) = self.visible(&st, &t, bound) {
                    return Ok((self.expand(st, m)?, Fl::End));
                }
            }
        }
        st.args.push(A::K(v));
        Ok((st, Fl::End))
    }

    /// Whether the machine would run a value it looked up, rather than push it.
    fn runs_static(&self, st: &St, v: &Val, bound: u64) -> bool {
        match v {
            Val::N(x) => st.repl_num && self.visible(st, &fmt_num(*x), bound).is_some(),
            Val::W(t, _) => self.visible(st, t, bound).is_some(),
            Val::K(t, _) => self.visible(st, t, bound).is_some(),
            Val::L(_) | Val::R(_) => {
                let t = v.text();
                !t.is_empty() && !t.chars().any(is_ws) && (cmd_of(&t).is_some() || active_form(&t) || t.starts_with('⟨') || self.visible(st, &t, bound).is_some())
            }
        }
    }
    /// A looked-up value from a known set: each value that runs is a path of its own.
    fn dispatch_each(&mut self, mut st: St, d: D, running: &[Val], i: usize, rest: bool, bound: u64) -> R<St> {
        if i == running.len() {
            st.args.push(A::D(d));
            return Ok(st);
        }
        if i + 1 == running.len() && !rest {
            let (s, fl) = self.dispatch(st, A::K(running[i].clone()), bound)?;
            if matches!(fl, Fl::Done) {
                return self.no("a value from a known set that returns from its frame");
            }
            return Ok(s);
        }
        let e = self.eq_ex(&A::D(d), &A::K(running[i].clone()));
        let c = self.bind(&mut st, Ty::B, e);
        self.dyn_ev += 1;
        let base = st.code.clone();
        let mut a = st.clone();
        a.code = Buf::default();
        let mut b = st;
        b.code = Buf::default();
        let ra = match self.dispatch(a, A::K(running[i].clone()), bound) {
            Ok((s, Fl::Done)) => {
                let _ = s;
                return self.no("a value from a known set that returns from its frame");
            }
            Ok((s, _)) => Ok(s),
            Err(e) => Err(e),
        };
        let ra = self.path_result(ra)?;
        let rb = self.dispatch_each(b, d, running, i + 1, rest, bound);
        let rb = self.path_result(rb)?;
        self.join(base, c, ra, rb)
    }

    /// Two values' equality when the build knows it.
    fn known_eq(&self, a: &A, b: &A) -> Option<bool> {
        let (x, y) = (self.dom_of(a)?, self.dom_of(b)?);
        if x.len() == 1 && y.len() == 1 {
            return Some(val_eq(&x[0], &y[0]));
        }
        if x.iter().all(|p| y.iter().all(|q| !val_eq(p, q))) {
            return Some(false);
        }
        None
    }

    fn arith2(&mut self, st: &mut St, c: Cmd, a: &A, b: &A) -> A {
        if let (A::K(x), A::K(y)) = (a, b) {
            let (a, b) = (x.num(), y.num());
            return A::K(Val::N(match c {
                Cmd::Add => a + b,
                Cmd::Sub => a - b,
                Cmd::Mul => a * b,
                Cmd::Div => a / b,
                Cmd::Pow => a.powf(b),
                Cmd::Mod => a.rem_euclid(b),
                _ => unreachable!(),
            }));
        }
        let p = match c {
            Cmd::Add => P::Add,
            Cmd::Sub => P::Sub,
            Cmd::Mul => P::Mul,
            Cmd::Div => P::Div,
            Cmd::Pow => P::Pow,
            Cmd::Mod => P::Mod,
            _ => unreachable!(),
        };
        let e = Ex::App(p, vec![self.ex_num(a), self.ex_num(b)]);
        self.bind_a(st, Kind::Num, e)
    }
    fn eq_ex(&self, a: &A, b: &A) -> Ex {
        match (kind_of(a), kind_of(b)) {
            (Kind::Num, Kind::Num) => Ex::App(P::NEq, vec![self.ex_num(a), self.ex_num(b)]),
            (Kind::Str, Kind::Str) => Ex::App(P::SEq, vec![self.ex_text(a), self.ex_text(b)]),
            _ => Ex::App(P::VEq, vec![self.ex_v(a), self.ex_v(b)]),
        }
    }
    fn cmp(&mut self, st: &mut St, c: Cmd) {
        let (a, b) = (st.args[0].clone(), st.args[1].clone());
        if let (A::K(x), A::K(y)) = (&a, &b) {
            let r = match c {
                Cmd::Eq => val_eq(x, y),
                Cmd::Ne => !val_eq(x, y),
                Cmd::AndNe | Cmd::AndEq => {
                    let e = if c == Cmd::AndEq { val_eq(x, y) } else { !val_eq(x, y) };
                    match st.ow {
                        AB::K(o) => o && e,
                        AB::D(o) => {
                            let v = self.bind(st, Ty::B, Ex::App(P::And, vec![Ex::Var(o), Ex::Bool(e)]));
                            st.ow = AB::D(v);
                            return;
                        }
                    }
                }
                Cmd::NumEq => x.num() == y.num(),
                Cmd::Lt => x.num() < y.num(),
                Cmd::Gt => x.num() > y.num(),
                Cmd::Le => x.num() <= y.num(),
                Cmd::Ge => x.num() >= y.num(),
                _ => unreachable!(),
            };
            st.ow = AB::K(r);
            return;
        }
        if matches!(c, Cmd::Eq | Cmd::Ne) {
            if let Some(e) = self.known_eq(&a, &b) {
                st.ow = AB::K(if c == Cmd::Eq { e } else { !e });
                return;
            }
        }
        let num = |p: P, s: &Pe| Ex::App(p, vec![s.ex_num(&a), s.ex_num(&b)]);
        let e = match c {
            Cmd::Eq => self.eq_ex(&a, &b),
            Cmd::Ne => Ex::App(P::Not, vec![self.eq_ex(&a, &b)]),
            Cmd::AndEq | Cmd::AndNe => {
                let mut e = self.eq_ex(&a, &b);
                if c == Cmd::AndNe {
                    e = Ex::App(P::Not, vec![e]);
                }
                Ex::App(P::And, vec![self.ex_b(st.ow), e])
            }
            Cmd::NumEq => num(P::NEq, self),
            Cmd::Lt => num(P::Lt, self),
            Cmd::Gt => num(P::Gt, self),
            Cmd::Le => num(P::Le, self),
            Cmd::Ge => num(P::Ge, self),
            _ => unreachable!(),
        };
        let v = self.bind(st, Ty::B, e);
        st.ow = AB::D(v);
    }

    fn write(&mut self, st: &mut St, parts: Vec<Ex>) {
        let e = cat(parts);
        if !matches!(e, Ex::Str(_)) {
            self.dyn_ev += 1;
        }
        st.emit(Stmt::Write(e));
    }

    /// A command: runs, and the line is empty after it unless it says otherwise.
    fn cmd(&mut self, mut st: St, c: Cmd, bound: u64) -> R<(St, Fl)> {
        use Cmd::*;
        if c.outside() && !matches!(c, ReadIn | Read | ReadAt | Write | Append | Move | Shell | PrngN | PrngS | Open | FWriteAt | FReadAt | FClose | FWriteLn | FReadLn | Eof | Spawn | Kill | Send | Recv | Par | Tcp) {
            return self.no(&format!("`{}` is not built to Bend yet", c.word()));
        }
        if let Some(lv) = st.low {
            match c {
                Depth | Open1 | Close1 | Ret | Exit => {}
                Comment => {
                    st.args.clear();
                    st.low = None;
                    return Ok((st, Fl::End));
                }
                Print | PrintRaw => {
                    let full = self.ex_list(&line_al(Some(lv), &st.args));
                    let text = if c == Print {
                        let head = Ex::App(P::LPush, vec![Ex::App(P::LNew, vec![]), Ex::App(P::VT, vec![Ex::Str(Rc::from(""))])]);
                        Ex::App(P::Join, vec![Ex::App(P::LCat, vec![head, full]), Ex::Str(Rc::from(" "))])
                    } else {
                        Ex::App(P::Join, vec![full, Ex::Str(Rc::from(""))])
                    };
                    let parts = if c == Print { vec![Ex::Str(Rc::from("\n")), text] } else { vec![text] };
                    self.write(&mut st, parts);
                    st.args.clear();
                    st.low = None;
                    return Ok((st, Fl::End));
                }
                _ => return self.no(&format!("`{}` over a line whose length comes from input", c.word())),
            }
        }
        match c {
            // workers: each message runs in the worker's machine when it is sent, a schedule
            // the machine can take; its writes land there, and its reply waits for `>t`
            Spawn => {
                let paths: Vec<String> = st.args.iter().map(|a| self.static_text(a, "a worker's file")).collect::<R<Vec<_>>>()?;
                for p in paths {
                    st = self.watch_file(st, "t", &p)?;
                    let src = match self.file_src(&p) {
                        Some(s) => s,
                        None => return self.no(&format!("t: {} is not there at build time", p)),
                    };
                    let w = self.fresh_st();
                    let wk = match self.in_worker(&mut st, w, &src)? {
                        Ok((w, _)) => Wk { st: w, replies: Default::default(), dead: false, pending: None },
                        Err(m) => {
                            let mut q = std::collections::VecDeque::new();
                            q.push_back(Reply::Err(m));
                            Wk { st: self.fresh_st(), replies: q, dead: true, pending: None }
                        }
                    };
                    Rc::make_mut(&mut st.workers).push(Some(Rc::new(wk)));
                    let k = (st.workers.len() - 1) as f64;
                    self.nset(&mut st, (S_THREADS, GL), A::K(Val::N(k)));
                }
            }
            Tcp => {
                self.need(&st, 3, c)?;
                let addr = st.args[0].clone();
                let k = self.arg_sym(&st, 1)?;
                let mode = self.static_text(&st.args[2].clone(), "a tcp mode")?;
                let onerr = match st.args.get(3).cloned() {
                    Some(a) => Some(self.static_text(&a, "a tcp error script")?),
                    None => None,
                };
                self.note(&mut st, k);
                let h = Ex::Str(Rc::from(self.handle(k).as_str()));
                let (op, a): (&'static str, Ex) = match mode.as_str() {
                    "c" => ("tc", self.ex_text(&addr)),
                    "s" => ("tl", self.ex_text(&addr)),
                    "a" => {
                        let lk = self.arg_sym(&st, 0)?;
                        ("ta", Ex::Str(Rc::from(self.handle(lk).as_str())))
                    }
                    _ => {
                        let m = format!("tcp: bad mode '{}', use c, s or a", mode);
                        return self.fail(st, m);
                    }
                };
                let pre = Ex::App(P::Cat, vec![Ex::Str(Rc::from("tcp: ")), self.ex_text(&addr), Ex::Str(Rc::from(": "))]);
                let scripted = onerr.is_some();
                let (mut s2, v) = self.fx_or(st, op, a, h, pre, onerr)?;
                let val = if scripted { Ex::App(P::FxVal, vec![Ex::Var(v)]) } else { Ex::Var(v) };
                match mode.as_str() {
                    "s" => {
                        let ps = self.it.sym("port");
                        let p = self.bind_a(&mut s2, Kind::Num, Ex::App(P::Parse, vec![val]));
                        self.nset(&mut s2, (ps, GL), p);
                    }
                    "a" => {
                        let ps = self.it.sym("peer");
                        let p = self.bind_a(&mut s2, Kind::Any, Ex::App(P::VOf, vec![val]));
                        self.mset(&mut s2, (ps, GL), p);
                    }
                    _ => {}
                }
                s2.args.clear();
                return Ok((s2, Fl::End));
            }
            Kill => {
                self.need(&st, 1, c)?;
                match self.worker_ix(&st, 0)? {
                    Some(k) => Rc::make_mut(&mut st.workers)[k] = None,
                    None => self.mset(&mut st, (S_ERR, GL), A::K(Val::L("Invalid thread index"))),
                }
            }
            Send => {
                self.need(&st, 2, c)?;
                let marg = st.args[0].clone();
                match self.worker_ix(&st, 1)? {
                    Some(k) => {
                        let mut wk = (**st.workers[k].as_ref().unwrap()).clone();
                        if wk.dead {
                            self.mset(&mut st, (S_ERR, GL), A::K(Val::L("thread closed")));
                        } else {
                            if wk.pending.is_some() {
                                return self.no("a second message to a worker before `>t` takes the reply to the first");
                            }
                            // the message runs in the worker's machine; what it does runs on the
                            // worker's own thread, from here until `>t`
                            let mut w = wk.st.clone();
                            w.code = Buf::default();
                            let fs = w.fs;
                            let ran = match &marg {
                                A::K(v) => {
                                    let t = v.text().into_owned();
                                    self.frame_src(w, &t)
                                }
                                A::D(d) => match self.texts.get(&d.v).cloned() {
                                    Some(parts) => self.run_template(w, parts),
                                    None => return self.no("a message to a worker built from input"),
                                },
                            };
                            let (stmts, end) = match ran {
                                Ok(mut w) => {
                                    let stmts = w.code.to_vec();
                                    w.code = Buf::default();
                                    w.fs = fs;
                                    if let Some(o) = Rc::make_mut(&mut w.mem).remove(&(S_OUT, GL)) {
                                        wk.replies.push_back(Reply::Code(o));
                                    }
                                    wk.st = w;
                                    (stmts, None)
                                }
                                Err(Stop::Dead(d)) => {
                                    let mut d = *d;
                                    wk.dead = true;
                                    if let Some(End::Fail(Ex::Str(e))) = &d.end {
                                        wk.replies.push_back(Reply::Err(e.to_string()));
                                    }
                                    (std::mem::take(&mut d.code).to_vec(), d.end.take())
                                }
                                Err(e) => return Err(e),
                            };
                            if !stmts.is_empty() || matches!(end, Some(End::Exit(_)) | Some(End::Fail(_))) {
                                let id = self.var(Ty::F);
                                let outs: Vec<(u32, Ty)> = if end.is_some() { Vec::new() } else { top_defs(&stmts).into_iter().map(|v| (v, self.tys[v as usize])).collect() };
                                let end = match end {
                                    Some(End::Fail(Ex::Str(_))) => End::Yield(Vec::new()),
                                    Some(e) => e,
                                    None => End::Yield(outs.iter().map(|o| Ex::Var(o.0)).collect()),
                                };
                                st.emit(Stmt::Spawn { id, body: Block { stmts, end } });
                                wk.pending = Some((id, outs));
                            }
                            Rc::make_mut(&mut st.workers)[k] = Some(Rc::new(wk));
                        }
                    }
                    None => self.mset(&mut st, (S_ERR, GL), A::K(Val::L("Invalid thread index"))),
                }
            }
            Recv => {
                self.need(&st, 1, c)?;
                match self.worker_ix(&st, 0)? {
                    Some(k) => {
                        let mut wk = (**st.workers[k].as_ref().unwrap()).clone();
                        if let Some((id, outs)) = wk.pending.take() {
                            st.emit(Stmt::Join { id, outs });
                        }
                        let r = wk.replies.pop_front();
                        Rc::make_mut(&mut st.workers)[k] = Some(Rc::new(wk.clone()));
                        st.args.clear();
                        match r {
                            Some(Reply::Code(A::K(v))) => {
                                let t = v.text().into_owned();
                                let st = self.frame_src(st, &t)?;
                                return Ok((st, Fl::End));
                            }
                            Some(Reply::Code(A::D(d))) => {
                                let parts = match self.texts.get(&d.v) {
                                    Some(p) => p.clone(),
                                    None => return self.no("a worker's reply built from input"),
                                };
                                let st = self.run_template(st, parts)?;
                                return Ok((st, Fl::End));
                            }
                            Some(Reply::Err(e)) => return self.fail(st, e),
                            None if wk.dead => self.mset(&mut st, (S_ERR, GL), A::K(Val::L("thread closed"))),
                            None => return self.no("`>t` on a worker that has no reply to give"),
                        }
                    }
                    None => {
                        st.args.clear();
                        self.mset(&mut st, (S_ERR, GL), A::K(Val::L("Invalid thread index")))
                    }
                }
                return Ok((st, Fl::End));
            }
            Par => {
                let paths: Vec<String> = st.args.iter().map(|a| self.static_text(a, "a file of `***`")).collect::<R<Vec<_>>>()?;
                st.args.clear();
                let mut codes = Vec::new();
                for p in paths {
                    st = self.watch_file(st, "***", &p)?;
                    let src = match self.file_src(&p) {
                        Some(s) => s,
                        None => return self.no(&format!("***: {} is not there at build time", p)),
                    };
                    let w = self.fresh_st();
                    match self.in_worker(&mut st, w, &src)? {
                        Ok((_, out)) => codes.push(out),
                        Err(e) => return self.fail(st, e),
                    }
                }
                for o in codes {
                    let t = match o {
                        None => String::new(),
                        Some(A::K(v)) => v.text().into_owned(),
                        Some(A::D(_)) => return self.no("a reply of `***` built from input"),
                    };
                    st = self.frame_src(st, &t)?;
                }
                return Ok((st, Fl::End));
            }
            Print | PrintRaw => {
                let mut parts = Vec::new();
                if c == Print {
                    parts.push(Ex::Str(Rc::from("\n")));
                }
                for a in st.args.clone() {
                    if c == Print {
                        parts.push(Ex::Str(Rc::from(" ")));
                    }
                    parts.push(self.ex_text(&a));
                }
                self.write(&mut st, parts);
            }
            ReadIn => {
                let d = self.dest(&st, 0, S_UNDER)?;
                let lv = self.var(Ty::V);
                let ev = self.var(Ty::F);
                st.emit(Stmt::ReadLine(lv, ev));
                self.dyn_ev += 1;
                self.mset(&mut st, d, A::D(D { v: lv, k: Kind::Any, m: 0 }));
                self.mset(&mut st, (S_EOF, GL), A::D(D { v: ev, k: Kind::Num, m: 0 }));
            }
            Read => {
                self.need(&st, 1, c)?;
                let p = st.args[0].clone();
                let k = if st.args.len() > 1 { self.arg_sym(&st, 1)? } else { self.arg_sym(&st, 0)? };
                let pe_ = self.ex_text(&p);
                let pre = Ex::App(P::Cat, vec![Ex::Str(Rc::from("r: ")), pe_.clone(), Ex::Str(Rc::from(": "))]);
                let (s2, v) = self.fx(st, "r", pe_, Ex::Str(Rc::from("")), pre)?;
                st = s2;
                let a = self.bind_a(&mut st, Kind::Any, Ex::App(P::VOf, vec![Ex::Var(v)]));
                self.mset(&mut st, k, a);
            }
            ReadAt => {
                self.need(&st, 4, c)?;
                let (p, off, len) = (st.args[0].clone(), st.args[1].clone(), st.args[2].clone());
                let d = self.arg_sym(&st, 3)?;
                let pe_ = self.ex_text(&p);
                let (off, len) = (self.as_num(&mut st, &off), self.as_num(&mut st, &len));
                let span = Ex::App(P::Cat, vec![self.ex_text(&off), Ex::Str(Rc::from(" ")), self.ex_text(&len)]);
                let pre = Ex::App(P::Cat, vec![Ex::Str(Rc::from("r@: ")), pe_.clone(), Ex::Str(Rc::from(": "))]);
                let (s2, v) = self.fx(st, "ra", pe_, span, pre)?;
                st = s2;
                let a = self.bind_a(&mut st, Kind::Any, Ex::App(P::VOf, vec![Ex::Var(v)]));
                self.mset(&mut st, d, a);
            }
            Open => {
                self.need(&st, 3, c)?;
                let (p, mode) = (st.args[0].clone(), st.args[2].clone());
                let k = self.arg_sym(&st, 1)?;
                let h = self.handle(k);
                let onerr = match st.args.get(3).cloned() {
                    Some(a) => Some(self.static_text(&a, "an f error script")?),
                    None => None,
                };
                let pe_ = self.ex_text(&p);
                let b = Ex::App(P::Cat, vec![Ex::Str(Rc::from(h.as_str())), Ex::Str(Rc::from("\u{1}")), self.ex_text(&mode)]);
                let pre = Ex::App(P::Cat, vec![Ex::Str(Rc::from("f: ")), pe_.clone(), Ex::Str(Rc::from(": "))]);
                let (s2, _) = self.fx_or(st, "fo", pe_, b, pre, onerr)?;
                return Ok((s2, Fl::End));
            }
            FWriteAt => {
                self.need(&st, 3, c)?;
                let k = self.arg_sym(&st, 0)?;
                let off = st.args[1].clone();
                let key = self.arg_sym(&st, 2)?;
                let data = match st.mem.get(&key).cloned() {
                    Some(v) => self.ex_text(&v),
                    None => Ex::Str(Rc::from("")),
                };
                let onerr = match st.args.get(3).cloned() {
                    Some(a) => Some(self.static_text(&a, "a +f error script")?),
                    None => None,
                };
                let h = self.handle(k);
                let off = self.as_num(&mut st, &off);
                let b = Ex::App(P::Cat, vec![self.ex_text(&off), Ex::Str(Rc::from("\u{1}")), data]);
                let pre = Ex::Str(Rc::from(format!("+f: {}: ", self.it.name(k.0)).as_str()));
                let (s2, _) = self.fx_or(st, "fat", Ex::Str(Rc::from(h.as_str())), b, pre, onerr)?;
                return Ok((s2, Fl::End));
            }
            FWriteLn => {
                self.need(&st, 2, c)?;
                let k = self.arg_sym(&st, 0)?;
                let line = st.args[1].clone();
                let onerr = match st.args.get(2).cloned() {
                    Some(a) => Some(self.static_text(&a, "an f_ error script")?),
                    None => None,
                };
                let h = self.handle(k);
                let b = Ex::App(P::Cat, vec![self.ex_text(&line), Ex::Str(Rc::from("\n"))]);
                let pre = Ex::Str(Rc::from(format!("f_: {}: ", self.it.name(k.0)).as_str()));
                let (s2, _) = self.fx_or(st, "fw", Ex::Str(Rc::from(h.as_str())), b, pre, onerr)?;
                return Ok((s2, Fl::End));
            }
            FReadAt => {
                self.need(&st, 4, c)?;
                let k = self.arg_sym(&st, 0)?;
                let (off, len) = (st.args[1].clone(), st.args[2].clone());
                let d = self.arg_sym(&st, 3)?;
                let h = self.handle(k);
                let (off, len) = (self.as_num(&mut st, &off), self.as_num(&mut st, &len));
                let span = Ex::App(P::Cat, vec![self.ex_text(&off), Ex::Str(Rc::from(" ")), self.ex_text(&len)]);
                let pre = Ex::Str(Rc::from(format!("@f: {}: ", self.it.name(k.0)).as_str()));
                let (s2, v) = self.fx(st, "fra", Ex::Str(Rc::from(h.as_str())), span, pre)?;
                st = s2;
                let a = self.bind_a(&mut st, Kind::Any, Ex::App(P::VOf, vec![Ex::Var(v)]));
                self.mset(&mut st, d, a);
            }
            FReadLn => {
                self.need(&st, 2, c)?;
                let k = self.arg_sym(&st, 0)?;
                let d = self.arg_sym(&st, 1)?;
                let h = self.handle(k);
                let pre = Ex::Str(Rc::from(format!("_f: {}: ", self.it.name(k.0)).as_str()));
                let (s2, v) = self.fx(st, "frl", Ex::Str(Rc::from(h.as_str())), Ex::Str(Rc::from("")), pre)?;
                st = s2;
                let a = self.bind_a(&mut st, Kind::Any, Ex::App(P::VOf, vec![Ex::Var(v)]));
                self.mset(&mut st, d, a);
            }
            FClose => {
                self.need(&st, 1, c)?;
                let k = self.arg_sym(&st, 0)?;
                let h = self.handle(k);
                let (s2, _) = self.fx(st, "fc", Ex::Str(Rc::from(h.as_str())), Ex::Str(Rc::from("")), Ex::Str(Rc::from("f-: ")))?;
                st = s2;
            }
            Eof => {
                self.need(&st, 1, c)?;
                let k = self.arg_sym(&st, 0)?;
                let h = self.handle(k);
                let (s2, v) = self.fx(st, "feof", Ex::Str(Rc::from(h.as_str())), Ex::Str(Rc::from("")), Ex::Str(Rc::from("?eof: ")))?;
                st = s2;
                let a = self.bind_a(&mut st, Kind::Num, Ex::App(P::Parse, vec![Ex::Var(v)]));
                self.mset(&mut st, (S_EOF, GL), a);
            }
            Move => {
                self.need(&st, 2, c)?;
                let (a, b) = (st.args[0].clone(), st.args[1].clone());
                let (ae, be) = (self.ex_text(&a), self.ex_text(&b));
                let pre = Ex::App(P::Cat, vec![Ex::Str(Rc::from("mv: ")), ae.clone(), Ex::Str(Rc::from(" ")), be.clone(), Ex::Str(Rc::from(": "))]);
                let (s2, _) = self.fx(st, "mv", ae, be, pre)?;
                st = s2;
            }
            Write | Append => {
                self.need(&st, 2, c)?;
                let (p, d) = (st.args[0].clone(), st.args[1].clone());
                let pe_ = self.ex_text(&p);
                let w = if c == Write { "w" } else { "+w" };
                let pre = Ex::App(P::Cat, vec![Ex::Str(Rc::from(format!("{}: ", w).as_str())), pe_.clone(), Ex::Str(Rc::from(": "))]);
                let de = self.ex_text(&d);
                let (s2, _) = self.fx(st, if c == Write { "w" } else { "a" }, pe_, de, pre)?;
                st = s2;
            }
            Shell => {
                self.need(&st, 1, c)?;
                let k = st.args[0].clone();
                let ke = self.ex_text(&k);
                let pre = Ex::App(P::Cat, vec![Ex::Str(Rc::from("<: ")), ke.clone(), Ex::Str(Rc::from(": "))]);
                let (s2, v) = self.fx(st, "sh", ke, Ex::Str(Rc::from("")), pre)?;
                st = s2;
                let out = self.bind_a(&mut st, Kind::Any, Ex::App(P::VOf, vec![Ex::App(P::ShOut, vec![Ex::Var(v)])]));
                self.mset(&mut st, (S_R, GL), out);
                let rc = self.bind_a(&mut st, Kind::Num, Ex::App(P::ShRc, vec![Ex::Var(v)]));
                self.nset(&mut st, (S_RC, GL), rc);
            }
            PrngN | PrngS => {
                let seed = match st.args.len() {
                    0 => Ex::Str(Rc::from("")),
                    1 => {
                        let a = st.args[0].clone();
                        let n = self.as_num(&mut st, &a);
                        self.ex_text(&n)
                    }
                    _ => {
                        let (a, b) = (st.args[0].clone(), st.args[1].clone());
                        let (a, b) = (self.as_num(&mut st, &a), self.as_num(&mut st, &b));
                        Ex::App(P::Cat, vec![self.ex_text(&a), Ex::Str(Rc::from(" ")), self.ex_text(&b)])
                    }
                };
                let d = self.dest(&st, 2, S_RN)?;
                let (s2, v) = self.fx(st, if c == PrngN { "pn" } else { "ps" }, Ex::Str(Rc::from("")), seed, Ex::Str(Rc::from("")))?;
                st = s2;
                if c == PrngN {
                    let x = self.bind_a(&mut st, Kind::Num, Ex::App(P::Parse, vec![Ex::Var(v)]));
                    self.nset(&mut st, d, x);
                } else {
                    let x = A::D(D { v, k: Kind::Str, m: 0 });
                    self.mset(&mut st, d, x);
                }
            }
            Match => {
                self.need(&st, 2, c)?;
                let table = self.static_text(&st.args[0].clone(), "a ?) table")?;
                let def = match st.args.get(2).cloned() {
                    Some(a) => Some(self.static_text(&a, "a ?) default")?),
                    None => None,
                };
                let key = st.args[1].clone();
                st.args.clear();
                let key = match key {
                    A::K(v) => v.text().into_owned(),
                    A::D(_) => return self.match_dyn(st, &table, key, def),
                };
                let mut ws = table.split_whitespace();
                let mut hit: Option<String> = None;
                while let (Some(k), Some(v)) = (ws.next(), ws.next()) {
                    if k == key {
                        hit = Some(v.to_string());
                        break;
                    }
                }
                if let Some(code) = hit.or(def) {
                    st = self.frame_src(st, &code)?;
                }
                return Ok((st, Fl::End));
            }
            Repeat => {
                self.need(&st, 2, c)?;
                let n = self.count(&st, 0, "&&&")?;
                let k = self.arg_sym(&st, 1)?;
                st.args.clear();
                st.fs += 1;
                let d = st.depth();
                st.reps.push((A::K(Val::N(0.0)), n));
                let mut s = self.rep_loop_amp(st, k)?;
                s.reps.pop();
                self.clear_depth(&mut s, d);
                s.fs -= 1;
                st = s;
            }
            SetN => {
                self.need(&st, 2, c)?;
                let x = st.args[0].clone();
                let x = self.as_num(&mut st, &x);
                let k = self.arg_sym(&st, 1)?;
                self.nset(&mut st, k, x);
            }
            Add | Sub | Mul | Div | Pow | Mod => {
                self.need(&st, 2, c)?;
                let (a, b) = (st.args[0].clone(), st.args[1].clone());
                let r = self.arith2(&mut st, c, &a, &b);
                let d = self.dest(&st, 2, S_EQ)?;
                self.nset(&mut st, d, r);
            }
            Not => {
                st.ow = match st.ow {
                    AB::K(b) => AB::K(!b),
                    AB::D(v) => AB::D(self.bind(&mut st, Ty::B, Ex::App(P::Not, vec![Ex::Var(v)]))),
                }
            }
            Eq | Ne | AndNe | AndEq | NumEq | Lt | Gt | Le | Ge => {
                self.need(&st, 2, c)?;
                self.cmp(&mut st, c);
            }
            LPush => {
                self.need(&st, 2, c)?;
                let n = st.args.len();
                let k = self.arg_sym(&st, n - 1)?;
                self.note(&mut st, k);
                let vs: Vec<A> = st.args[..n - 1].to_vec();
                match self.list_of(&st, k) {
                    AL::K(_) => {
                        let len = {
                            let l = Rc::make_mut(&mut st.lists).entry(k).or_insert_with(|| AL::K(Rc::new(Vec::new())));
                            match l {
                                AL::K(xs) => {
                                    let xs = Rc::make_mut(xs);
                                    xs.extend(vs);
                                    xs.len()
                                }
                                AL::C(..) => unreachable!("a named list never holds a line"),
                                AL::D(_) => 0,
                            }
                        };
                        if len > KLIST {
                            self.dyn_list(&mut st, k);
                        }
                    }
                    AL::D(l) => {
                        let mut e = Ex::Var(l);
                        for v in &vs {
                            e = Ex::App(P::LPush, vec![e, self.ex_v(v)]);
                        }
                        let l2 = self.bind(&mut st, Ty::L, e);
                        self.set_list(&mut st, k, AL::D(l2));
                    }
                    AL::C(..) => unreachable!("a named list never holds a line"),
                }
            }
            LPop => {
                self.need(&st, 1, c)?;
                let k = self.arg_sym(&st, 0)?;
                self.note(&mut st, k);
                let v = match self.list_of(&st, k) {
                    AL::K(xs) => {
                        let mut xs = (*xs).clone();
                        let v = xs.pop().unwrap_or(A::K(Val::L("404")));
                        self.set_list(&mut st, k, AL::K(Rc::new(xs)));
                        v
                    }
                    AL::C(..) => unreachable!("a named list never holds a line"),
                    AL::D(l) => {
                        let v = self.bind_a(&mut st, Kind::Any, Ex::App(P::LLast, vec![Ex::Var(l)]));
                        let l2 = self.bind(&mut st, Ty::L, Ex::App(P::LPop, vec![Ex::Var(l)]));
                        self.set_list(&mut st, k, AL::D(l2));
                        v
                    }
                };
                self.mset(&mut st, (S_DOLLAR, GL), v);
            }
            LGet => {
                self.need(&st, 3, c)?;
                let i = st.args[1].clone();
                let d = self.arg_sym(&st, 2)?;
                let mut idom: Option<Vec<Val>> = None;
                let l = match st.args[0].clone() {
                    a @ A::D(_) => {
                        idom = self.named_items(&st, &a);
                        let e = self.list_named(&st, &a);
                        let v = self.bind(&mut st, Ty::L, e);
                        AL::D(v)
                    }
                    _ => {
                        let k = self.arg_sym(&st, 0)?;
                        let l = self.list_of(&st, k);
                        if let AL::K(xs) = &l {
                            idom = self.items_dom(xs);
                        }
                        l
                    }
                };
                match (&l, &i) {
                    (AL::K(xs), A::K(iv)) => {
                        let i = iv.num() as i64;
                        let i = if i < 0 { i + xs.len() as i64 } else { i };
                        if i >= 0 && (i as usize) < xs.len() {
                            let v = xs[i as usize].clone();
                            self.mset(&mut st, d, v);
                        }
                    }
                    _ => {
                        let le = self.ex_list(&l);
                        let ie = self.ex_num(&i);
                        if st.nums.contains_key(&d) {
                            return self.no("a list item read from input into a name that holds a number");
                        }
                        let old = match st.mem.get(&d).cloned() {
                            Some(v) if st.maybe.contains(&(d, 0)) => {
                                let none = A::K(Val::L("404"));
                                match &v {
                                    A::K(Val::L(t)) if *t == ABSENT => none,
                                    A::K(_) => v,
                                    A::D(_) => self.or_absent(&mut st, &v, &none),
                                }
                            }
                            Some(v) => v,
                            None => A::K(Val::L("404")),
                        };
                        let e = Ex::App(P::If, vec![Ex::App(P::LHas, vec![le.clone(), ie.clone()]), Ex::App(P::LGet, vec![le, ie]), self.ex_v(&old)]);
                        let mut v = self.bind_a(&mut st, Kind::Any, e);
                        if let (Some(mut dd), Some(mut od)) = (idom, self.dom_of(&old)) {
                            dd.append(&mut od);
                            let m = self.mk_dom(dd);
                            if let A::D(x) = &mut v {
                                x.m = m;
                            }
                        }
                        self.mset(&mut st, d, v);
                    }
                }
            }
            LSet => {
                self.need(&st, 3, c)?;
                let v = st.args[0].clone();
                let k = self.arg_sym(&st, 1)?;
                self.note(&mut st, k);
                let i = st.args[2].clone();
                match (self.list_of(&st, k), &i) {
                    (AL::K(xs), A::K(iv)) => {
                        let i = iv.num() as i64;
                        let i = if i < 0 { i + xs.len() as i64 } else { i };
                        if i >= 0 && (i as usize) < xs.len() {
                            let mut xs = (*xs).clone();
                            xs[i as usize] = v;
                            self.set_list(&mut st, k, AL::K(Rc::new(xs)));
                        }
                    }
                    _ => {
                        let l = self.dyn_list(&mut st, k);
                        let e = Ex::App(P::LSet, vec![Ex::Var(l), self.ex_num(&i), self.ex_v(&v)]);
                        let l2 = self.bind(&mut st, Ty::L, e);
                        self.set_list(&mut st, k, AL::D(l2));
                    }
                }
            }
            LDel => {
                self.need(&st, 1, c)?;
                let k = self.arg_sym(&st, 0)?;
                self.note(&mut st, k);
                if st.lists.contains_key(&k) {
                    Rc::make_mut(&mut st.lists).remove(&k);
                }
            }
            LLen => {
                self.need(&st, 1, c)?;
                let l = match st.args[0].clone() {
                    a @ A::D(_) => {
                        let e = self.list_named(&st, &a);
                        AL::D(self.bind(&mut st, Ty::L, e))
                    }
                    _ => {
                        let k = self.arg_sym(&st, 0)?;
                        self.list_of(&st, k)
                    }
                };
                let n = match l {
                    AL::K(xs) => A::K(Val::N(xs.len() as f64)),
                    AL::C(..) => unreachable!("a named list never holds a line"),
                    AL::D(l) => self.bind_a(&mut st, Kind::Num, Ex::App(P::LLen, vec![Ex::Var(l)])),
                };
                let d = self.dest(&st, 1, S_EQ)?;
                self.nset(&mut st, d, n);
            }
            Split => {
                self.need(&st, 2, c)?;
                let text = st.args[0].clone();
                let k = self.arg_sym(&st, 1)?;
                let sep = st.args.get(2).cloned();
                self.note(&mut st, k);
                match (&text, &sep) {
                    (A::K(t), None) => {
                        let xs: Vec<A> = t.text().split_whitespace().map(|s| A::K(Val::of(s.to_string()))).collect();
                        self.set_list(&mut st, k, AL::K(Rc::new(xs)));
                    }
                    (A::K(t), Some(A::K(s))) => {
                        let (t, s) = (t.text().into_owned(), s.text().into_owned());
                        let xs: Vec<A> = if s.is_empty() { t.chars().map(|ch| A::K(Val::of(ch.to_string()))).collect() } else { t.split(s.as_str()).map(|p| A::K(Val::of(p.to_string()))).collect() };
                        self.set_list(&mut st, k, AL::K(Rc::new(xs)));
                    }
                    _ => {
                        let te = self.ex_text(&text);
                        let e = match &sep {
                            None => Ex::App(P::SplitWs, vec![te]),
                            Some(A::K(s)) if s.text().is_empty() => Ex::App(P::SplitChars, vec![te]),
                            Some(s) => Ex::App(P::SplitBy, vec![te, self.ex_text(s)]),
                        };
                        let l = self.bind(&mut st, Ty::L, e);
                        self.set_list(&mut st, k, AL::D(l));
                    }
                }
            }
            Join => {
                self.need(&st, 2, c)?;
                let d = self.arg_sym(&st, 1)?;
                let sep = st.args.get(2).cloned().unwrap_or(A::K(Val::L(" ")));
                let l = match st.args[0].clone() {
                    a @ A::D(_) => {
                        let e = self.list_named(&st, &a);
                        AL::D(self.bind(&mut st, Ty::L, e))
                    }
                    _ => {
                        let k = self.arg_sym(&st, 0)?;
                        self.list_of(&st, k)
                    }
                };
                let v = match (&l, &sep) {
                    (AL::K(xs), A::K(s)) if xs.iter().all(|x| matches!(x, A::K(_))) => {
                        let ts: Vec<String> = xs.iter().map(|x| if let A::K(v) = x { v.text().into_owned() } else { String::new() }).collect();
                        A::K(Val::of(ts.join(&s.text())))
                    }
                    _ => {
                        let e = Ex::App(P::VOf, vec![Ex::App(P::Join, vec![self.ex_list(&l), self.ex_text(&sep)])]);
                        self.bind_a(&mut st, Kind::Any, e)
                    }
                };
                self.mset(&mut st, d, v);
            }
            SLen => {
                self.need(&st, 1, c)?;
                let n = match &st.args[0] {
                    A::K(v) => A::K(Val::N(v.text().chars().count() as f64)),
                    a => {
                        let e = Ex::App(P::SLen, vec![self.ex_text(&a.clone())]);
                        self.bind_a(&mut st, Kind::Num, e)
                    }
                };
                let d = self.dest(&st, 1, S_EQ)?;
                self.nset(&mut st, d, n);
            }
            Replace => {
                self.need(&st, 2, c)?;
                let key = self.static_text(&st.args[1].clone(), "a replacement's word")?;
                let v = match &st.args[0] {
                    A::K(v) => v.clone(),
                    A::D(_) => return self.no("a replacement's value built from input"),
                };
                let code = self.code_of(&v.text()).ok();
                if canon_num(&key).is_some() {
                    st.repl_num = true;
                }
                let gen = st.gen;
                st.gen += 1;
                self.note_repl(&mut st, &key);
                let mut ms: Vec<Mac> = st.repl.get(key.as_str()).map(|v| (**v).clone()).unwrap_or_default();
                ms.push(Mac { val: v, code, nat: None, gen });
                Rc::make_mut(&mut st.repl).insert(Rc::from(key.as_str()), Rc::new(ms));
            }
            Open1 => {
                self.need(&st, 1, c)?;
                let n = st.args.len() - 1;
                let k = match self.count(&st, n, "(")? {
                    A::K(v) => v.num() as usize,
                    kd => {
                        // a count from input: the line cut where input says, the first part
                        // put aside, the last k values kept
                        st.args.pop();
                        let full = self.ex_list(&line_al(st.low, &st.args));
                        let lf = self.bind(&mut st, Ty::L, full);
                        let m = Ex::App(P::Sub, vec![Ex::App(P::LLen, vec![Ex::Var(lf)]), self.ex_num(&kd)]);
                        let mv = self.bind(&mut st, Ty::F, m);
                        let aside = self.bind(&mut st, Ty::L, Ex::App(P::LTake, vec![Ex::Var(lf), Ex::Var(mv)]));
                        let kept = self.bind(&mut st, Ty::L, Ex::App(P::LDrop, vec![Ex::Var(lf), Ex::Var(mv)]));
                        st.args.clear();
                        st.low = Some(kept);
                        st.frames.push((Some(aside), Vec::new()));
                        return Ok((st, Fl::End));
                    }
                };
                st.args.pop();
                if st.low.is_some() && n < k {
                    self.line_take(&mut st, k);
                }
                let n = st.args.len();
                let keep = k.min(n);
                let rest: Vec<A> = st.args.drain(..n - keep).collect();
                let low = st.low.take();
                st.frames.push((low, rest));
                return Ok((st, Fl::End));
            }
            Close1 => {
                let (flow, mut saved) = match st.frames.pop() {
                    Some(f) => f,
                    None => return self.fail(st, String::from(")") + ": no open frame"),
                };
                if let Some(lv) = st.low {
                    // a lower part from input made inside the frame goes after what was put aside
                    if flow.is_some() || !saved.is_empty() {
                        let before = self.ex_list(&line_al(flow, &saved));
                        let l = self.bind(&mut st, Ty::L, Ex::App(P::LCat, vec![before, Ex::Var(lv)]));
                        st.low = Some(l);
                    }
                } else {
                    st.low = flow;
                    saved.append(&mut st.args);
                    st.args = saved;
                }
                return Ok((st, Fl::End));
            }
            Eval => {
                let v = match st.args.pop() {
                    Some(v) => v,
                    None => return self.fail(st, "~: stack is empty".to_string()),
                };
                let t = match v {
                    A::K(v) => v.text().into_owned(),
                    A::D(_) => return self.no("`~` runs text built from input, which a Bend build cannot know"),
                };
                let st = self.frame_src(st, &t)?;
                return Ok((st, Fl::End));
            }
            Depth => {
                let n = st.args.len() as f64;
                let d = match st.low {
                    None => A::K(Val::N(n)),
                    Some(lv) => self.bind_a(&mut st, Kind::Num, Ex::App(P::Add, vec![Ex::App(P::LLen, vec![Ex::Var(lv)]), Ex::Num(n)])),
                };
                st.args.push(d);
                return Ok((st, Fl::End));
            }
            Comment => {}
            Ret => return Ok((st, Fl::Done)),
            Exit => {
                let code = match st.args.first() {
                    Some(a) => {
                        let a = a.clone();
                        self.ex_num(&a)
                    }
                    None => Ex::Num(0.0),
                };
                st.end = Some(End::Exit(code));
                return Err(Stop::Dead(Box::new(st)));
            }
            _ => return self.no(&format!("`{}` is not built to Bend yet", c.word())),
        }
        let _ = bound;
        st.args.clear();
        Ok((st, Fl::End))
    }

    fn items_dom(&self, xs: &[A]) -> Option<Vec<Val>> {
        let mut dom: Vec<Val> = Vec::new();
        for x in xs {
            dom.extend(self.dom_of(x)?);
        }
        Some(dom)
    }
    /// The values the items of the lists a name from input can name hold, when known.
    fn named_items(&self, st: &St, a: &A) -> Option<Vec<Val>> {
        let names: Vec<String> = self.dom_of(a)?.iter().map(|v| v.text().into_owned()).collect();
        let mut dom = Vec::new();
        for (nm, l) in st.lists.iter() {
            if nm.1 == GL && names.iter().any(|n| n == self.it.name(nm.0)) {
                match l {
                    AL::K(xs) => dom.extend(self.items_dom(xs)?),
                    AL::C(..) => unreachable!("a named list never holds a line"),
                    AL::D(_) => return None,
                }
            }
        }
        Some(dom)
    }
    /// The list a name from input names, from every list the program holds by name.
    fn list_named(&mut self, st: &St, a: &A) -> Ex {
        let te = self.ex_text(a);
        let dom: Option<Vec<String>> = self.dom_of(a).map(|d| d.iter().map(|v| v.text().into_owned()).collect());
        let mut stat: Vec<(String, Ex)> = Vec::new();
        let mut dynl: Vec<(String, Ex)> = Vec::new();
        for (nm, l) in st.lists.iter() {
            if nm.1 != GL {
                continue;
            }
            let name = self.it.name(nm.0).to_string();
            if let Some(d) = &dom {
                if !d.contains(&name) {
                    continue;
                }
            }
            match l {
                AL::K(xs) if xs.iter().all(|x| matches!(x, A::K(_))) => stat.push((name, self.ex_list(l))),
                _ => dynl.push((name, self.ex_list(l))),
            }
        }
        let key = format!("{:?}", stat);
        let mi = match self.lmaps.iter().position(|(k, _)| *k == key) {
            Some(i) => i,
            None => {
                self.lmaps.push((key, stat));
                self.lmaps.len() - 1
            }
        };
        let mut args = vec![Ex::Num(mi as f64), te];
        for (n, e) in dynl {
            args.push(Ex::Str(Rc::from(n.as_str())));
            args.push(e);
        }
        Ex::App(P::LNamed, args)
    }

    /// The name a file handle is kept under.
    fn handle(&self, k: Nm) -> String {
        if k.1 == GL { self.it.name(k.0).to_string() } else { format!("{}@{}", self.it.name(k.0), k.1) }
    }
    /// An effect whose error runs a script when one is given, with err holding the message,
    /// and ends the program when none is.
    fn fx_or(&mut self, mut st: St, op: &'static str, a: Ex, b: Ex, pre: Ex, onerr: Option<String>) -> R<(St, u32)> {
        let script = match onerr {
            None => {
                let (mut s, v) = self.fx(st, op, a, b, pre)?;
                s.args.clear();
                return Ok((s, v));
            }
            Some(sc) => sc,
        };
        let raw = self.var(Ty::S);
        st.emit(Stmt::Fx(op, a, b, raw));
        self.dyn_ev += 1;
        let bad = self.bind(&mut st, Ty::B, Ex::App(P::Not, vec![Ex::App(P::FxOk, vec![Ex::Var(raw)])]));
        let base = st.code.clone();
        let mut e = st.clone();
        e.code = Buf::default();
        e.args.clear();
        let m = self.bind_a(&mut e, Kind::Any, Ex::App(P::VOf, vec![Ex::App(P::Cat, vec![pre, Ex::App(P::FxVal, vec![Ex::Var(raw)])])]));
        self.mset(&mut e, (S_ERR, GL), m);
        let re = self.frame_src(e, &script);
        let re = self.path_result(re)?;
        let mut live = st;
        live.code = Buf::default();
        live.args.clear();
        let m = self.join(base, bad, re, Ok(live))?;
        Ok((m, raw))
    }

    /// An effect through S.fx: its answer, or the program ends with the error after pre.
    fn fx(&mut self, mut st: St, op: &'static str, a: Ex, b: Ex, pre: Ex) -> R<(St, u32)> {
        let raw = self.var(Ty::S);
        st.emit(Stmt::Fx(op, a, b, raw));
        self.dyn_ev += 1;
        let ok = self.bind(&mut st, Ty::B, Ex::App(P::Not, vec![Ex::App(P::FxOk, vec![Ex::Var(raw)])]));
        let msg = Ex::App(P::Cat, vec![pre, Ex::App(P::FxVal, vec![Ex::Var(raw)])]);
        let base = st.code.clone();
        let mut dead = st.clone();
        dead.code = Buf::default();
        let msg = match st.lane_at {
            Some(i) => Ex::App(P::Cat, vec![Ex::Str(Rc::from(format!("lane {}: ", i).as_str())), msg]),
            None => msg,
        };
        dead.end = Some(End::Fail(msg));
        let mut live = st;
        live.code = Buf::default();
        let mut m = self.join(base, ok, Err(dead), Ok(live))?;
        let v = self.bind(&mut m, Ty::S, Ex::App(P::FxVal, vec![Ex::Var(raw)]));
        Ok((m, v))
    }

    /// `?)` on a key from input: one path per entry of the table, tried in order.
    fn match_dyn(&mut self, st: St, table: &str, key: A, def: Option<String>) -> R<(St, Fl)> {
        let ws: Vec<&str> = table.split_whitespace().collect();
        let mut entries: Vec<(String, String)> = Vec::new();
        let mut i = 0;
        while i + 1 < ws.len() {
            if !entries.iter().any(|(k, _)| k == ws[i]) {
                entries.push((ws[i].to_string(), ws[i + 1].to_string()));
            }
            i += 2;
        }
        if let Some(dom) = self.dom_of(&key) {
            let texts: Vec<String> = dom.iter().map(|v| v.text().into_owned()).collect();
            let all = texts.iter().all(|t| entries.iter().any(|(k, _)| k == t));
            entries.retain(|(k, _)| texts.contains(k));
            if all && !entries.is_empty() {
                // no key falls through to the default: the last entry needs no test
                let last = entries.pop().unwrap();
                let st = self.match_from(st, &entries, 0, &key, &Some(last.1))?;
                return Ok((st, Fl::End));
            }
        }
        let st = self.match_from(st, &entries, 0, &key, &def)?;
        Ok((st, Fl::End))
    }
    fn match_from(&mut self, mut st: St, entries: &[(String, String)], i: usize, key: &A, def: &Option<String>) -> R<St> {
        if i == entries.len() {
            return match def {
                Some(code) => self.frame_src(st, code),
                None => Ok(st),
            };
        }
        let e = self.eq_ex(key, &A::K(Val::of(entries[i].0.clone())));
        let c = self.bind(&mut st, Ty::B, e);
        let code = entries[i].1.clone();
        self.dyn_ev += 1;
        let base = st.code.clone();
        let mut a = st.clone();
        a.code = Buf::default();
        let mut b = st;
        b.code = Buf::default();
        let ra = self.frame_src(a, &code);
        let ra = self.path_result(ra)?;
        let rb = self.match_from(b, entries, i + 1, key, def);
        let rb = self.path_result(rb)?;
        self.join(base, c, ra, rb)
    }

    fn rep_loop_nm(&mut self, st: St, k: Nm, amp: bool) -> R<St> {
        self.rep_loop(st, k, amp, false)
    }
    fn rep_loop_amp(&mut self, st: St, k: Nm) -> R<St> {
        self.rep_loop(st, k, true, false)
    }
}

// ---------- lanes ----------

impl Pe {
    fn end_view(&mut self, st: &mut St) {
        if let Some(v) = st.view.take() {
            for (s, old) in v.iter().rev() {
                self.put_slot(st, *s, old.clone());
            }
            st.gen += 1;
        }
    }
    fn close_log(&mut self, st: &mut St, log: Rc<Log>) -> LW {
        let mut w = LW::default();
        let log = (*log).clone();
        for (s, old) in log.old.into_iter().rev() {
            let cur = self.slot(st, s);
            let empty = cur.mem.is_none() && cur.num.is_none() && cur.list.is_none() && cur.script.is_none();
            let was_empty = old.mem.is_none() && old.num.is_none() && old.list.is_none() && old.script.is_none();
            if !(empty && was_empty) {
                w.names.push((s, cur));
            }
            self.put_slot(st, s, old);
        }
        w.names.reverse();
        for (k, old) in log.repl_old.into_iter().rev() {
            let cur = st.repl.get(&*k).cloned();
            w.repl.push((k.clone(), cur));
            match old {
                Some(v) => {
                    Rc::make_mut(&mut st.repl).insert(k, v);
                }
                None => {
                    Rc::make_mut(&mut st.repl).remove(&*k);
                }
            }
        }
        st.gen += 1;
        w
    }
    fn apply_writes(&mut self, st: &mut St, w: &LW) -> Vec<(Nm, Slot)> {
        let mut saved = Vec::new();
        for (s, sl) in &w.names {
            self.note(st, *s);
            saved.push((*s, self.slot(st, *s)));
            self.put_slot(st, *s, sl.clone());
        }
        for (k, v) in &w.repl {
            self.note_repl(st, k);
            match v {
                Some(v) => {
                    Rc::make_mut(&mut st.repl).insert(k.clone(), v.clone());
                }
                None => {
                    Rc::make_mut(&mut st.repl).remove(&**k);
                }
            }
        }
        st.gen += 1;
        saved
    }

    /// FORK … JOIN: each lane runs on its own line and its own copy of the names, and the
    /// lanes are held at the join.
    fn lanes_op(&mut self, mut st: St, f: Rc<crate::lanes::Fork>) -> R<St> {
        self.end_view(&mut st);
        let base = std::mem::take(&mut st.args);
        let spread = match f.spread {
            Some(k) => Some(self.l(&st, k)?),
            None => None,
        };
        let items: Vec<A> = match spread {
            Some(k) => match self.list_of(&st, k) {
                AL::K(xs) if xs.len() <= FORK_MAX => (*xs).clone(),
                _ => return self.lanes_map(st, &f, k, base),
            },
            None => Vec::new(),
        };
        st.jdyn = None;
        let n = if spread.is_some() { items.len() } else { f.texts.len() };
        let ow0 = st.ow;
        let outer_log = st.log.take();
        let outer_at = st.lane_at;
        st.fs += 1;
        let fd = st.depth();
        let mut rows = Vec::with_capacity(n);
        // each lane's code is gathered apart: lanes with no effects run as parallel calls
        let main_code = std::mem::take(&mut st.code);
        let mut lane_code: Vec<(Vec<Stmt>, String)> = Vec::with_capacity(n);
        // every lane starts from the names as they were at the fork, as the machine's do
        let snap = (st.mem.clone(), st.nums.clone(), st.lists.clone(), st.maybe.clone());
        for i in 0..n {
            st.mem = snap.0.clone();
            st.nums = snap.1.clone();
            st.lists = snap.2.clone();
            st.maybe = snap.3.clone();
            let mut line = base.clone();
            st.log = Some(Rc::new(Log::default()));
            st.lane_at = Some(i);
            st.ow = ow0;
            let text = if spread.is_some() {
                line.push(items[i].clone());
                f.shared.clone()
            } else {
                f.texts[i].clone()
            };
            st.args = line;
            st.code = Buf::default();
            if !text.trim().is_empty() {
                st = match self.frame_src(st, &text) {
                    Ok(s) => s,
                    Err(Stop::Dead(mut d)) => {
                        // the lanes before ran in order, then this one ended the program
                        let mut code = main_code.clone();
                        for (stmts, _) in &lane_code {
                            for x in stmts {
                                code.push(x.clone());
                            }
                        }
                        for x in d.code.to_vec() {
                            code.push(x);
                        }
                        d.code = code;
                        return Err(Stop::Dead(d));
                    }
                    Err(e) => return Err(e),
                };
            }
            lane_code.push((std::mem::take(&mut st.code).to_vec(), text.to_string()));
            let log = st.log.take().unwrap_or_default();
            let w = self.close_log(&mut st, log);
            let args = std::mem::take(&mut st.args);
            rows.push(Row { line: args, w });
        }
        st.code = main_code;
        let busy = lane_code.iter().filter(|(c, _)| !c.is_empty()).count();
        let pure = lane_code.iter().all(|(c, _)| self.pure_code(c, &mut Vec::new()));
        if busy >= 2 && pure {
            let mut calls = Vec::new();
            for (stmts, text) in lane_code {
                if stmts.is_empty() {
                    continue;
                }
                let outs: Vec<(u32, Ty)> = top_defs(&stmts).into_iter().map(|v| (v, self.tys[v as usize])).collect();
                let end = End::Yield(outs.iter().map(|o| Ex::Var(o.0)).collect());
                let params: Vec<(u32, Ty)> = free_vars(&stmts, &end).into_iter().map(|v| (v, self.tys[v as usize])).collect();
                let fid = self.funs.len();
                let code = self.code_of(&text)?;
                self.funs.push(FunInfo {
                    key: String::new(),
                    g: st.clone(),
                    gb: Base { d: st.depth(), r: st.reps.len() },
                    exit: Ex0::Never,
                    fun: Some(Fun { name: format!("f{}", fid), params: params.clone(), outs: outs.iter().map(|o| o.1).collect(), body: Block { stmts, end }, what: format!("a lane of {}", text.chars().take(50).collect::<String>()) }),
                    body: HB::Code(code, UNBOUND),
                    used: false,
                    provisional: false,
                });
                calls.push((fid, params.iter().map(|p| Ex::Var(p.0)).collect(), outs));
            }
            st.emit(Stmt::Par { calls });
        } else {
            for (stmts, _) in lane_code {
                for x in stmts {
                    st.emit(x);
                }
            }
        }
        st.log = outer_log;
        st.lane_at = outer_at;
        st.ow = ow0;
        st.args = base;
        st.joined = Some(Rc::new(rows));
        st.gone = false;
        self.clear_depth(&mut st, fd);
        st.fs -= 1;
        Ok(st)
    }

    /// A fork over a list from input: the lane is one function of the state at the fork and
    /// its item, mapped over the list; the join holds a list per value the lane keeps.
    fn lanes_map(&mut self, mut st: St, f: &crate::lanes::Fork, k: Nm, base: Vec<A>) -> R<St> {
        let lv = self.dyn_list(&mut st, k);
        let ow0 = st.ow;
        let mut entry = st.clone();
        entry.args = base.clone();
        entry.fs += 1;
        entry.log = Some(Rc::new(Log::default()));
        entry.lane_at = None;
        entry.code = Buf::default();
        let b = Base { d: entry.depth() + 1, r: entry.reps.len() };
        let z = match self.zip(&[&entry], Some(&[b]), Mode::Gen) {
            Ok(z) => z,
            Err(m) => return self.no(&m),
        };
        let mut g = z.st;
        self.strip(&mut g, b);
        let iv = self.var(Ty::V);
        g.args.push(A::D(D { v: iv, k: Kind::Any, m: 0 }));
        let mut ex = match self.frame_src(g.clone(), &f.shared) {
            Ok(s) => s,
            Err(Stop::Dead(_)) => return self.no("a lane over a list from input that ends the program"),
            Err(e) => return Err(e),
        };
        let log = ex.log.take().unwrap_or_default();
        let w = self.close_log(&mut ex, log);
        let mut vals = Vec::new();
        for a in &ex.args {
            vals.push(self.ex_v(a));
        }
        let nl = ex.args.len();
        let mut wr: Vec<(Nm, bool)> = Vec::new();
        for (nm, sl) in &w.names {
            if sl.list.is_some() || sl.script.is_some() {
                return self.no("a lane over a list from input that writes a list or a script");
            }
            if let Some(a) = &sl.mem {
                vals.push(self.ex_v(a));
                wr.push((*nm, true));
            }
            if let Some(a) = &sl.num {
                vals.push(self.ex_v(a));
                wr.push((*nm, false));
            }
        }
        let mut params = z.outs.clone();
        params.push((iv, Ty::V));
        let what = format!("the lane of {}", f.shared.chars().take(50).collect::<String>());
        let fid = self.funs.len();
        let lane_code = self.code_of(&f.shared)?;
        let outs_t = vec![Ty::V; vals.len()];
        self.funs.push(FunInfo {
            key: String::new(),
            g,
            gb: b,
            exit: Ex0::Never,
            fun: Some(Fun { name: format!("f{}", fid), params, outs: outs_t, body: Block { stmts: ex.code.to_vec(), end: End::Yield(vals.clone()) }, what }),
            body: HB::Code(lane_code, UNBOUND),
            used: false,
            provisional: false,
        });
        let outs: Vec<(u32, Ty)> = (0..vals.len()).map(|_| (self.var(Ty::L), Ty::L)).collect();
        st.emit(Stmt::Map { f: fid, xs: Ex::Var(lv), args: z.vals[0].clone(), outs: outs.clone() });
        self.dyn_ev += 1;
        let n = self.bind_a(&mut st, Kind::Num, Ex::App(P::LLen, vec![Ex::Var(lv)]));
        let lines = outs[..nl].iter().map(|o| o.0).collect();
        let writes = wr.iter().zip(outs[nl..].iter()).map(|((nm, m), o)| (*nm, *m, o.0)).collect();
        st.jdyn = Some(Rc::new(JD { n, lines, writes }));
        st.joined = None;
        st.gone = false;
        st.args = base;
        st.ow = ow0;
        st.gen += 1;
        Ok(st)
    }

    /// Code with no effects and no end of the program in it, through the functions it calls.
    fn pure_code(&self, stmts: &[Stmt], seen: &mut Vec<usize>) -> bool {
        // an error is the one effect a lane may have: it stops the program as the machine's does
        let block = |b: &Block, seen: &mut Vec<usize>| matches!(b.end, End::Yield(_) | End::Fail(_)) && self.pure_code(&b.stmts, seen);
        stmts.iter().all(|s| match s {
            Stmt::Let(..) => true,
            Stmt::Write(_) | Stmt::ReadLine(..) | Stmt::Args(..) | Stmt::Fx(..) | Stmt::Spawn { .. } | Stmt::Join { .. } => false,
            Stmt::Branch { t, e, .. } => block(t, seen) && block(e, seen),
            Stmt::Call { f, .. } | Stmt::Map { f, .. } => self.pure_fun(*f, seen),
            Stmt::Par { calls } => calls.iter().all(|c| self.pure_fun(c.0, seen)),
        })
    }
    fn pure_fun(&self, f: usize, seen: &mut Vec<usize>) -> bool {
        if seen.contains(&f) {
            return true;
        }
        seen.push(f);
        match self.funs.get(f).and_then(|x| x.fun.as_ref()) {
            Some(fun) => matches!(fun.body.end, End::Yield(_) | End::Fail(_)) && self.pure_code(&fun.body.stmts, seen),
            None => false,
        }
    }
    /// A worker file is built in: when the program runs, it stops if the file on disk is no
    /// longer the one it was built from, where the machine would run the new one.
    fn watch_file(&mut self, st: St, who: &str, p: &str) -> R<St> {
        let bytes = match std::fs::read(code_path(p)) {
            Ok(b) => b,
            Err(_) => return Ok(st),
        };
        let mut h: u64 = 0xcbf29ce484222325;
        for b in &bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        let pre = Ex::Str(Rc::from(format!("{}: {}: ", who, p).as_str()));
        let (s2, _) = self.fx(st, "chk", Ex::Str(Rc::from(p)), Ex::Str(Rc::from(format!("{:016x}", h).as_str())), pre)?;
        Ok(s2)
    }
    fn joined_or(&mut self, st: &St, who: &str) -> R<Rc<Vec<Row>>> {
        if st.gone {
            return self.no(&format!("`{}` on lanes held from before a loop, read before the loop forks again", who));
        }
        match &st.joined {
            Some(j) => Ok(j.clone()),
            None => self.fail(st.clone(), format!("{}: no joined lanes", who)),
        }
    }
    fn pop(&mut self, st: &mut St, who: &str) -> R<A> {
        match st.args.pop() {
            Some(v) => Ok(v),
            None => self.fail(st.clone(), format!("{}: stack is empty", who)),
        }
    }

    fn native(&mut self, mut st: St, nat: Nat, _gen: u64) -> R<St> {
        if let Some(j) = st.jdyn.clone() {
            match nat {
                Nat::Lanes => {
                    st.args.push(j.n.clone());
                    return Ok(st);
                }
                Nat::Into => {
                    let v = self.pop(&mut st, "into")?;
                    let k = self.word_sym(&v)?;
                    let k = self.l(&st, k)?;
                    self.note(&mut st, k);
                    let l = match j.lines.len() {
                        0 => AL::K(Rc::new(Vec::new())),
                        1 => AL::D(j.lines[0]),
                        _ => return self.no("`into` over lanes from input whose lines hold more than one value"),
                    };
                    self.set_list(&mut st, k, l);
                    return Ok(st);
                }
                Nat::Lane => {
                    let i = self.pop(&mut st, "lane")?;
                    self.end_view(&mut st);
                    let ie = self.ex_num(&i);
                    let mut saved = Vec::new();
                    for (nm, m, l) in &j.writes {
                        self.note(&mut st, *nm);
                        saved.push((*nm, self.slot(&st, *nm)));
                        let v = self.bind_a(&mut st, Kind::Any, Ex::App(P::LGet, vec![Ex::Var(*l), ie.clone()]));
                        if *m {
                            Rc::make_mut(&mut st.mem).insert(*nm, v);
                        } else {
                            let x = self.as_num(&mut st, &v);
                            Rc::make_mut(&mut st.nums).insert(*nm, x);
                        }
                    }
                    st.gen += 1;
                    st.view = Some(Rc::new(saved));
                    for l in &j.lines {
                        let v = self.bind_a(&mut st, Kind::Any, Ex::App(P::LGet, vec![Ex::Var(*l), ie.clone()]));
                        st.args.push(v);
                    }
                    return Ok(st);
                }
                Nat::EndLane | Nat::Try => {}
                _ => return self.no("this word over lanes from input is not built to Bend yet"),
            }
        }
        match nat {
            Nat::Lanes => {
                let j = self.joined_or(&st, "lanes")?;
                st.args.push(A::K(Val::N(j.len() as f64)));
            }
            Nat::Lane => {
                let x = self.pop(&mut st, "lane")?;
                let x = match x {
                    A::K(v) => v.num(),
                    A::D(_) => return self.no("a lane number from input"),
                };
                if !(x >= 0.0 && x.fract() == 0.0) {
                    return self.fail(st, format!("lane: a lane is a whole number >= 0, got {}", fmt_num(x)));
                }
                let i = x as usize;
                self.end_view(&mut st);
                let j = self.joined_or(&st, "lane")?;
                if i >= j.len() {
                    let m = format!("lane: no lane {}, there are {}", i, j.len());
                    return self.fail(st, m);
                }
                let saved = self.apply_writes(&mut st, &j[i].w);
                st.view = Some(Rc::new(saved));
                st.args.extend(j[i].line.iter().cloned());
            }
            Nat::EndLane => self.end_view(&mut st),
            Nat::Collect => {
                let j = self.joined_or(&st, "collect")?;
                for r in j.iter() {
                    st.args.extend(r.line.iter().cloned());
                }
            }
            Nat::Into => {
                let v = self.pop(&mut st, "into")?;
                let k = self.word_sym(&v)?;
                let k = self.l(&st, k)?;
                let j = self.joined_or(&st, "into")?;
                let mut o = Vec::new();
                for r in j.iter() {
                    o.extend(r.line.iter().cloned());
                }
                self.note(&mut st, k);
                self.set_list(&mut st, k, AL::K(Rc::new(o)));
            }
            Nat::Reduce => {
                let q = self.pop(&mut st, "reduce")?;
                let q = self.static_text(&q, "reduce's code")?;
                let j = self.joined_or(&st, "reduce")?;
                if j.is_empty() {
                    return Ok(st);
                }
                st.args.extend(j[0].line.iter().cloned());
                st.fs += 1;
                let d = st.depth();
                for r in j.iter().skip(1) {
                    st.args.extend(r.line.iter().cloned());
                    st = self.frame_src(st, &q)?;
                }
                self.clear_depth(&mut st, d);
                st.fs -= 1;
            }
            Nat::Lanewise => {
                let q = self.pop(&mut st, "lanewise")?;
                let q = self.static_text(&q, "lanewise's code")?;
                let j = self.joined_or(&st, "lanewise")?;
                self.end_view(&mut st);
                let base = std::mem::take(&mut st.args);
                let outer_log = st.log.take();
                let outer_at = st.lane_at;
                let ow0 = st.ow;
                st.fs += 1;
                let d = st.depth();
                let mut rows = Vec::new();
                for (i, r) in j.iter().enumerate() {
                    st.log = Some(Rc::new(Log::default()));
                    st.lane_at = Some(i);
                    st.ow = ow0;
                    self.apply_writes(&mut st, &r.w);
                    st.args = r.line.clone();
                    st = self.frame_src(st, &q)?;
                    let log = st.log.take().unwrap_or_default();
                    let w = self.close_log(&mut st, log);
                    let args = std::mem::take(&mut st.args);
                    rows.push(Row { line: args, w });
                }
                st.log = outer_log;
                st.lane_at = outer_at;
                st.ow = ow0;
                st.args = base;
                st.joined = Some(Rc::new(rows));
        st.gone = false;
                st.jdyn = None;
                self.clear_depth(&mut st, d);
                st.fs -= 1;
            }
            Nat::Try => {
                let h = self.pop(&mut st, "try")?;
                let h = self.static_text(&h, "try's handler")?;
                let body = self.pop(&mut st, "try")?;
                let body = self.static_text(&body, "try's body")?;
                st.args.clear();
                let frames = st.frames.len();
                st.fs += 1;
                let cd = st.depth();
                let log0 = st.log.as_ref().map(|l| Rc::as_ptr(l) as usize);
                match self.frame_src(st, &body) {
                    Ok(mut s) => {
                        self.clear_depth(&mut s, cd);
                        s.fs -= 1;
                        return Ok(s);
                    }
                    Err(Stop::Dead(s)) => {
                        let mut s = *s;
                        let msg = match s.end.take() {
                            Some(End::Fail(m)) => m,
                            Some(e) => {
                                s.end = Some(e);
                                return Err(Stop::Dead(Box::new(s)));
                            }
                            None => unreachable!(),
                        };
                        if s.log.as_ref().map(|l| Rc::as_ptr(l) as usize) != log0 && s.log.is_some() {
                            return self.no("an error inside lanes inside a try");
                        }
                        while s.fs > cd {
                            let d = s.depth();
                            self.clear_depth(&mut s, d);
                            s.fs -= 1;
                        }
                        self.clear_depth(&mut s, cd);
                        s.fs = cd;
                        s.frames.truncate(frames);
                        s.args.clear();
                        let mv = match msg {
                            Ex::Str(t) => A::K(Val::of(t.to_string())),
                            e => self.bind_a(&mut s, Kind::Any, Ex::App(P::VOf, vec![e])),
                        };
                        self.mset(&mut s, (S_ERR, GL), mv);
                        return self.frame_src(s, &h);
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(st)
    }
}

// ---------- states compared, joined and made general ----------

/// The part of a state a head owns: `:` names from frame depth d up, repeats from r up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Base {
    d: u32,
    r: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// two paths of one frame: a variable both hold stays
    Merge,
    /// what a function gives back: every unknown is an output
    Exit,
    /// what a function takes: every unknown is a parameter
    Gen,
}

#[derive(Clone)]
enum SV {
    A(A),
    B(AB),
    L(AL),
}

pub struct Z {
    st: St,
    outs: Vec<(u32, Ty)>,
    vals: Vec<Vec<Ex>>,
}

#[derive(Clone)]
enum Ex0 {
    Unknown,
    Known(St, Vec<Ty>),
    Never,
}

impl Pe {
    fn nm_id(&self, nm: &Nm, b: Option<Base>) -> String {
        let name = self.it.name(nm.0);
        if nm.1 == GL {
            name.to_string()
        } else {
            let d = nm.1 as i64 - b.map_or(0, |b| b.d as i64);
            format!("{}@{}", name, d)
        }
    }
    fn incl(nm: &Nm, b: Option<Base>) -> bool {
        match b {
            None => true,
            Some(b) => nm.1 == GL || nm.1 >= b.d,
        }
    }
    fn slot_walk(&self, sl: &mut Slot, id: &str, f: &mut dyn FnMut(String, &mut SV)) {
        if let Some(a) = sl.mem.as_mut() {
            let mut sv = SV::A(a.clone());
            f(format!("{}.m", id), &mut sv);
            if let SV::A(x) = sv {
                *a = x;
            }
        }
        if let Some(a) = sl.num.as_mut() {
            let mut sv = SV::A(a.clone());
            f(format!("{}.n", id), &mut sv);
            if let SV::A(x) = sv {
                *a = x;
            }
        }
        if let Some(a) = sl.list.as_mut() {
            let mut sv = SV::L(a.clone());
            f(format!("{}.l", id), &mut sv);
            if let SV::L(x) = sv {
                *a = x;
            }
        }
    }
    /// Every value of a state, in one order.
    fn walk(&self, st: &mut St, b: Option<Base>, f: &mut dyn FnMut(String, &mut SV)) {
        {
            let mut sv = SV::L(line_al(st.low, &st.args));
            f("a".to_string(), &mut sv);
            if let SV::L(x) = sv {
                let (l, a) = al_line(x);
                st.low = l;
                st.args = a;
            }
        }
        {
            let mut sv = SV::B(st.ow);
            f("o".to_string(), &mut sv);
            if let SV::B(x) = sv {
                st.ow = x;
            }
        }
        for (tag, m) in [("m", &mut st.mem), ("n", &mut st.nums)] {
            let keys: Vec<Nm> = m.keys().filter(|k| Pe::incl(k, b)).cloned().collect();
            if keys.is_empty() {
                continue;
            }
            let mm = Rc::make_mut(m);
            for k in keys {
                let a = mm.get_mut(&k).unwrap();
                let mut sv = SV::A(a.clone());
                f(format!("{}{}", tag, self.nm_id(&k, b)), &mut sv);
                if let SV::A(x) = sv {
                    *a = x;
                }
            }
        }
        {
            let keys: Vec<Nm> = st.lists.keys().filter(|k| Pe::incl(k, b)).cloned().collect();
            if !keys.is_empty() {
                let mm = Rc::make_mut(&mut st.lists);
                for k in keys {
                    let a = mm.get_mut(&k).unwrap();
                    let mut sv = SV::L(a.clone());
                    f(format!("l{}", self.nm_id(&k, b)), &mut sv);
                    if let SV::L(x) = sv {
                        *a = x;
                    }
                }
            }
        }
        for (i, fr) in st.frames.iter_mut().enumerate() {
            let mut sv = SV::L(line_al(fr.0, &fr.1));
            f(format!("f{}", i), &mut sv);
            if let SV::L(x) = sv {
                *fr = al_line(x);
            }
        }
        let r0 = b.map_or(0, |b| b.r);
        for (i, (x, n)) in st.reps.iter_mut().enumerate().skip(r0) {
            for (t, a) in [("i", x), ("n", n)] {
                let mut sv = SV::A(a.clone());
                f(format!("r{}{}", i - r0, t), &mut sv);
                if let SV::A(y) = sv {
                    *a = y;
                }
            }
        }
        if let Some(j) = st.joined.as_mut() {
            let rows = Rc::make_mut(j);
            for (i, r) in rows.iter_mut().enumerate() {
                for (k, a) in r.line.iter_mut().enumerate() {
                    let mut sv = SV::A(a.clone());
                    f(format!("j{}.{}", i, k), &mut sv);
                    if let SV::A(x) = sv {
                        *a = x;
                    }
                }
                for (nm, sl) in r.w.names.iter_mut() {
                    let id = format!("jw{}.{}", i, self.nm_id(nm, b));
                    self.slot_walk(sl, &id, f);
                }
            }
        }
        if let Some(j) = st.jdyn.as_mut() {
            let j = Rc::make_mut(j);
            let mut sv = SV::A(j.n.clone());
            f("jn".to_string(), &mut sv);
            if let SV::A(x) = sv {
                j.n = x;
            }
            for (i, l) in j.lines.iter_mut().enumerate() {
                let mut sv = SV::L(AL::D(*l));
                f(format!("jl{}", i), &mut sv);
                if let SV::L(AL::D(x)) = sv {
                    *l = x;
                }
            }
            for (i, (_, _, l)) in j.writes.iter_mut().enumerate() {
                let mut sv = SV::L(AL::D(*l));
                f(format!("jx{}", i), &mut sv);
                if let SV::L(AL::D(x)) = sv {
                    *l = x;
                }
            }
        }
        if let Some(v) = st.view.as_mut() {
            let v = Rc::make_mut(v);
            for (nm, sl) in v.iter_mut() {
                let id = format!("v{}", self.nm_id(nm, b));
                self.slot_walk(sl, &id, f);
            }
        }
        if let Some(l) = st.log.as_mut() {
            let l = Rc::make_mut(l);
            for (nm, sl) in l.old.iter_mut() {
                let id = format!("g{}", self.nm_id(nm, b));
                self.slot_walk(sl, &id, f);
            }
        }
    }
    /// What two states must share exactly to be joined.
    fn fixed(&self, st: &St, b: Option<Base>) -> String {
        let mut s = String::new();
        let script = |s: &mut String, x: &Option<(Rc<Code>, u64)>| {
            if let Some((c, g)) = x {
                let _ = write!(s, "{:x}:{}", Rc::as_ptr(c) as usize, g);
            }
        };
        for (k, (c, g)) in st.scripts.iter().filter(|(k, _)| Pe::incl(k, b)) {
            let _ = write!(s, "s{}={:x}:{};", self.nm_id(k, b), Rc::as_ptr(c) as usize, g);
        }
        for (k, ms) in st.repl.iter() {
            let _ = write!(s, "R{}:", k);
            for m in ms.iter() {
                let _ = write!(s, "{},{},{:x},{:?};", m.gen, m.val.text(), m.code.as_ref().map_or(0, |c| Rc::as_ptr(c) as usize), m.nat);
            }
        }
        for (k, f) in st.maybe.iter().filter(|(k, _)| Pe::incl(k, b)) {
            let _ = write!(s, "?{}{}", self.nm_id(k, b), f);
        }
        let _ = write!(s, "|{}|", st.repl_num);
        let _ = write!(s, "F{}", st.frames.len());
        let _ = write!(s, "|r{}", st.reps.len() - b.map_or(0, |b| b.r).min(st.reps.len()));
        if let Some(j) = &st.joined {
            let _ = write!(s, "|J{}", j.len());
            for r in j.iter() {
                let _ = write!(s, ",{}", r.line.len());
                for (nm, sl) in &r.w.names {
                    let _ = write!(s, "<{}:{}{}{}", self.nm_id(nm, b), sl.mem.is_some() as u8, sl.num.is_some() as u8, sl.list.is_some() as u8);
                    script(&mut s, &sl.script);
                }
                for (k, v) in &r.w.repl {
                    let _ = write!(s, "<R{}:{:x}", k, v.as_ref().map_or(0, |v| Rc::as_ptr(v) as usize));
                }
            }
        }
        if let Some(j) = &st.jdyn {
            let _ = write!(s, "|JD{}", j.lines.len());
            for (nm, m, _) in &j.writes {
                let _ = write!(s, "<{}:{}", self.nm_id(nm, b), m);
            }
        }
        if let Some(v) = &st.view {
            s.push_str("|V");
            for (nm, sl) in v.iter() {
                let _ = write!(s, "<{}:{}{}{}", self.nm_id(nm, b), sl.mem.is_some() as u8, sl.num.is_some() as u8, sl.list.is_some() as u8);
                script(&mut s, &sl.script);
            }
        }
        if let Some(l) = &st.log {
            s.push_str("|G");
            for nm in &l.seen {
                let _ = write!(s, "{},", self.nm_id(nm, b));
            }
            for (nm, sl) in &l.old {
                let _ = write!(s, "<{}:{}{}{}", self.nm_id(nm, b), sl.mem.is_some() as u8, sl.num.is_some() as u8, sl.list.is_some() as u8);
                script(&mut s, &sl.script);
            }
            for k in &l.repl_seen {
                let _ = write!(s, "<R{}", k);
            }
            for (k, v) in &l.repl_old {
                let _ = write!(s, "<O{}:{:x}", k, v.as_ref().map_or(0, |v| Rc::as_ptr(v) as usize));
            }
        }
        let _ = write!(s, "|L{:?}{}", st.lane_at, if st.gone { "g" } else { "" });
        if !st.workers.is_empty() {
            let _ = write!(s, "|W{:x}", Rc::as_ptr(&st.workers) as usize);
        }
        s
    }
    fn flat(&self, st: &St, b: Option<Base>) -> (Vec<String>, Vec<SV>) {
        let mut c = st.clone();
        let mut ids = Vec::new();
        let mut vals = Vec::new();
        self.walk(&mut c, b, &mut |id, sv| {
            ids.push(id);
            vals.push(sv.clone());
        });
        (ids, vals)
    }

    fn meet_a(&mut self, xs: &[A], mode: Mode, outs: &mut Vec<(u32, Ty)>, vals: &mut [Vec<Ex>]) -> A {
        if let A::K(x0) = &xs[0] {
            if xs.iter().all(|x| matches!(x, A::K(y) if same_val(x0, y))) {
                return xs[0].clone();
            }
        }
        if mode == Mode::Merge {
            if let A::D(d0) = &xs[0] {
                if xs.iter().all(|x| matches!(x, A::D(d) if d.v == d0.v)) {
                    return xs[0].clone();
                }
            }
        }
        let k = xs.iter().map(kind_of).reduce(Kind::join).unwrap();
        let v = self.var(k.ty());
        outs.push((v, k.ty()));
        for (i, x) in xs.iter().enumerate() {
            let e = self.ex_as(x, k);
            vals[i].push(e);
        }
        let mut dom: Option<Vec<Val>> = Some(Vec::new());
        for x in xs {
            dom = match (dom, self.dom_of(x)) {
                (Some(mut a), Some(b)) => {
                    a.extend(b);
                    Some(a)
                }
                _ => None,
            };
        }
        let m = match dom {
            Some(d) => self.mk_dom(d),
            None => 0,
        };
        A::D(D { v, k, m })
    }
    fn meet(&mut self, xs: &[SV], mode: Mode, outs: &mut Vec<(u32, Ty)>, vals: &mut [Vec<Ex>]) -> SV {
        match &xs[0] {
            SV::A(_) => {
                let v: Vec<A> = xs.iter().map(|x| if let SV::A(a) = x { a.clone() } else { unreachable!() }).collect();
                SV::A(self.meet_a(&v, mode, outs, vals))
            }
            SV::B(_) => {
                let v: Vec<AB> = xs.iter().map(|x| if let SV::B(a) = x { *a } else { unreachable!() }).collect();
                if let AB::K(b0) = v[0] {
                    if v.iter().all(|x| *x == AB::K(b0)) {
                        return SV::B(v[0]);
                    }
                }
                if mode == Mode::Merge {
                    if let AB::D(d0) = v[0] {
                        if v.iter().all(|x| *x == AB::D(d0)) {
                            return SV::B(v[0]);
                        }
                    }
                }
                let n = self.var(Ty::B);
                outs.push((n, Ty::B));
                for (i, x) in v.iter().enumerate() {
                    vals[i].push(self.ex_b(*x));
                }
                SV::B(AB::D(n))
            }
            SV::L(_) => {
                let v: Vec<AL> = xs.iter().map(|x| if let SV::L(a) = x { a.clone() } else { unreachable!() }).collect();
                if let AL::K(l0) = &v[0] {
                    if v.iter().all(|x| matches!(x, AL::K(l) if l.len() == l0.len())) {
                        let mut items = Vec::new();
                        for j in 0..l0.len() {
                            let col: Vec<A> = v.iter().map(|x| if let AL::K(l) = x { l[j].clone() } else { unreachable!() }).collect();
                            items.push(self.meet_a(&col, mode, outs, vals));
                        }
                        return SV::L(AL::K(Rc::new(items)));
                    }
                }
                if mode == Mode::Merge {
                    if let AL::D(d0) = v[0] {
                        if v.iter().all(|x| matches!(x, AL::D(d) if *d == d0)) {
                            return SV::L(v[0].clone());
                        }
                    }
                }
                let n = self.var(Ty::L);
                outs.push((n, Ty::L));
                for (i, x) in v.iter().enumerate() {
                    vals[i].push(self.ex_list(x));
                }
                SV::L(AL::D(n))
            }
        }
    }

    /// States joined into one: what they share stays known, the rest is held in new
    /// variables, and each state gives its values for them.
    fn zip(&mut self, sides: &[&St], bases: Option<&[Base]>, mode: Mode) -> Result<Z, String> {
        let b0 = bases.map(|b| b[0]);
        // a name with no text on one path reads as 404 there, when it holds no number either
        let mut filled: Vec<St> = Vec::new();
        if sides.len() > 1 {
            let mut all: Vec<(Sym, i64)> = Vec::new();
            let mut lall: Vec<(Sym, i64)> = Vec::new();
            let mut nall: Vec<(Sym, i64)> = Vec::new();
            let mut mflag: Vec<((Sym, i64), u8)> = Vec::new();
            for (i, s) in sides.iter().enumerate() {
                let b = bases.map(|b| b[i]);
                for k in s.mem.keys().filter(|k| Pe::incl(k, b)) {
                    let rel = if k.1 == GL { i64::MAX } else { k.1 as i64 - b.map_or(0, |b| b.d as i64) };
                    if !all.contains(&(k.0, rel)) {
                        all.push((k.0, rel));
                    }
                }
                for k in s.lists.keys().filter(|k| Pe::incl(k, b)) {
                    let rel = if k.1 == GL { i64::MAX } else { k.1 as i64 - b.map_or(0, |b| b.d as i64) };
                    if !lall.contains(&(k.0, rel)) {
                        lall.push((k.0, rel));
                    }
                }
                for k in s.nums.keys().filter(|k| Pe::incl(k, b)) {
                    let rel = if k.1 == GL { i64::MAX } else { k.1 as i64 - b.map_or(0, |b| b.d as i64) };
                    if !nall.contains(&(k.0, rel)) {
                        nall.push((k.0, rel));
                    }
                }
                for (k, f) in s.maybe.iter().filter(|(k, _)| Pe::incl(k, b)) {
                    let rel = if k.1 == GL { i64::MAX } else { k.1 as i64 - b.map_or(0, |b| b.d as i64) };
                    if !mflag.contains(&((k.0, rel), *f)) {
                        mflag.push(((k.0, rel), *f));
                    }
                }
            }
            for (i, s) in sides.iter().enumerate() {
                let b = bases.map(|b| b[i]);
                let mut c = (*s).clone();
                for (k, rel) in &all {
                    let nm = if *rel == i64::MAX { (*k, GL) } else { (*k, (*rel + b.map_or(0, |b| b.d as i64)) as u32) };
                    if !c.mem.contains_key(&nm) {
                        Rc::make_mut(&mut c.mem).insert(nm, A::K(Val::L(ABSENT)));
                        mflag.push(((*k, *rel), 0));
                    }
                }
                for (k, rel) in &nall {
                    let nm = if *rel == i64::MAX { (*k, GL) } else { (*k, (*rel + b.map_or(0, |b| b.d as i64)) as u32) };
                    if !c.nums.contains_key(&nm) {
                        Rc::make_mut(&mut c.nums).insert(nm, A::K(Val::L(ABSENT)));
                        mflag.push(((*k, *rel), 1));
                    }
                }
                for ((k, rel), f) in &mflag {
                    let nm = if *rel == i64::MAX { (*k, GL) } else { (*k, (*rel + b.map_or(0, |b| b.d as i64)) as u32) };
                    Rc::make_mut(&mut c.maybe).insert((nm, *f));
                }
                for (k, rel) in &lall {
                    let nm = if *rel == i64::MAX { (*k, GL) } else { (*k, (*rel + b.map_or(0, |b| b.d as i64)) as u32) };
                    if !c.lists.contains_key(&nm) {
                        Rc::make_mut(&mut c.lists).insert(nm, AL::K(Rc::new(Vec::new())));
                    }
                }
                filled.push(c);
            }
        }
        if filled.len() > 1 {
            // every path carries the flags of all: a flag says only that a path may lack it
            let mut flags: Vec<((Sym, i64), u8)> = Vec::new();
            for (i, c) in filled.iter().enumerate() {
                let b = bases.map(|b| b[i]);
                for (k, f) in c.maybe.iter().filter(|(k, _)| Pe::incl(k, b)) {
                    let rel = if k.1 == GL { i64::MAX } else { k.1 as i64 - b.map_or(0, |b| b.d as i64) };
                    if !flags.contains(&((k.0, rel), *f)) {
                        flags.push(((k.0, rel), *f));
                    }
                }
            }
            for (i, c) in filled.iter_mut().enumerate() {
                let b = bases.map(|b| b[i]);
                for ((k, rel), f) in &flags {
                    let nm = if *rel == i64::MAX { (*k, GL) } else { (*k, (*rel + b.map_or(0, |b| b.d as i64)) as u32) };
                    Rc::make_mut(&mut c.maybe).insert((nm, *f));
                }
            }
        }
        let filled_refs: Vec<&St> = filled.iter().collect();
        let sides: &[&St] = if filled.is_empty() { sides } else { &filled_refs };
        let f0 = self.fixed(sides[0], b0);
        let mut flats = Vec::new();
        for (i, s) in sides.iter().enumerate() {
            let b = bases.map(|b| b[i]);
            if i > 0 && self.fixed(s, b) != f0 {
                return Err(self.differ(sides[0], b0, s, b));
            }
            flats.push(self.flat(s, b));
        }
        for f in &flats[1..] {
            if f.0 != flats[0].0 {
                let a: Vec<&String> = flats[0].0.iter().filter(|x| !f.0.contains(x)).collect();
                let b: Vec<&String> = f.0.iter().filter(|x| !flats[0].0.contains(x)).collect();
                return Err(format!("names set on one path and not the other: {:?} {:?}", a, b));
            }
        }
        let mut outs = Vec::new();
        let mut vals = vec![Vec::new(); sides.len()];
        let mut merged = Vec::new();
        for j in 0..flats[0].1.len() {
            let col: Vec<SV> = flats.iter().map(|f| f.1[j].clone()).collect();
            merged.push(self.meet(&col, mode, &mut outs, &mut vals));
        }
        let mut st = sides[0].clone();
        let mut it = merged.into_iter();
        self.walk(&mut st, b0, &mut |_, sv| {
            *sv = it.next().unwrap();
        });
        st.gen = sides.iter().map(|s| s.gen).max().unwrap();
        Ok(Z { st, outs, vals })
    }
    fn differ(&self, a: &St, ab: Option<Base>, b: &St, bb: Option<Base>) -> String {
        let sa: Vec<String> = a.scripts.iter().filter(|(k, _)| Pe::incl(k, ab)).map(|(k, _)| self.nm_id(k, ab)).collect();
        let sb: Vec<String> = b.scripts.iter().filter(|(k, _)| Pe::incl(k, bb)).map(|(k, _)| self.nm_id(k, bb)).collect();
        if sa != sb {
            return format!("scripts differ: {:?} and {:?}", sa, sb);
        }
        if !Rc::ptr_eq(&a.repl, &b.repl) {
            return "the replacements differ".to_string();
        }
        if a.frames.len() != b.frames.len() {
            return "the `(` frames differ".to_string();
        }
        if std::env::var("S3_BEND_DEBUG").is_ok() {
            return format!("the scripts, replacements or lanes differ:\n{}\n{}", self.fixed(a, ab), self.fixed(b, bb));
        }
        "the scripts, replacements or lanes differ".to_string()
    }

    fn base_of(&self, st: &St, key: &Key) -> Base {
        match key {
            Key::Code(_) => Base { d: st.depth(), r: st.reps.len() },
            Key::Rep(_) => Base { d: st.depth(), r: st.reps.len().saturating_sub(1) },
        }
    }
    fn hb_key(body: &HB) -> Key {
        match body {
            HB::Code(c, _) => Key::Code(Rc::as_ptr(c) as usize),
            HB::Rep(k, _) => Key::Rep(*k),
        }
    }

    /// st with the names g holds that st lacks, as they read: text 404, an empty list.
    fn fill_like(&self, st: &St, sb: Base, g: &St, gb: Base) -> St {
        let mut c = st.clone();
        if g.gone && !c.gone {
            c.joined = None;
            c.jdyn = None;
            c.gone = true;
        }
        let mv = |k: &Nm| -> Nm { if k.1 == GL { *k } else { (k.0, k.1 - gb.d + sb.d) } };
        for k in g.mem.keys().filter(|k| Pe::incl(k, Some(gb))) {
            let nm = mv(k);
            if !c.mem.contains_key(&nm) {
                Rc::make_mut(&mut c.mem).insert(nm, A::K(Val::L(ABSENT)));
            }
        }
        for k in g.nums.keys().filter(|k| Pe::incl(k, Some(gb))) {
            let nm = mv(k);
            if !c.nums.contains_key(&nm) {
                Rc::make_mut(&mut c.nums).insert(nm, A::K(Val::L(ABSENT)));
            }
        }
        for (k, f) in g.maybe.iter().filter(|(k, _)| Pe::incl(k, Some(gb))) {
            Rc::make_mut(&mut c.maybe).insert((mv(k), *f));
        }
        for k in g.lists.keys().filter(|k| Pe::incl(k, Some(gb))) {
            let nm = mv(k);
            if !c.lists.contains_key(&nm) {
                Rc::make_mut(&mut c.lists).insert(nm, AL::K(Rc::new(Vec::new())));
            }
        }
        c
    }
    /// cur fits g: g's known values are cur's, and cur's unknowns are of g's kinds.
    fn fits(&self, st: &St, sb: Base, g: &St, gb: Base) -> bool {
        let st = &self.fill_like(st, sb, g, gb);
        if self.fixed(st, Some(sb)) != self.fixed(g, Some(gb)) {
            return false;
        }
        let (ia, va) = self.flat(st, Some(sb));
        let (ib, vb) = self.flat(g, Some(gb));
        if ia != ib {
            return false;
        }
        let fit_a = |a: &A, g: &A| -> bool {
            match (a, g) {
                (A::K(x), A::K(y)) => same_val(x, y),
                (_, A::D(d)) => {
                    Kind::within(kind_of(a), d.k)
                        && (d.m == 0
                            || match self.dom_of(a) {
                                Some(xs) => xs.iter().all(|x| self.doms[d.m as usize].iter().any(|y| same_val(x, y))),
                                None => false,
                            })
                }
                _ => false,
            }
        };
        va.iter().zip(vb.iter()).all(|(a, g)| match (a, g) {
            (SV::A(a), SV::A(g)) => fit_a(a, g),
            (SV::B(a), SV::B(g)) => matches!(g, AB::D(_)) || a == g,
            (SV::L(_), SV::L(AL::D(_))) => true,
            (SV::L(AL::K(a)), SV::L(AL::K(g))) => a.len() == g.len() && a.iter().zip(g.iter()).all(|(x, y)| fit_a(x, y)),
            (SV::L(AL::C(v, a)), SV::L(AL::C(w, g))) => v == w && a.len() == g.len() && a.iter().zip(g.iter()).all(|(x, y)| fit_a(x, y)),
            _ => false,
        })
    }
    /// The values cur gives for g's unknowns.
    fn args_for(&self, st: &St, sb: Base, g: &St, gb: Base) -> Vec<Ex> {
        let st = &self.fill_like(st, sb, g, gb);
        let (_, va) = self.flat(st, Some(sb));
        let (_, vb) = self.flat(g, Some(gb));
        let mut out = Vec::new();
        for (a, g) in va.iter().zip(vb.iter()) {
            match (a, g) {
                (SV::A(a), SV::A(A::D(d))) => out.push(self.ex_as(a, d.k)),
                (SV::B(a), SV::B(AB::D(_))) => out.push(self.ex_b(*a)),
                (SV::L(a), SV::L(AL::D(_))) => out.push(self.ex_list(a)),
                (SV::L(AL::K(a)), SV::L(AL::K(g))) | (SV::L(AL::C(_, a)), SV::L(AL::C(_, g))) => {
                    for (x, y) in a.iter().zip(g.iter()) {
                        if let A::D(d) = y {
                            out.push(self.ex_as(x, d.k));
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }
    fn dyn_vars(&self, st: &St, b: Base) -> Vec<(u32, Ty)> {
        let (_, vs) = self.flat(st, Some(b));
        let mut out = Vec::new();
        for v in &vs {
            match v {
                SV::A(A::D(d)) => out.push((d.v, d.k.ty())),
                SV::B(AB::D(x)) => out.push((*x, Ty::B)),
                SV::L(AL::D(x)) => out.push((*x, Ty::L)),
                SV::L(AL::K(xs)) => {
                    for x in xs.iter() {
                        if let A::D(d) = x {
                            out.push((d.v, d.k.ty()));
                        }
                    }
                }
                SV::L(AL::C(v, xs)) => {
                    out.push((*v, Ty::L));
                    for x in xs.iter() {
                        if let A::D(d) = x {
                            out.push((d.v, d.k.ty()));
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }
    fn sig(&self, st: &St, b: Base) -> String {
        let mut s = self.fixed(st, Some(b));
        let (ids, vs) = self.flat(st, Some(b));
        let a = |s: &mut String, a: &A| match a {
            A::K(Val::N(x)) => {
                let _ = write!(s, "N{:x}", x.to_bits());
            }
            A::K(v) => {
                let _ = write!(s, "T{:?}", v.text());
            }
            A::D(d) => {
                let _ = write!(s, "D{:?}", d.k);
                if d.m > 0 {
                    for v in self.doms[d.m as usize].iter() {
                        let _ = write!(s, "|{:?}", v.text());
                    }
                }
            }
        };
        for (id, v) in ids.iter().zip(vs.iter()) {
            let _ = write!(s, ";{}=", id);
            match v {
                SV::A(x) => a(&mut s, x),
                SV::B(AB::K(x)) => {
                    let _ = write!(s, "B{}", x);
                }
                SV::B(AB::D(_)) => s.push_str("BD"),
                SV::L(AL::D(_)) => s.push_str("LD"),
                SV::L(AL::C(_, xs)) => {
                    s.push_str("LC[");
                    for x in xs.iter() {
                        a(&mut s, x);
                        s.push(',');
                    }
                    s.push(']');
                }
                SV::L(AL::K(xs)) => {
                    s.push('[');
                    for x in xs.iter() {
                        a(&mut s, x);
                        s.push(',');
                    }
                    s.push(']');
                }
            }
        }
        s
    }

    /// The general state over two entries of one head, as a function takes it.
    fn generalize(&mut self, saved: &St, sb: Base, cur: &St, cb: Base) -> R<St> {
        if std::env::var("S3_BEND_DEBUG").is_ok() && (saved.args.len() != cur.args.len() || saved.low != cur.low) {
            eprintln!("generalize: line {:?}+{} against {:?}+{}", saved.low, saved.args.len(), cur.low, cur.args.len());
        }
        // a fork in a loop holds lanes from the second round on: the loop lets them go, and
        // the build stops where a round reads them before forking again
        let held = |s: &St| s.joined.is_some() || s.jdyn.is_some();
        let (mut a, mut b) = (saved.clone(), cur.clone());
        if held(&a) != held(&b) || a.gone != b.gone {
            for s in [&mut a, &mut b] {
                s.joined = None;
                s.jdyn = None;
                s.gone = true;
            }
        }
        let (saved, cur) = (&a, &b);
        let z = match self.zip(&[saved, cur], Some(&[sb, cb]), Mode::Gen) {
            Ok(z) => z,
            Err(m) => return self.no(&format!("a loop whose rounds differ: {}", m)),
        };
        let mut g = z.st;
        self.strip(&mut g, sb);
        Ok(g)
    }
    /// A state without what lies below base b: the outer frames' `:` names and repeats.
    fn strip(&self, g: &mut St, b: Base) {
        fn keep<T: Clone>(m: &mut Rc<BTreeMap<Nm, T>>, d: u32) {
            if m.keys().any(|k| k.1 != GL && k.1 < d) {
                Rc::make_mut(m).retain(|k, _| k.1 == GL || k.1 >= d);
            }
        }
        keep(&mut g.mem, b.d);
        keep(&mut g.nums, b.d);
        keep(&mut g.lists, b.d);
        keep(&mut g.scripts, b.d);
        if g.maybe.iter().any(|(k, _)| k.1 != GL && k.1 < b.d) {
            Rc::make_mut(&mut g.maybe).retain(|(k, _)| k.1 == GL || k.1 >= b.d);
        }
        g.reps.drain(..b.r.min(g.reps.len()));
        g.code = Buf::default();
        g.end = None;
    }
    /// A function's end as the caller sees it: the outer frames' names are the caller's, and
    /// `:` names move to the caller's depth.
    fn rebase(&self, inst: St, ib: Base, caller: &St, cb: Base) -> St {
        let mut r = inst;
        let shift = |nm: Nm| -> Nm {
            if nm.1 == GL || ib.d == cb.d {
                nm
            } else {
                (nm.0, nm.1 - ib.d + cb.d)
            }
        };
        fn mv<T: Clone>(m: &BTreeMap<Nm, T>, outer: &BTreeMap<Nm, T>, cd: u32, shift: &dyn Fn(Nm) -> Nm) -> Rc<BTreeMap<Nm, T>> {
            let mut out: BTreeMap<Nm, T> = outer.iter().filter(|(k, _)| k.1 != GL && k.1 < cd).map(|(k, v)| (*k, v.clone())).collect();
            for (k, v) in m.iter() {
                out.insert(shift(*k), v.clone());
            }
            Rc::new(out)
        }
        r.mem = mv(&r.mem, &caller.mem, cb.d, &shift);
        r.nums = mv(&r.nums, &caller.nums, cb.d, &shift);
        r.lists = mv(&r.lists, &caller.lists, cb.d, &shift);
        r.scripts = mv(&r.scripts, &caller.scripts, cb.d, &shift);
        let mut fl: std::collections::BTreeSet<(Nm, u8)> = caller.maybe.iter().filter(|(k, _)| k.1 != GL && k.1 < cb.d).cloned().collect();
        for (k, f) in r.maybe.iter() {
            fl.insert((shift(*k), *f));
        }
        r.maybe = Rc::new(fl);
        let mut reps: Vec<(A, A)> = caller.reps[..cb.r.min(caller.reps.len())].to_vec();
        reps.extend(r.reps.drain(..));
        r.reps = reps;
        if ib.d != cb.d {
            if let Some(l) = r.log.as_mut() {
                let l = Rc::make_mut(l);
                for nm in l.seen.iter_mut() {
                    *nm = shift(*nm);
                }
                for (nm, _) in l.old.iter_mut() {
                    *nm = shift(*nm);
                }
            }
        }
        r.code = caller.code.clone();
        r.fs = caller.fs;
        r.end = None;
        r
    }
    fn instantiate(&mut self, e: &St, eb: Base, outs: &[(u32, Ty)]) -> St {
        let mut s = e.clone();
        let mut it = outs.iter();
        self.walk(&mut s, Some(eb), &mut |_, sv| match sv {
            SV::A(A::D(d)) => d.v = it.next().unwrap().0,
            SV::B(AB::D(v)) => *v = it.next().unwrap().0,
            SV::L(AL::D(v)) => *v = it.next().unwrap().0,
            SV::L(AL::C(v, xs)) => {
                *v = it.next().unwrap().0;
                let xs = Rc::make_mut(xs);
                for x in xs.iter_mut() {
                    if let A::D(d) = x {
                        d.v = it.next().unwrap().0;
                    }
                }
            }
            SV::L(AL::K(xs)) => {
                let xs = Rc::make_mut(xs);
                for x in xs.iter_mut() {
                    if let A::D(d) = x {
                        d.v = it.next().unwrap().0;
                    }
                }
            }
            _ => {}
        });
        s
    }

    // ---------- functions ----------

    fn residual(&mut self, saved: St, body: HB, g: St, sb: Base) -> R<St> {
        if std::env::var("S3_BEND_DEBUG").is_ok() {
            eprintln!("residual at {}", self.here());
        }
        let gb = Base { d: sb.d, r: 0 };
        let key = format!("{:?}|{}", Pe::hb_key(&body), self.sig(&g, gb));
        let fid = match self.memo.get(&key) {
            Some(&f) => f,
            None => {
                let fid = self.funs.len();
                self.funs.push(FunInfo { key: key.clone(), g: g.clone(), exit: Ex0::Unknown, fun: None, gb, body: body.clone(), used: false, provisional: false });
                self.memo.insert(key.clone(), fid);
                match self.specialize(fid) {
                    Ok(None) => {}
                    Ok(Some(g2)) => {
                        self.memo.remove(&key);
                        return self.residual(saved, body, g2, sb);
                    }
                    Err(e) => {
                        self.memo.remove(&key);
                        return Err(e);
                    }
                }
                if self.funs[fid].provisional {
                    self.memo.remove(&key);
                }
                fid
            }
        };
        self.call_fun(saved, fid)
    }

    fn specialize(&mut self, fid: usize) -> R<Option<St>> {
        for _round in 0..32 {
            if std::env::var("S3_BEND_DEBUG").is_ok() {
                eprintln!("specialize f{} round {} {}", fid, _round, self.funs[fid].key);
            }
            let g = self.funs[fid].g.clone();
            let gb = self.funs[fid].gb;
            let body = self.funs[fid].body.clone();
            let assumed = self.funs[fid].exit.clone();
            self.funs[fid].used = false;
            let mut st = g.clone();
            st.code = Buf::default();
            st.end = None;
            let hid = self.next_head;
            self.next_head += 1;
            let hpos = self.heads.len();
            self.heads.push(Head { id: hid, key: Pe::hb_key(&body), kind: HK::Fun(fid), saved: None, body: body.clone(), dyn0: 0, steps0: 0, b: gb, peel: 0, rounds: 0, nf: g.frames.len() });
            self.fun_stack.push(fid);
            let r = match &body {
                HB::Code(code, bound) => self.chain(st, code.clone(), 0, *bound, true),
                HB::Rep(k, amp) => self.rep_loop(st, *k, *amp, true),
            };
            self.fun_stack.pop();
            self.heads.truncate(hpos);
            let (blk, exit) = match r {
                Ok(s) => {
                    let z = match self.zip(&[&s], Some(&[gb]), Mode::Exit) {
                        Ok(z) => z,
                        Err(m) => return self.no(&m),
                    };
                    let blk = Block { stmts: s.code.to_vec(), end: End::Yield(z.vals[0].clone()) };
                    let tys = z.outs.iter().map(|o| o.1).collect();
                    (blk, Ex0::Known(z.st, tys))
                }
                Err(Stop::Dead(s)) => (Block { stmts: s.code.to_vec(), end: s.end.clone().unwrap() }, Ex0::Never),
                Err(Stop::Restart(h, g2)) if h == hid => return Ok(Some(*g2)),
                Err(e) => return Err(e),
            };
            let used = self.funs[fid].used;
            let same = match (&assumed, &exit) {
                (Ex0::Known(a, _), Ex0::Known(e, _)) => self.sig(a, gb) == self.sig(e, gb),
                (Ex0::Never, Ex0::Never) => true,
                _ => false,
            };
            if !used || same {
                let params = self.dyn_vars(&g, gb);
                let outs = match &exit {
                    Ex0::Known(_, t) => t.clone(),
                    _ => Vec::new(),
                };
                let what = match &body {
                    HB::Code(c, _) => c.src.chars().take(60).collect::<String>().replace('\n', " "),
                    HB::Rep(k, _) => format!("{}. repeated", self.it.name(k.0)),
                };
                self.funs[fid].fun = Some(Fun { name: format!("f{}", fid), params, outs, body: blk, what });
                self.funs[fid].exit = exit;
                return Ok(None);
            }
            self.funs[fid].exit = match (assumed, exit) {
                (Ex0::Known(a, _), Ex0::Known(e, _)) => match self.zip(&[&a, &e], Some(&[gb, gb]), Mode::Exit) {
                    Ok(z) => {
                        let tys = z.outs.iter().map(|o| o.1).collect();
                        Ex0::Known(z.st, tys)
                    }
                    Err(m) => return self.no(&format!("a function whose ends differ: {}", m)),
                },
                (_, e) => e,
            };
        }
        self.no("a recursion whose end does not settle")
    }

    fn call_fun(&mut self, mut st: St, fid: usize) -> R<St> {
        let key = Pe::hb_key(&self.funs[fid].body);
        let cb = self.base_of(&st, &key);
        let gb = self.funs[fid].gb;
        let g = self.funs[fid].g.clone();
        let args = self.args_for(&st, cb, &g, gb);
        self.funs[fid].used = true;
        self.dyn_ev += 1;
        match self.funs[fid].exit.clone() {
            Ex0::Unknown => {
                if let Some(p) = self.fun_stack.iter().position(|&f| f == fid) {
                    for &f in &self.fun_stack[p + 1..].to_vec() {
                        self.funs[f].provisional = true;
                    }
                }
                st.end = Some(End::Never);
                Err(Stop::Dead(Box::new(st)))
            }
            Ex0::Never => {
                st.emit(Stmt::Call { f: fid, args, outs: vec![] });
                st.end = Some(End::Never);
                Err(Stop::Dead(Box::new(st)))
            }
            Ex0::Known(e, tys) => {
                let outs: Vec<(u32, Ty)> = tys.iter().map(|t| (self.var(*t), *t)).collect();
                st.emit(Stmt::Call { f: fid, args, outs: outs.clone() });
                let inst = self.instantiate(&e, gb, &outs);
                Ok(self.rebase(inst, gb, &st, cb))
            }
        }
    }

    // ---------- the program ----------

    pub fn run_main(&mut self, src: &str) -> Result<Block, String> {
        let code = match self.code_of(src) {
            Ok(c) => c,
            Err(Stop::Build(m)) => return Err(m),
            Err(_) => unreachable!(),
        };
        let mut st = self.fresh_st();
        let lv = self.var(Ty::L);
        let nv = self.var(Ty::F);
        st.emit(Stmt::Args(lv, nv));
        Rc::make_mut(&mut st.lists).insert((S_ARGS, GL), AL::D(lv));
        Rc::make_mut(&mut st.nums).insert((S_ARGC, GL), A::D(D { v: nv, k: Kind::Num, m: 0 }));
        match self.frame_code(st, code, UNBOUND) {
            Ok(s) => Ok(Block { stmts: s.code.to_vec(), end: End::Yield(Vec::new()) }),
            Err(Stop::Dead(s)) => Ok(Block { stmts: s.code.to_vec(), end: s.end.clone().unwrap() }),
            Err(Stop::Build(m)) => Err(m),
            Err(Stop::Restart(..)) => Err("internal: a restart reached the top".to_string()),
        }
    }

    /// A machine as it starts: the main one, or a worker's.
    fn fresh_st(&mut self) -> St {
        let mut repl: BTreeMap<Rc<str>, Rc<Vec<Mac>>> = BTreeMap::new();
        for (name, nat) in [("lanes", Nat::Lanes), ("lane", Nat::Lane), ("endlane", Nat::EndLane), ("collect", Nat::Collect), ("into", Nat::Into), ("reduce", Nat::Reduce), ("lanewise", Nat::Lanewise), ("try", Nat::Try)] {
            repl.insert(Rc::from(name), Rc::new(vec![Mac { val: Val::L(name), code: None, nat: Some(nat), gen: 0 }]));
        }
        St {
            args: Vec::new(),
            low: None,
            ow: AB::K(false),
            mem: Rc::new(BTreeMap::new()),
            nums: Rc::new(BTreeMap::new()),
            lists: Rc::new(BTreeMap::new()),
            scripts: Rc::new(BTreeMap::new()),
            repl: Rc::new(repl),
            repl_num: false,
            gen: 1,
            frames: Vec::new(),
            fs: 0,
            maybe: Rc::new(std::collections::BTreeSet::new()),
            joined: None,
            jdyn: None,
            view: None,
            log: None,
            lane_at: None,
            reps: Vec::new(),
            code: Buf::default(),
            end: None,
            workers: Rc::new(Vec::new()),
            gone: false,
        }
    }

    /// Code run in a worker's machine, its writes going where main's go: the worker after,
    /// and what it left in `out`.
    fn in_worker(&mut self, st: &mut St, mut w: St, src: &str) -> R<Result<(St, Option<A>), String>> {
        w.code = std::mem::take(&mut st.code);
        let fs = w.fs;
        match self.frame_src(w, src) {
            Ok(mut w) => {
                st.code = std::mem::take(&mut w.code);
                w.fs = fs;
                let out = Rc::make_mut(&mut w.mem).remove(&(S_OUT, GL));
                Ok(Ok((w, out)))
            }
            Err(Stop::Dead(s)) => match s.end.clone() {
                Some(End::Fail(Ex::Str(m))) if s.joined.is_none() => {
                    let mut s = *s;
                    st.code = std::mem::take(&mut s.code);
                    Ok(Err(m.to_string()))
                }
                Some(End::Fail(_)) => self.no("a worker's error that depends on input"),
                _ => Err(Stop::Dead(s)),
            },
            Err(e) => Err(e),
        }
    }
    fn worker_ix(&mut self, st: &St, i: usize) -> R<Option<usize>> {
        let x = match &st.args[i] {
            A::K(v) => v.num(),
            A::D(_) => return self.no("a thread index from input"),
        };
        let k = x as i64;
        Ok(if x.fract() == 0.0 && k >= 0 && (k as usize) < st.workers.len() && st.workers[k as usize].is_some() { Some(k as usize) } else { None })
    }
}
