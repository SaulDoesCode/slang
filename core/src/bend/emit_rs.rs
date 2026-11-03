// The residue written as Rust: numbers as f64, texts as Arc<str>, lists as Arc<Vec<V>> moved
// on their last use and changed in place, a function's call of itself in tail position as a
// loop, effects as calls into std. Nothing of the machine is in the program.
use super::ir::*;
use std::collections::BTreeSet;
use std::fmt::Write as _;

pub const PRELUDE: &str = include_str!("prelude.rs");

type Live = BTreeSet<u32>;

#[derive(Clone, Copy, PartialEq)]
enum M {
    /// the value itself: a copy, a move on its last use, else a clone
    Val,
    /// a borrow: &str for a text, &V, &L
    Ref,
}

pub struct EmitRs<'a> {
    pub lmaps: Vec<Vec<(String, Ex)>>,
    funs: &'a [Option<&'a Fun>],
    tys: Vec<Ty>,
    /// each function's output types, number lists marked
    outs_ty: Vec<Vec<Ty>>,
    keysets: &'a [Vec<String>],
    io: Vec<bool>,
    used: Vec<Vec<bool>>,
    /// which outputs of each function some caller reads
    need: Vec<Vec<bool>>,
    changed: bool,
    /// V parameters the body reads only as a number (F) or only as text (S): passed unboxed
    pk: std::collections::HashMap<u32, Ty>,
    /// V vars that only ever hold numbers: written as f64
    nv: BTreeSet<u32>,
    /// kernels for forks over lists of numbers
    kdefs: Vec<String>,
    /// each function's body, header to end, as written this pass
    ftexts: std::collections::HashMap<usize, (String, String, String)>,
    maps_all: Vec<usize>,
    lits: Vec<String>,
    cur: usize,
    looped: bool,
    n: usize,
}

fn rty(t: Ty) -> &'static str {
    match t {
        Ty::F => "f64",
        Ty::S => "S",
        Ty::B => "bool",
        Ty::V => "V",
        Ty::L => "L",
        Ty::N => "LN",
    }
}
fn copy(t: Ty) -> bool {
    matches!(t, Ty::F | Ty::B)
}
fn tuple_ty(ts: &[Ty]) -> String {
    match ts.len() {
        1 => rty(ts[0]).to_string(),
        _ => format!("({})", ts.iter().map(|t| rty(*t)).collect::<Vec<_>>().join(", ")),
    }
}
fn tuple(xs: &[String]) -> String {
    match xs.len() {
        1 => xs[0].clone(),
        _ => format!("({})", xs.join(", ")),
    }
}
fn lit_num(x: f64) -> String {
    if x.is_nan() {
        "f64::NAN".into()
    } else if x.is_infinite() {
        if x > 0.0 { "f64::INFINITY".into() } else { "f64::NEG_INFINITY".into() }
    } else {
        format!("({:?}f64)", x)
    }
}

fn tail_of(stmts: &[Stmt], end: &End) -> bool {
    let outs = match stmts.last() {
        Some(Stmt::Call { outs, .. } | Stmt::Branch { outs, .. }) => outs,
        _ => return false,
    };
    match end {
        End::Yield(xs) => xs.len() == outs.len() && xs.iter().zip(outs.iter()).all(|(x, o)| matches!(x, Ex::Var(v) if *v == o.0)),
        End::Never => true,
        _ => false,
    }
}

impl<'a> EmitRs<'a> {
    pub fn new(funs: &'a [Option<&'a Fun>], tys: &'a [Ty], keysets: &'a [Vec<String>]) -> EmitRs<'a> {
        EmitRs { lmaps: Vec::new(), funs, tys: tys.to_vec(), outs_ty: funs.iter().map(|f| f.map_or(Vec::new(), |f| f.outs.clone())).collect(), keysets, io: vec![false; funs.len()], used: Vec::new(), need: Vec::new(), changed: false, pk: Default::default(), nv: BTreeSet::new(), kdefs: Vec::new(), ftexts: Default::default(), maps_all: Vec::new(), lits: Vec::new(), cur: usize::MAX, looped: false, n: 0 }
    }

    fn block_io(&self, b: &Block) -> bool {
        matches!(b.end, End::Exit(_) | End::Fail(_)) || b.stmts.iter().any(|s| self.stmt_io(s))
    }
    fn stmt_io(&self, s: &Stmt) -> bool {
        match s {
            Stmt::Spawn { .. } | Stmt::Join { .. } => true,
            Stmt::Let(..) => false,
            Stmt::Write(_) | Stmt::ReadLine(..) | Stmt::Args(..) | Stmt::Fx(..) => true,
            Stmt::Call { f, .. } | Stmt::Map { f, .. } => self.io[*f],
            Stmt::Branch { t, e, .. } => self.block_io(t) || self.block_io(e),
            Stmt::Par { calls } => calls.iter().any(|c| self.io[c.0]),
        }
    }
    fn analyse(&mut self) {
        loop {
            let mut changed = false;
            for i in 0..self.funs.len() {
                if let Some(f) = self.funs[i] {
                    if !self.io[i] && self.block_io(&f.body) {
                        self.io[i] = true;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
    }
    fn kept(&self, f: usize, j: usize) -> bool {
        self.used.get(f).and_then(|u| u.get(j)).copied().unwrap_or(true)
    }

    fn ty_of(&self, e: &Ex) -> Ty {
        match e {
            Ex::Var(v) => self.vt(*v),
            Ex::Num(_) => Ty::F,
            Ex::Str(_) => Ty::S,
            Ex::Bool(_) => Ty::B,
            Ex::Strs(_) => Ty::L,
            Ex::App(p, a) => match p {
                P::Add | P::Sub | P::Mul | P::Div | P::Pow | P::Mod | P::Parse | P::VNum | P::SLen | P::LLen | P::ShRc => Ty::F,
                P::Lt | P::Gt | P::Le | P::Ge | P::NEq | P::Not | P::And | P::VEq | P::SEq | P::IsX | P::LHas | P::Runs | P::FxOk => Ty::B,
                P::Fmt | P::VText | P::Cat | P::Join | P::FxVal | P::ShOut => Ty::S,
                P::VN | P::VOf | P::VT | P::VX | P::LLast | P::LGet => Ty::V,
                P::LNew | P::LPush | P::LPop | P::LSet | P::LCat | P::LTake | P::LDrop | P::SplitWs | P::SplitChars | P::SplitBy | P::LNamed => Ty::L,
                P::If => {
                    let (x, y) = (self.ty_of(&a[1]), self.ty_of(&a[2]));
                    // a number where a value is wanted on one side: the choice is a value
                    if x == y { x } else if matches!((x, y), (Ty::F, Ty::V) | (Ty::V, Ty::F)) { Ty::V } else { x }
                }
            },
        }
    }
    /// The type a var is written with.
    fn vt(&self, v: u32) -> Ty {
        if let Some(t) = self.pk.get(&v) {
            return *t;
        }
        if self.nv.contains(&v) {
            return Ty::F;
        }
        self.tys[v as usize]
    }
    /// A V expression that only ever holds a number.
    fn numexpr(&self, e: &Ex) -> bool {
        match e {
            Ex::App(P::VN, _) => true,
            Ex::Var(v) => self.nv.contains(v) || self.pk.get(v) == Some(&Ty::F),
            Ex::App(P::If, a) => self.numexpr(&a[1]) && self.numexpr(&a[2]),
            _ => false,
        }
    }
    /// A V expression as f64: it holds only numbers.
    fn exf(&mut self, e: &Ex, live: &mut Live) -> String {
        match e {
            Ex::App(P::VN, x) => self.ex(&x[0], M::Val, live),
            _ => self.ex(e, M::Val, live),
        }
    }
    /// An expression where a V is wanted: a number held as f64 is boxed there.
    fn exv(&mut self, e: &Ex, m: M, live: &mut Live) -> String {
        if self.ty_of(e) == Ty::F {
            let x = self.ex(e, M::Val, live);
            return match m {
                M::Val => format!("V::N({})", x),
                M::Ref => format!("&V::N({})", x),
            };
        }
        self.ex(e, m, live)
    }
    /// An expression written as the type t: a list of numbers where one is or is not wanted,
    /// a number held as f64, or a V.
    fn ex_as(&mut self, e: &Ex, t: Ty, live: &mut Live) -> String {
        match t {
            Ty::N => self.ex_to(e, true, live),
            Ty::L => self.ex_to(e, false, live),
            Ty::F if self.ty_of(e) == Ty::V || matches!(e, Ex::App(P::VN, _)) => self.exf(e, live),
            Ty::V => self.exv(e, M::Val, live),
            _ => self.ex(e, M::Val, live),
        }
    }
    fn tmp(&mut self) -> String {
        self.n += 1;
        format!("t{}", self.n)
    }
    fn lit(&mut self, s: &str) -> String {
        let i = match self.lits.iter().position(|x| x == s) {
            Some(i) => i,
            None => {
                self.lits.push(s.to_string());
                self.lits.len() - 1
            }
        };
        format!("lit({})", i)
    }

    /// An expression, read backward: `live` holds what is read after it, and gains what it reads.
    fn ex(&mut self, e: &Ex, m: M, live: &mut Live) -> String {
        let t = self.ty_of(e);
        let s = match e {
            Ex::Var(v) => {
                let name = format!("v{}", v);
                let s = match m {
                    M::Val if copy(t) => name,
                    M::Val if live.contains(v) => format!("{}.clone()", name),
                    M::Val => name,
                    M::Ref if t == Ty::S => format!("&*{}", name),
                    M::Ref => format!("&{}", name),
                };
                live.insert(*v);
                return s;
            }
            Ex::Num(x) => return lit_num(*x),
            Ex::Bool(b) => return b.to_string(),
            Ex::Str(s) => {
                return match m {
                    M::Ref => format!("{:?}", &**s),
                    M::Val => self.lit(s),
                }
            }
            Ex::Strs(xs) => {
                let items: Vec<String> = xs.iter().map(|x| format!("vof(rs({:?}))", x)).collect();
                format!("Arc::new(vec![{}])", items.join(", "))
            }
            Ex::App(p, a) => self.app(*p, a, live),
        };
        match m {
            M::Val => s,
            M::Ref if t == Ty::S => format!("&*{}", s),
            M::Ref => format!("&{}", s),
        }
    }

    /// Arguments in the order the code evaluates them, read backward.
    fn args(&mut self, a: &[Ex], ms: &[M], live: &mut Live) -> Vec<String> {
        let mut out = vec![String::new(); a.len()];
        for i in (0..a.len()).rev() {
            out[i] = self.ex(&a[i], ms[i], live);
        }
        out
    }

    fn app(&mut self, p: P, a: &[Ex], live: &mut Live) -> String {
        use M::*;
        let bin = |me: &mut Self, op: &str, live: &mut Live| {
            let x = me.args(a, &[Val, Val], live);
            format!("({} {} {})", x[0], op, x[1])
        };
        let call = |me: &mut Self, f: &str, ms: &[M], live: &mut Live| {
            let x = me.args(a, ms, live);
            format!("{}({})", f, x.join(", "))
        };
        match p {
            P::Add => bin(self, "+", live),
            P::Sub => bin(self, "-", live),
            P::Mul => bin(self, "*", live),
            P::Div => bin(self, "/", live),
            P::Lt => bin(self, "<", live),
            P::Gt => bin(self, ">", live),
            P::Le => bin(self, "<=", live),
            P::Ge => bin(self, ">=", live),
            P::NEq => bin(self, "==", live),
            P::And => bin(self, "&&", live),
            P::Pow => {
                let x = self.args(a, &[Val, Val], live);
                format!("{}.powf({})", x[0], x[1])
            }
            P::Mod => {
                let x = self.args(a, &[Val, Val], live);
                format!("srem({}, {})", x[0], x[1])
            }
            P::Not => format!("(!{})", self.ex(&a[0], Val, live)),
            P::VEq => {
                let y = self.exv(&a[1], Ref, live);
                let x = self.exv(&a[0], Ref, live);
                format!("veq({}, {})", x, y)
            }
            P::SEq => call(self, "seq", &[Ref, Ref], live),
            P::Fmt => call(self, "fmt", &[Val], live),
            P::Parse => call(self, "parse", &[Ref], live),
            P::VN => call(self, "V::N", &[Val], live),
            P::VOf => call(self, "vof", &[Val], live),
            P::VT => call(self, "V::T", &[Val], live),
            P::VX => "V::X".to_string(),
            P::IsX if self.ty_of(&a[0]) == Ty::F => "false".to_string(),
            P::IsX => call(self, "isx", &[Ref], live),
            P::VText if matches!(&a[0], Ex::Var(v) if self.pk.get(v) == Some(&Ty::S)) => self.ex(&a[0], Val, live),
            P::VText if self.ty_of(&a[0]) == Ty::F => format!("fmt({})", self.ex(&a[0], Val, live)),
            P::VText => call(self, "vtext", &[Ref], live),
            P::VNum if matches!(&a[0], Ex::Var(v) if self.pk.get(v) == Some(&Ty::F)) => self.ex(&a[0], Val, live),
            P::VNum if self.ty_of(&a[0]) == Ty::F => self.ex(&a[0], Val, live),
            P::VNum => call(self, "vnum", &[Ref], live),
            P::Cat => {
                let ms = vec![Ref; a.len()];
                let x = self.args(a, &ms, live);
                format!("cat(&[{}])", x.join(", "))
            }
            P::SLen => call(self, "slen", &[Ref], live),
            P::LNew => "lnew()".to_string(),
            // a list changed: what goes in is worked out first, the list moved in last
            P::LPush if Self::const_list(&a[0]).is_some() && Self::const_v(&a[1]) => {
                let nl = self.nlexpr(&Ex::App(p, a.to_vec()));
                let mut items = Self::const_list(&a[0]).unwrap();
                items.push(a[1].clone());
                let xs: Vec<String> = items
                    .iter()
                    .map(|x| match x {
                        Ex::App(P::VN, n) if nl => self.ex(&n[0], Val, live),
                        _ => self.ex(x, Val, live),
                    })
                    .collect();
                format!("Arc::new(vec![{}])", xs.join(", "))
            }
            P::LPush | P::LSet if self.nlexpr(&Ex::App(p, a.to_vec())) => {
                let k = a.len();
                let l = self.ex(&a[0], Val, live);
                let mut rest = vec![String::new(); k];
                for i in (1..k).rev() {
                    rest[i] = if i == k - 1 { self.exf(&a[i], live) } else { self.ex(&a[i], Val, live) };
                }
                let mut o = String::from("{ ");
                let mut names = vec![l];
                for r in rest.iter().skip(1) {
                    let t = self.tmp();
                    let _ = write!(o, "let {} = {}; ", t, r);
                    names.push(t);
                }
                let _ = write!(o, "{}({}) }}", if p == P::LPush { "lpush" } else { "lset" }, names.join(", "));
                o
            }
            P::LPush | P::LSet => {
                let k = a.len();
                let l = self.ex_to(&a[0], false, live);
                let mut rest = vec![String::new(); k];
                for i in (1..k).rev() {
                    rest[i] = if i == k - 1 { self.exv(&a[i], Val, live) } else { self.ex(&a[i], Val, live) };
                }
                let mut o = String::from("{ ");
                let mut names = vec![l];
                for r in rest.iter().skip(1) {
                    let t = self.tmp();
                    let _ = write!(o, "let {} = {}; ", t, r);
                    names.push(t);
                }
                let _ = write!(o, "{}({}) }}", if p == P::LPush { "lpush" } else { "lset" }, names.join(", "));
                o
            }
            P::LPop => call(self, "lpop", &[Val], live),
            P::LTake => {
                let n = self.ex(&a[1], Val, live);
                let l = self.ex_to(&a[0], false, live);
                format!("ltake({}, {})", l, n)
            }
            P::LDrop => {
                let n = self.ex(&a[1], Val, live);
                let l = self.ex_to(&a[0], false, live);
                format!("ldrop({}, {})", l, n)
            }
            P::LCat => {
                let x = self.ex_to(&a[0], false, live);
                let y = self.ex_to(&a[1], false, live);
                format!("lcat({}, &{})", x, y)
            }
            P::LLast if self.nlexpr(&a[0]) => call(self, "llast_n", &[Ref], live),
            P::LLast => call(self, "llast", &[Ref], live),
            P::LGet if self.nlexpr(&a[0]) => call(self, "lget_n", &[Ref, Val], live),
            P::LGet => call(self, "lget", &[Ref, Val], live),
            P::LHas => call(self, "lhas", &[Ref, Val], live),
            P::LLen => call(self, "llen", &[Ref], live),
            P::SplitWs => call(self, "splitws", &[Ref], live),
            P::SplitChars => call(self, "splitchars", &[Ref], live),
            P::SplitBy => call(self, "splitby", &[Ref, Ref], live),
            P::Join if self.nlexpr(&a[0]) => call(self, "join_n", &[Ref, Ref], live),
            P::Join => call(self, "join", &[Ref, Ref], live),
            P::Runs => {
                let k = match &a[1] {
                    Ex::Num(k) => *k as usize,
                    _ => 0,
                };
                // a value read as text to see if it runs: a number runs only where a number is a key
                if let Ex::App(P::VText, v) = &a[0] {
                    return format!("runsv{}({})", k, self.ex(&v[0], Ref, live));
                }
                format!("runs{}({})", k, self.ex(&a[0], Ref, live))
            }
            P::FxOk => call(self, "fx_ok", &[Ref], live),
            P::FxVal => call(self, "fx_val", &[Ref], live),
            P::ShRc => call(self, "sh_rc", &[Ref], live),
            P::ShOut => call(self, "sh_out", &[Ref], live),
            P::If => {
                let whole = Ex::App(p, a.to_vec());
                let t = match self.ty_of(&whole) {
                    Ty::L | Ty::N => if self.nlexpr(&whole) { Ty::N } else { Ty::L },
                    t => t,
                };
                let after = live.clone();
                let mut la = after.clone();
                let x = self.ex_as(&a[1], t, &mut la);
                let mut lb = after;
                let y = self.ex_as(&a[2], t, &mut lb);
                live.extend(la);
                live.extend(lb);
                let c = self.ex(&a[0], Val, live);
                format!("(if {} {{ {} }} else {{ {} }})", c, x, y)
            }
            P::LNamed => {
                let mi = match &a[0] {
                    Ex::Num(k) => *k as usize,
                    _ => 0,
                };
                // the later pairs are looked at first, as the machine's newest replacement wins
                let after = live.clone();
                let mut arms = Vec::new();
                let mut i = 2;
                while i + 1 < a.len() {
                    let mut l = after.clone();
                    let x = self.ex_to(&a[i + 1], false, &mut l);
                    let n = self.ex(&a[i], Ref, &mut l);
                    live.extend(l);
                    arms.push((n, x));
                    i += 2;
                }
                let k = self.ex(&a[1], Val, live);
                let mut o = format!("{{ let k_: S = {}; ", k);
                for (n, x) in arms.iter().rev() {
                    let _ = write!(o, "if seq(&*k_, {}) {{ {} }} else ", n, x);
                }
                let _ = write!(o, "{{ ln{}(&*k_) }} }}", mi);
                o
            }
        }
    }

    /// Whether a list expression holds numbers only, by the vars and outputs marked so far.
    fn nlexpr(&self, e: &Ex) -> bool {
        match e {
            Ex::Var(v) => self.tys[*v as usize] == Ty::N,
            Ex::App(P::LNew, _) => true,
            Ex::App(P::LPush, a) => self.nlexpr(&a[0]) && self.numexpr(&a[1]),
            Ex::App(P::LSet, a) => self.nlexpr(&a[0]) && self.numexpr(&a[2]),
            Ex::App(P::LPop, a) => self.nlexpr(&a[0]),
            Ex::App(P::If, a) if matches!(self.ty_of(&a[1]), Ty::L | Ty::N) => self.nlexpr(&a[1]) && self.nlexpr(&a[2]),
            _ => false,
        }
    }
    /// A list expression where a list of numbers is (want) or is not wanted: a number list
    /// given where a list of values is wanted is turned into one.
    fn ex_to(&mut self, e: &Ex, want: bool, live: &mut Live) -> String {
        let x = self.ex(e, M::Val, live);
        if !want && matches!(self.ty_of(e), Ty::L | Ty::N) && self.nlexpr(e) {
            format!("to_l(&{})", x)
        } else {
            x
        }
    }
    /// Lists whose items are all numbers: every list that flows into one is built from the
    /// empty list by pushing numbers, or is another such list. The greatest such set.
    fn numlists_init(&mut self) {
        for t in self.tys.iter_mut() {
            if *t == Ty::L {
                *t = Ty::N;
            }
        }
        for o in self.outs_ty.iter_mut() {
            for t in o.iter_mut() {
                if *t == Ty::L {
                    *t = Ty::N;
                }
            }
        }
    }
    /// One round of taking out the lists something not a number flows into; true when one went.
    fn numlists(&mut self, main: &Block) -> bool {
        fn yield_nl(me: &EmitRs, b: &Block, j: usize) -> bool {
            match &b.end {
                End::Yield(xs) => xs.get(j).map_or(true, |x| me.nlexpr(x)),
                _ => true,
            }
        }
        fn walk(me: &EmitRs, b: &Block, bad: &mut Vec<u32>, badout: &mut Vec<(usize, usize)>) {
            let args = |f: usize, a: &[Ex], bad: &mut Vec<u32>| {
                let ps = &me.funs[f].unwrap().params;
                for (j, x) in a.iter().enumerate() {
                    if let Some(p) = ps.get(j) {
                        if me.tys[p.0 as usize] == Ty::N && !me.nlexpr(x) {
                            bad.push(p.0)
                        }
                    }
                }
            };
            for s in &b.stmts {
                match s {
                    Stmt::Spawn { body, .. } => walk(me, body, bad, badout),
                    Stmt::Join { outs, .. } => {
                        for o in outs {
                            bad.push(o.0)
                        }
                    }
                    Stmt::Let(v, _, e) => {
                        if me.tys[*v as usize] == Ty::N && !me.nlexpr(e) {
                            bad.push(*v)
                        }
                    }
                    Stmt::Args(a, _) => bad.push(*a),
                    Stmt::Write(_) | Stmt::ReadLine(..) | Stmt::Fx(..) => {}
                    Stmt::Call { f, args: a, outs } => {
                        args(*f, a, bad);
                        for (j, o) in outs.iter().enumerate() {
                            if me.tys[o.0 as usize] == Ty::N && me.outs_ty[*f].get(j) != Some(&Ty::N) {
                                bad.push(o.0)
                            }
                        }
                    }
                    Stmt::Par { calls } => {
                        for (f, a, outs) in calls {
                            args(*f, a, bad);
                            for (j, o) in outs.iter().enumerate() {
                                if me.tys[o.0 as usize] == Ty::N && me.outs_ty[*f].get(j) != Some(&Ty::N) {
                                    bad.push(o.0)
                                }
                            }
                        }
                    }
                    Stmt::Map { f, args: a, outs, .. } => {
                        args(*f, a, bad);
                        // a lane that gives back only numbers fills a list of numbers
                        for (j, o) in outs.iter().enumerate() {
                            if me.tys[o.0 as usize] == Ty::N && me.outs_ty[*f].get(j) != Some(&Ty::F) {
                                bad.push(o.0)
                            }
                        }
                    }
                    Stmt::Branch { t, e, outs, .. } => {
                        for (j, o) in outs.iter().enumerate() {
                            if me.tys[o.0 as usize] == Ty::N && !(yield_nl(me, t, j) && yield_nl(me, e, j)) {
                                bad.push(o.0)
                            }
                        }
                        walk(me, t, bad, badout);
                        walk(me, e, bad, badout);
                    }
                }
            }
        }
        let mut bad = Vec::new();
        let mut badout = Vec::new();
        for (i, f) in self.funs.iter().enumerate() {
            if let Some(f) = f {
                walk(self, &f.body, &mut bad, &mut badout);
                for j in 0..self.outs_ty[i].len() {
                    if self.outs_ty[i][j] == Ty::N && !yield_nl(self, &f.body, j) {
                        badout.push((i, j));
                    }
                }
            }
        }
        walk(self, main, &mut bad, &mut badout);
        let mut ch = false;
        for v in bad {
            if self.tys[v as usize] == Ty::N {
                self.tys[v as usize] = Ty::L;
                ch = true;
            }
        }
        for (i, j) in badout {
            self.outs_ty[i][j] = Ty::L;
            ch = true;
        }
        ch
    }
    fn numv_init(&mut self) {
        for (v, t) in self.tys.iter().enumerate() {
            if *t == Ty::V && !self.pk.contains_key(&(v as u32)) {
                self.nv.insert(v as u32);
            }
        }
        for (i, f) in self.funs.iter().enumerate() {
            if let Some(f) = f {
                for (j, t) in f.outs.iter().enumerate() {
                    if *t == Ty::V {
                        self.outs_ty[i][j] = Ty::F;
                    }
                }
            }
        }
    }
    /// One round of taking out the V vars something not a number flows into.
    fn numv(&mut self, main: &Block) -> bool {
        fn yield_num(me: &EmitRs, b: &Block, j: usize) -> bool {
            match &b.end {
                End::Yield(xs) => xs.get(j).map_or(true, |x| me.numexpr(x)),
                _ => true,
            }
        }
        fn walk(me: &EmitRs, b: &Block, bad: &mut Vec<u32>) {
            let params = |f: usize, a: &[Ex], upto: usize, bad: &mut Vec<u32>| {
                let ps = &me.funs[f].unwrap().params;
                for (j, x) in a.iter().enumerate().take(upto) {
                    if let Some(p) = ps.get(j) {
                        if me.nv.contains(&p.0) && !me.numexpr(x) {
                            bad.push(p.0)
                        }
                    }
                }
            };
            let outs_of = |f: usize, outs: &[(u32, Ty)], bad: &mut Vec<u32>| {
                for (j, o) in outs.iter().enumerate() {
                    if me.nv.contains(&o.0) && me.outs_ty[f].get(j) != Some(&Ty::F) {
                        bad.push(o.0)
                    }
                }
            };
            for s in &b.stmts {
                match s {
                    Stmt::Let(v, _, e) => {
                        if me.nv.contains(v) && !me.numexpr(e) {
                            bad.push(*v)
                        }
                    }
                    Stmt::ReadLine(a, _) => bad.push(*a),
                    Stmt::Join { outs, .. } => bad.extend(outs.iter().map(|o| o.0)),
                    Stmt::Write(_) | Stmt::Args(..) | Stmt::Fx(..) => {}
                    Stmt::Spawn { body, .. } => walk(me, body, bad),
                    Stmt::Call { f, args, outs } => {
                        params(*f, args, usize::MAX, bad);
                        outs_of(*f, outs, bad);
                    }
                    Stmt::Par { calls } => {
                        for (f, args, outs) in calls {
                            params(*f, args, usize::MAX, bad);
                            outs_of(*f, outs, bad);
                        }
                    }
                    Stmt::Map { f, xs, args, .. } => {
                        let ps = &me.funs[*f].unwrap().params;
                        params(*f, args, ps.len() - 1, bad);
                        // a lane's item is a number when the list holds only numbers
                        if let Some(p) = ps.last() {
                            if me.nv.contains(&p.0) && !me.nlexpr(xs) {
                                bad.push(p.0)
                            }
                        }
                    }
                    Stmt::Branch { t, e, outs, .. } => {
                        for (j, o) in outs.iter().enumerate() {
                            if me.nv.contains(&o.0) && !(yield_num(me, t, j) && yield_num(me, e, j)) {
                                bad.push(o.0)
                            }
                        }
                        walk(me, t, bad);
                        walk(me, e, bad);
                    }
                }
            }
        }
        let mut bad = Vec::new();
        let mut badout = Vec::new();
        for (i, f) in self.funs.iter().enumerate() {
            if let Some(f) = f {
                walk(self, &f.body, &mut bad);
                for j in 0..self.outs_ty[i].len() {
                    if self.outs_ty[i][j] == Ty::F && f.outs[j] == Ty::V && !yield_num(self, &f.body, j) {
                        badout.push((i, j));
                    }
                }
            }
        }
        walk(self, main, &mut bad);
        let mut ch = false;
        for v in bad {
            ch |= self.nv.remove(&v);
        }
        for (i, j) in badout {
            self.outs_ty[i][j] = Ty::V;
            ch = true;
        }
        ch
    }
    /// A V known at build time.
    fn const_v(e: &Ex) -> bool {
        match e {
            Ex::App(P::VN, a) => matches!(a[0], Ex::Num(_)),
            Ex::App(P::VT | P::VOf, a) => matches!(a[0], Ex::Str(_)),
            Ex::App(P::VX, _) => true,
            _ => false,
        }
    }
    /// The items of a list built at build time by pushes onto the empty list.
    fn const_list(e: &Ex) -> Option<Vec<Ex>> {
        match e {
            Ex::App(P::LNew, _) => Some(Vec::new()),
            Ex::App(P::LPush, a) if Self::const_v(&a[1]) => {
                let mut v = Self::const_list(&a[0])?;
                v.push(a[1].clone());
                Some(v)
            }
            _ => None,
        }
    }
    fn kill(live: &mut Live, outs: &[(u32, Ty)]) {
        for o in outs {
            live.remove(&o.0);
        }
    }
    fn bind(&self, outs: &[(u32, Ty)]) -> String {
        match outs.len() {
            0 => String::new(),
            1 => format!("let mut v{}: {} = ", outs[0].0, rty(self.vt(outs[0].0))),
            _ => {
                let ns: Vec<String> = outs.iter().map(|o| format!("mut v{}", o.0)).collect();
                let ts: Vec<Ty> = outs.iter().map(|o| self.vt(o.0)).collect();
                format!("let ({}): {} = ", ns.join(", "), tuple_ty(&ts))
            }
        }
    }
    /// The outputs read after a call mark the callee's outputs as needed; what it binds is the
    /// callee's needed outputs.
    fn needs(&mut self, f: usize, outs: &[(u32, Ty)], live: &Live) -> (bool, Vec<(u32, Ty)>) {
        let mut any = false;
        for (j, o) in outs.iter().enumerate() {
            if live.contains(&o.0) {
                any = true;
                if !self.need[f][j] {
                    self.need[f][j] = true;
                    self.changed = true;
                }
            }
        }
        let bound = outs.iter().enumerate().filter(|(j, _)| self.need[f][*j]).map(|(_, o)| *o).collect();
        (any, bound)
    }
    /// An argument for parameter j of f, unboxed when the parameter is.
    fn arg(&mut self, f: usize, j: usize, a: &Ex, live: &mut Live) -> String {
        let p = self.funs[f].unwrap().params[j].0;
        match self.pk.get(&p).copied() {
            Some(Ty::F) => match a {
                Ex::App(P::VN, x) => self.ex(&x[0], M::Val, live),
                Ex::Var(q) if self.pk.get(q) == Some(&Ty::F) => self.ex(a, M::Val, live),
                _ => self.ex(&Ex::App(P::VNum, vec![a.clone()]), M::Val, live),
            },
            Some(Ty::S) => match a {
                Ex::App(P::VT, x) | Ex::App(P::VOf, x) => self.ex(&x[0], M::Val, live),
                Ex::App(P::VN, x) => self.ex(&Ex::App(P::Fmt, vec![x[0].clone()]), M::Val, live),
                Ex::Var(q) if self.pk.get(q) == Some(&Ty::S) => self.ex(a, M::Val, live),
                _ => self.ex(&Ex::App(P::VText, vec![a.clone()]), M::Val, live),
            },
            _ => {
                let t = self.vt(p);
                self.ex_as(a, t, live)
            }
        }
    }
    /// Which V parameters are read only as numbers, or only as text: every read is that, or
    /// passes the value on to a parameter of the same kind.
    fn kinds(&mut self, main: &Block) {
        let mut pk: std::collections::HashMap<u32, Ty> = Default::default();
        let mut owner: std::collections::HashMap<u32, (usize, usize)> = Default::default();
        for (i, f) in self.funs.iter().enumerate() {
            if let Some(f) = f {
                for (j, p) in f.params.iter().enumerate() {
                    owner.insert(p.0, (i, j));
                    if p.1 == Ty::V {
                        pk.insert(p.0, Ty::F);
                    }
                }
            }
        }
        // a read seen: Some(kind) when it is one of the two, None for any other
        fn ex_reads(e: &Ex, out: &mut Vec<(u32, Option<Ty>)>) {
            match e {
                Ex::Var(v) => out.push((*v, None)),
                Ex::App(P::VNum, a) if matches!(&a[0], Ex::Var(_)) => {
                    if let Ex::Var(v) = &a[0] {
                        out.push((*v, Some(Ty::F)))
                    }
                }
                Ex::App(P::VText, a) if matches!(&a[0], Ex::Var(_)) => {
                    if let Ex::Var(v) = &a[0] {
                        out.push((*v, Some(Ty::S)))
                    }
                }
                Ex::App(_, a) => {
                    for x in a {
                        ex_reads(x, out)
                    }
                }
                _ => {}
            }
        }
        // (var, kind of the read, or the parameter it is passed to)
        fn walk(b: &Block, out: &mut Vec<(u32, Option<Ty>)>, pass: &mut Vec<(u32, u32)>, funs: &[Option<&Fun>]) {
            let args = |f: usize, a: &[Ex], skip_last: bool, out: &mut Vec<(u32, Option<Ty>)>, pass: &mut Vec<(u32, u32)>| {
                let ps = &funs[f].unwrap().params;
                for (j, x) in a.iter().enumerate() {
                    match x {
                        Ex::Var(v) if j < ps.len() && !(skip_last && j + 1 == ps.len()) => pass.push((*v, ps[j].0)),
                        _ => ex_reads(x, out),
                    }
                }
            };
            for s in &b.stmts {
                match s {
                    Stmt::Spawn { body, .. } => walk(body, out, pass, funs),
                    Stmt::Join { .. } => {}
                    Stmt::Let(_, _, e) | Stmt::Write(e) => ex_reads(e, out),
                    Stmt::Fx(_, a, b, _) => {
                        ex_reads(a, out);
                        ex_reads(b, out)
                    }
                    Stmt::ReadLine(..) | Stmt::Args(..) => {}
                    Stmt::Call { f, args: a, .. } => args(*f, a, false, out, pass),
                    Stmt::Map { f, xs, args: a, .. } => {
                        ex_reads(xs, out);
                        args(*f, a, false, out, pass)
                    }
                    Stmt::Par { calls } => {
                        for (f, a, _) in calls {
                            args(*f, a, false, out, pass)
                        }
                    }
                    Stmt::Branch { c, t, e, .. } => {
                        ex_reads(c, out);
                        walk(t, out, pass, funs);
                        walk(e, out, pass, funs);
                    }
                }
            }
            match &b.end {
                End::Yield(xs) => {
                    for x in xs {
                        ex_reads(x, out)
                    }
                }
                End::Exit(x) | End::Fail(x) => ex_reads(x, out),
                End::Never => {}
            }
        }
        let mut reads = Vec::new();
        let mut pass = Vec::new();
        for f in self.funs.iter().flatten() {
            walk(&f.body, &mut reads, &mut pass, self.funs);
        }
        walk(main, &mut reads, &mut pass, self.funs);
        // the kind each parameter is first read as decides; a read of the other kind, or any
        // other read, keeps it a V
        let mut first: std::collections::HashMap<u32, Ty> = Default::default();
        let mut bad: BTreeSet<u32> = BTreeSet::new();
        for (v, k) in &reads {
            if !pk.contains_key(v) {
                continue;
            }
            match k {
                None => {
                    bad.insert(*v);
                }
                Some(k) => match first.get(v) {
                    Some(k0) if k0 != k => {
                        bad.insert(*v);
                    }
                    _ => {
                        first.insert(*v, *k);
                    }
                },
            }
        }
        // a lane's item comes from the list as a V
        for f in &self.maps_all {
            if let Some(fun) = self.funs[*f] {
                if let Some(p) = fun.params.last() {
                    bad.insert(p.0);
                }
            }
        }
        loop {
            let mut ch = false;
            for (v, p) in &pass {
                if !pk.contains_key(v) {
                    continue;
                }
                // passed on: the same kind as where it goes, else boxed
                let target = if !pk.contains_key(p) || bad.contains(p) { None } else { Some(first.get(p).copied().unwrap_or(Ty::F)) };
                match target {
                    None => {
                        if bad.insert(*v) {
                            ch = true;
                        }
                    }
                    Some(k) => match first.get(v).copied() {
                        Some(k0) if k0 != k => {
                            if bad.insert(*v) {
                                ch = true;
                            }
                        }
                        Some(_) => {}
                        None => {
                            first.insert(*v, k);
                            ch = true;
                        }
                    },
                }
            }
            if !ch {
                break;
            }
        }
        let _ = owner;
        self.pk = pk.into_iter().filter(|(v, _)| !bad.contains(v)).map(|(v, _)| (v, first.get(&v).copied().unwrap_or(Ty::F))).collect();
    }
    /// A call's arguments: those the callee reads, read backward.
    fn call_args(&mut self, f: usize, args: &[Ex], live: &mut Live) -> String {
        let mut xs = Vec::new();
        for j in (0..args.len()).rev() {
            if self.kept(f, j) {
                xs.push(self.arg(f, j, &args[j], live));
            }
        }
        xs.reverse();
        if self.io[f] {
            xs.insert(0, "io".to_string());
        }
        format!("f{}({})", f, xs.join(", "))
    }

    /// A block, read backward from what is read after it; `yn` says which of the values it
    /// gives back are read. In tail position its end returns from the function, and a call of
    /// the function itself goes round its loop again.
    fn block(&mut self, b: &Block, tail: bool, yn: &[bool], want: &[Ty], live: &mut Live) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut stmts: &[Stmt] = &b.stmts;
        let self_call = matches!(b.stmts.last(), Some(Stmt::Call { f, .. }) if *f == self.cur);
        let tail_branch = matches!(b.stmts.last(), Some(Stmt::Branch { .. }));
        if tail && (self_call || tail_branch) && tail_of(&b.stmts, &b.end) {
            let last = b.stmts.last().unwrap();
            stmts = &b.stmts[..b.stmts.len() - 1];
            match last {
                Stmt::Call { f, args, .. } => {
                    self.looped = true;
                    live.clear();
                    let fun = self.funs[*f].unwrap();
                    let mut asg = Vec::new();
                    let mut tmps = Vec::new();
                    for j in (0..args.len()).rev() {
                        if self.kept(*f, j) {
                            let x = self.arg(*f, j, &args[j], live);
                            let t = self.tmp();
                            tmps.push(format!("let {} = {};", t, x));
                            asg.push(format!("v{} = {};", fun.params[j].0, t));
                        }
                    }
                    tmps.reverse();
                    parts.push(format!("{} {} continue;", tmps.join(" "), asg.join(" ")));
                }
                Stmt::Branch { c, t, e, .. } => {
                    let mut lt = Live::new();
                    let tc = self.block(t, true, yn, want, &mut lt);
                    let mut le = Live::new();
                    let ec = self.block(e, true, yn, want, &mut le);
                    live.clear();
                    live.extend(lt);
                    live.extend(le);
                    let cc = self.ex(c, M::Val, live);
                    parts.push(format!("if {} {{\n{}\n}} else {{\n{}\n}}", cc, tc, ec));
                }
                _ => unreachable!(),
            }
        } else {
            match &b.end {
                End::Yield(xs) => {
                    let mut es = Vec::new();
                    for i in (0..xs.len()).rev() {
                        if yn.get(i).copied().unwrap_or(false) {
                            let w = want.get(i).copied().unwrap_or(Ty::V);
                            es.push(self.ex_as(&xs[i], w, live));
                        }
                    }
                    es.reverse();
                    let v = if es.is_empty() { "()".to_string() } else { tuple(&es) };
                    parts.push(if tail { format!("return {};", v) } else { v });
                }
                End::Exit(x) => {
                    let c = self.ex(x, M::Val, live);
                    parts.push(format!("io.exit({})", c));
                }
                End::Fail(x) => {
                    let c = self.ex(x, M::Ref, live);
                    parts.push(format!("io.fail({})", c));
                }
                End::Never => parts.push("unreachable!()".to_string()),
            }
        }
        for s in stmts.iter().rev() {
            let c = self.stmt(s, live);
            if !c.is_empty() {
                parts.push(c);
            }
        }
        parts.reverse();
        parts.join("\n")
    }

    fn stmt(&mut self, s: &Stmt, live: &mut Live) -> String {
        match s {
            Stmt::Join { id, outs } => {
                Self::kill(live, outs);
                format!("{}h{}.join().unwrap();", self.bind(outs), id)
            }
            Stmt::Spawn { id, body } => {
                let xs: Vec<u32> = match &body.end {
                    End::Yield(xs) => xs.iter().filter_map(|x| if let Ex::Var(v) = x { Some(*v) } else { None }).collect(),
                    _ => Vec::new(),
                };
                let yn = vec![true; xs.len()];
                let want: Vec<Ty> = xs.iter().map(|v| self.vt(*v)).collect();
                let mut lb = Live::new();
                let b = self.block(body, false, &yn, &want, &mut lb);
                // the thread takes its own copy of each value it reads
                let caps: Vec<String> = lb.iter().map(|v| format!("let v{} = v{}.clone();", v, v)).collect();
                live.extend(lb);
                format!("let h{} = {{ {} std::thread::Builder::new().stack_size(1 << 28).spawn(move || {{ let mut wio = Io::new(); let io = &mut wio; let r = {{\n{}\n}}; io.flush(); r }}).unwrap() }};", id, caps.join(" "), b)
            }
            Stmt::Let(v, t, e) => {
                if !live.remove(v) {
                    return String::new();
                }
                let vt = self.vt(*v);
                let x = self.ex_as(e, vt, live);
                format!("let mut v{}: {} = {};", v, rty(vt), x)
            }
            Stmt::Write(e) => format!("io.w({});", self.ex(e, M::Ref, live)),
            Stmt::ReadLine(a, b) => {
                live.remove(a);
                live.remove(b);
                format!("let (mut v{}, mut v{}) = io.read_line();", a, b)
            }
            Stmt::Args(a, b) => {
                live.remove(a);
                live.remove(b);
                format!("let mut v{} = args(); let mut v{} = v{}.len() as f64;", a, b, a)
            }
            Stmt::Fx(op, a, b, o) => {
                live.remove(o);
                let y = self.ex(b, M::Ref, live);
                let x = self.ex(a, M::Ref, live);
                format!("let mut v{}: S = io.fx({:?}, {}, {});", o, op, x, y)
            }
            Stmt::Call { f, args, outs } => {
                let (any, bound) = self.needs(*f, outs, live);
                if !any && !self.io[*f] {
                    return String::new();
                }
                Self::kill(live, outs);
                let c = self.call_args(*f, args, live);
                format!("{}{};", self.bind(&bound), c)
            }
            // lanes written as calls with no effects run on spare cores; the rest one after another
            // the build step makes a parallel call only of lanes whose one effect is an error:
            // such a lane gets an output of its own, and main's is written out before
            Stmt::Par { calls } if calls.len() >= 2 => {
                let mut parts: Vec<(String, String, Vec<(u32, Ty)>)> = Vec::new();
                for (f, args, outs) in calls.iter().rev() {
                    let (any, bound) = self.needs(*f, outs, live);
                    if !any {
                        continue;
                    }
                    Self::kill(live, outs);
                    let mut decl = Vec::new();
                    let mut names = Vec::new();
                    for j in (0..args.len()).rev() {
                        if self.kept(*f, j) {
                            let x = self.arg(*f, j, &args[j], live);
                            let t = self.tmp();
                            decl.push(format!("let {} = {};", t, x));
                            names.push(t);
                        }
                    }
                    decl.reverse();
                    names.reverse();
                    if self.io[*f] {
                        names.insert(0, "&mut Io::new()".to_string());
                    }
                    parts.push((decl.join(" "), format!("f{}({})", f, names.join(", ")), bound));
                }
                parts.reverse();
                if parts.is_empty() {
                    return String::new();
                }
                let decls: Vec<String> = parts.iter().map(|p| p.0.clone()).collect();
                if parts.len() == 1 {
                    return format!("{} {}{};", decls.join(" "), self.bind(&parts[0].2), parts[0].1);
                }
                let pat_of = |b: &[(u32, Ty)]| -> String {
                    match b.len() {
                        0 => "_".to_string(),
                        1 => format!("mut v{}", b[0].0),
                        _ => format!("({})", b.iter().map(|o| format!("mut v{}", o.0)).collect::<Vec<_>>().join(", ")),
                    }
                };
                // halves of the lanes on two cores, each half split again while cores are spare
                fn tree(parts: &[(String, String, Vec<(u32, Ty)>)], pat_of: &dyn Fn(&[(u32, Ty)]) -> String) -> (String, String) {
                    if parts.len() == 1 {
                        return (parts[0].1.clone(), pat_of(&parts[0].2));
                    }
                    let m = parts.len() / 2;
                    let (le, lp) = tree(&parts[..m], pat_of);
                    let (re, rp) = tree(&parts[m..], pat_of);
                    (format!("par_go(move || {}, move || {})", le, re), format!("({}, {})", lp, rp))
                }
                let (e, pat) = tree(&parts, &pat_of);
                let flush = if calls.iter().any(|c| self.io[c.0]) { "io.flush(); " } else { "" };
                format!("{} {}let {} = {};", decls.join(" "), flush, pat, e)
            }
            Stmt::Par { calls } => {
                let mut cs = Vec::new();
                for (f, args, outs) in calls.iter().rev() {
                    let c = self.stmt(&Stmt::Call { f: *f, args: args.clone(), outs: outs.clone() }, live);
                    if !c.is_empty() {
                        cs.push(c);
                    }
                }
                cs.reverse();
                cs.join("\n")
            }
            Stmt::Branch { c, t, e, outs } => {
                let pos: Vec<bool> = outs.iter().map(|o| live.contains(&o.0)).collect();
                if !pos.iter().any(|x| *x) && !self.block_io(t) && !self.block_io(e) {
                    return String::new();
                }
                Self::kill(live, outs);
                let after = live.clone();
                let mut lt = after.clone();
                let bw: Vec<Ty> = outs.iter().map(|o| self.vt(o.0)).collect();
                let tc = self.block(t, false, &pos, &bw, &mut lt);
                let mut le = after;
                let ec = self.block(e, false, &pos, &bw, &mut le);
                live.extend(lt);
                live.extend(le);
                let cc = self.ex(c, M::Val, live);
                let bound: Vec<(u32, Ty)> = outs.iter().zip(pos.iter()).filter(|(_, p)| **p).map(|(o, _)| *o).collect();
                format!("{}if {} {{\n{}\n}} else {{\n{}\n}};", self.bind(&bound), cc, tc, ec)
            }
            Stmt::Map { f, xs, args, outs } => {
                let (any, bound) = self.needs(*f, outs, live);
                let fun = self.funs[*f].unwrap();
                let io = self.io[*f];
                if !any && !io {
                    return String::new();
                }
                Self::kill(live, outs);
                let np = fun.params.len();
                let item_num = self.vt(fun.params[np - 1].0) == Ty::F;
                let mut pre = Vec::new();
                let mut names = Vec::new();
                let mut kargs: Vec<(String, Ty)> = Vec::new();
                for j in (0..np - 1).rev() {
                    if j < args.len() && self.kept(*f, j) {
                        let x = self.arg(*f, j, &args[j], live);
                        let t = self.tmp();
                        pre.push(format!("let {} = {};", t, x));
                        let pt = self.vt(fun.params[j].0);
                        names.push(if copy(pt) { t.clone() } else { format!("{}.clone()", t) });
                        kargs.push((t, pt));
                    }
                }
                pre.reverse();
                names.reverse();
                kargs.reverse();
                let xsc = self.ex_to(xs, item_num, live);
                // the lane gives back its needed outputs; each becomes one list
                let kept: Vec<usize> = (0..fun.outs.len()).filter(|j| self.need[*f][*j]).collect();
                let k = kept.len();
                let num_out: Vec<bool> = kept.iter().map(|j| self.tys[outs[*j].0 as usize] == Ty::N).collect();
                let xty = if item_num { "LN" } else { "L" };
                let mut o = String::new();
                let _ = write!(o, "{}{{ let xs_: {} = {}; {} ", self.bind(&bound), xty, xsc, pre.join(" "));
                // a fork over numbers whose lanes give back numbers: one kernel over the column,
                // for the widest vectors the machine it runs on has, cut across its cores
                if !io && item_num && k > 0 && num_out.iter().all(|x| *x) && kargs.iter().all(|(_, t)| copy(*t)) {
                    let id = self.kdefs.len();
                    let outs_p: Vec<String> = (0..k).map(|i| format!("o{}: &mut [f64]", i)).collect();
                    let args_p: Vec<String> = kargs.iter().enumerate().map(|(i, (_, t))| format!("a{}: {}", i, rty(*t))).collect();
                    let mut ps = vec!["xs: &[f64]".to_string()];
                    ps.extend(outs_p.iter().cloned());
                    ps.extend(args_p.iter().cloned());
                    let ps = ps.join(", ");
                    let mut an = vec!["xs".to_string()];
                    an.extend((0..k).map(|i| format!("o{}", i)));
                    an.extend((0..kargs.len()).map(|i| format!("a{}", i)));
                    let an = an.join(", ");
                    let mut call_args: Vec<String> = (0..kargs.len()).map(|i| format!("a{}", i)).collect();
                    call_args.push("xs[i]".to_string());
                    let sets: Vec<String> = (0..k).map(|i| if k == 1 { "o0[i] = r;".to_string() } else { format!("o{}[i] = r.{};", i, i) }).collect();
                    let cuts: Vec<String> = (0..k).map(|i| format!("let o{} = &mut o{}[..n];", i, i)).collect();
                    let mut d = String::new();
                    // a lane taking a remainder runs its exact-for-whole-numbers form in vectors and
                    // the column again exactly when any input was not a whole number below 2^53
                    let fast = match self.ftexts.get(f) {
                        Some((_, _, body)) => body.contains("srem(") && !body.contains("io.") && {
                            let mut other = false;
                            let mut rest = body.as_str();
                            while let Some(k) = rest.find('f') {
                                let after = &rest[k + 1..];
                                let digits = after.chars().take_while(|c| c.is_ascii_digit()).count();
                                let before_ok = k == 0 || !rest[..k].chars().last().map_or(false, |c| c.is_alphanumeric() || c == '_');
                                if digits > 0 && before_ok && after[digits..].starts_with('(') {
                                    other = true;
                                    break;
                                }
                                rest = after;
                            }
                            !other
                        },
                        None => false,
                    };
                    if fast {
                        let (fps, frt, body) = self.ftexts[f].clone();
                        let vps = if fps.is_empty() { "bad: &mut bool".to_string() } else { format!("bad: &mut bool, {}", fps) };
                        let _ = writeln!(d, "#[inline(always)]\nfn f{f}_v({vps}){frt} {{\n{body}\n}}", f = f, vps = vps, frt = frt, body = body.replace("srem(", "sremv(bad, "));
                        let _ = writeln!(d, "#[inline(always)]\nfn kb{id}({ps}) {{ let n = xs.len(); {cuts} let mut bad = false; for i in 0..n {{ let r = f{f}_v(&mut bad, {ca}); {sets} }} if bad {{ for i in 0..n {{ let r = f{f}({ca}); {sets} }} }} }}", id = id, ps = ps, cuts = cuts.join(" "), f = f, ca = call_args.join(", "), sets = sets.join(" "));
                    } else {
                        let _ = writeln!(d, "#[inline(always)]\nfn kb{id}({ps}) {{ let n = xs.len(); {cuts} for i in 0..n {{ let r = f{f}({ca}); {sets} }} }}", id = id, ps = ps, cuts = cuts.join(" "), f = f, ca = call_args.join(", "), sets = sets.join(" "));
                    }
                    let _ = writeln!(d, "#[cfg(target_arch = \"x86_64\")]\n#[target_feature(enable = \"avx512f\")]\nunsafe fn kz{id}({ps}) {{ kb{id}({an}) }}", id = id, ps = ps, an = an);
                    let _ = writeln!(d, "#[cfg(target_arch = \"x86_64\")]\n#[target_feature(enable = \"avx2\")]\nunsafe fn ky{id}({ps}) {{ kb{id}({an}) }}", id = id, ps = ps, an = an);
                    let _ = writeln!(d, "fn kc{id}({ps}) {{\n    #[cfg(target_arch = \"x86_64\")]\n    {{\n        if std::is_x86_feature_detected!(\"avx512f\") {{ return unsafe {{ kz{id}({an}) }}; }}\n        if std::is_x86_feature_detected!(\"avx2\") {{ return unsafe {{ ky{id}({an}) }}; }}\n    }}\n    kb{id}({an})\n}}", id = id, ps = ps, an = an);
                    let chunks: Vec<String> = (0..k).map(|i| format!("let mut i{} = o{}.chunks_mut(size);", i, i)).collect();
                    let nexts: Vec<String> = (0..k).map(|i| format!("let p{} = i{}.next().unwrap();", i, i)).collect();
                    let mut sub = vec!["c".to_string()];
                    sub.extend((0..k).map(|i| format!("p{}", i)));
                    sub.extend((0..kargs.len()).map(|i| format!("a{}", i)));
                    let _ = writeln!(d, "fn k{id}({ps}) {{\n    let t0 = std::time::Instant::now();\n    kr{id}({an});\n    if forks_on() {{ eprintln!(\"s-fork\\t{id}\\t{{}} · {{}} lanes · native kernel · {{:.1}} ms\", simd_level(), xs.len(), t0.elapsed().as_secs_f64() * 1000.0); }}\n}}", id = id, ps = ps, an = an);
                    let _ = writeln!(d, "fn kr{id}({ps}) {{\n    let th = std::thread::available_parallelism().map_or(1, |n| n.get());\n    if th < 2 || xs.len() < (1 << 16) {{ return kc{id}({an}); }}\n    let size = (xs.len() + th - 1) / th;\n    {chunks}\n    std::thread::scope(|s| {{ for c in xs.chunks(size) {{ {nexts} s.spawn(move || kc{id}({sub})); }} }});\n}}", id = id, ps = ps, an = an, chunks = chunks.join(" "), nexts = nexts.join(" "), sub = sub.join(", "));
                    self.kdefs.push(d);
                    for i in 0..k {
                        let _ = write!(o, "let mut o{}: Vec<f64> = vec![0.0; xs_.len()]; ", i);
                    }
                    let mut ka = vec!["&xs_".to_string()];
                    ka.extend((0..k).map(|i| format!("&mut o{}", i)));
                    ka.extend(kargs.iter().map(|(t, _)| t.clone()));
                    let _ = write!(o, "k{}({}); ", id, ka.join(", "));
                } else {
                    names.push(if item_num { "*x".to_string() } else { "x.clone()".to_string() });
                    let callee = if io { format!("f{}(io, {})", f, names.join(", ")) } else { format!("f{}({})", f, names.join(", ")) };
                    for i in 0..k {
                        let _ = write!(o, "let mut o{}: Vec<{}> = Vec::with_capacity(xs_.len()); ", i, if num_out[i] { "f64" } else { "V" });
                    }
                    let push = |r: &str| -> String {
                        (0..k)
                            .map(|i| {
                                let el = if k == 1 { r.to_string() } else { format!("{}.{}", r, i) };
                                if num_out[i] {
                                    return format!("o{}.push({});", i, el);
                                }
                                let conv = match self.outs_ty[*f][kept[i]] {
                                    Ty::V => el,
                                    Ty::F => format!("V::N({})", el),
                                    Ty::S => format!("vof({})", el),
                                    Ty::B => format!("V::N(if {} {{ 1.0 }} else {{ 0.0 }})", el),
                                    Ty::L => format!("V::T(join(&{}, \" \"))", el),
                                    Ty::N => format!("V::T(join_n(&{}, \" \"))", el),
                                };
                                format!("o{}.push({});", i, conv)
                            })
                            .collect::<Vec<_>>()
                            .join(" ")
                    };
                    let it = if item_num { "f64" } else { "V" };
                    if io {
                        let _ = write!(o, "for x in xs_.iter() {{ let r = {}; {} }} ", callee, push("r"));
                    } else {
                        let _ = write!(o, "for part in par_chunks(&xs_, |c: &[{}]| c.iter().map(|x| {}).collect::<Vec<_>>()) {{ for r in part {{ {} }} }} ", it, callee, push("r"));
                    }
                }
                let res: Vec<String> = (0..k).map(|i| format!("Arc::new(o{})", i)).collect();
                let _ = write!(o, "{} }};", if k == 0 { "()".to_string() } else { tuple(&res) });
                o
            }
        }
    }

    /// Every function and main written once: what each reads and what of each is read.
    fn pass(&mut self, main: &Block) -> (String, String) {
        self.kdefs.clear();
        let mut fns = String::new();
        for i in 0..self.funs.len() {
            if let Some(f) = self.funs[i] {
                self.cur = i;
                self.looped = false;
                let mut live = Live::new();
                let yn = self.need[i].clone();
                let fw: Vec<Ty> = self.outs_ty[i].clone();
                let body = self.block(&f.body, true, &yn, &fw, &mut live);
                let lane = self.maps_all.contains(&i);
                let np = f.params.len();
                for (j, p) in f.params.iter().enumerate() {
                    let u = live.contains(&p.0) || (lane && j + 1 == np);
                    if u && !self.used[i][j] {
                        self.used[i][j] = true;
                        self.changed = true;
                    }
                }
                let mut ps: Vec<String> = f.params.iter().enumerate().filter(|(j, _)| self.used[i][*j]).map(|(_, p)| format!("mut v{}: {}", p.0, rty(self.vt(p.0)))).collect();
                if self.io[i] {
                    ps.insert(0, "io: &mut Io".to_string());
                }
                let outs: Vec<Ty> = self.outs_ty[i].iter().enumerate().filter(|(j, _)| self.need[i][*j]).map(|(_, t)| *t).collect();
                let rt = if outs.is_empty() { String::new() } else { format!(" -> {}", tuple_ty(&outs)) };
                let _ = writeln!(fns, "// {}", f.what.replace('\n', " "));
                if self.maps_all.contains(&i) && !self.io[i] {
                    let _ = writeln!(fns, "#[inline(always)]");
                }
                if self.looped {
                    let _ = writeln!(fns, "fn f{}({}){} {{\nloop {{\n{}\n}}\n}}\n", i, ps.join(", "), rt, body);
                    self.ftexts.insert(i, (ps.join(", "), rt.clone(), format!("loop {{\n{}\n}}", body)));
                } else {
                    let _ = writeln!(fns, "fn f{}({}){} {{\n{}\n}}\n", i, ps.join(", "), rt, body);
                    self.ftexts.insert(i, (ps.join(", "), rt.clone(), body.clone()));
                }
            }
        }
        self.cur = usize::MAX;
        let mut live = Live::new();
        let mb = self.block(main, true, &[], &[], &mut live);
        (fns, mb)
    }

    pub fn program(mut self, main: &Block, title: &str) -> String {
        self.analyse();
        fn maps_in(b: &Block, out: &mut Vec<usize>) {
            for s in &b.stmts {
                match s {
                    Stmt::Map { f, .. } => out.push(*f),
                    Stmt::Branch { t, e, .. } => {
                        maps_in(t, out);
                        maps_in(e, out);
                    }
                    _ => {}
                }
            }
        }
        let mut ms = Vec::new();
        maps_in(main, &mut ms);
        for f in self.funs.iter().flatten() {
            maps_in(&f.body, &mut ms);
        }
        self.maps_all = ms;
        self.kinds(main);
        self.numlists_init();
        self.numv_init();
        loop {
            let a = self.numlists(main);
            let b = self.numv(main);
            if !a && !b {
                break;
            }
        }
        self.used = self.funs.iter().map(|f| vec![false; f.map_or(0, |f| f.params.len())]).collect();
        self.need = self.funs.iter().map(|f| vec![false; f.map_or(0, |f| f.outs.len())]).collect();
        let (fns, mb) = loop {
            self.changed = false;
            let r = self.pass(main);
            if !self.changed {
                break r;
            }
        };
        // once more with every function's text known, for the kernels that use it
        let _ = (fns, mb);
        let (fns, mb) = self.pass(main);
        let mut o = String::new();
        let _ = writeln!(o, "// generated by s3 from {}: the residue of the machine's run at build time\n", title.replace('\n', " "));
        o.push_str(PRELUDE);
        o.push('\n');
        for (i, ks) in self.keysets.iter().enumerate() {
            let mut keys: Vec<String> = ks.clone();
            keys.extend(crate::rt::CMD_WORDS.iter().map(|w| w.to_string()));
            keys.sort();
            keys.dedup();
            let ks: Vec<String> = keys.iter().map(|k| format!("{:?}", k)).collect();
            let _ = writeln!(o, "static KS{}: &[&str] = &[{}];", i, ks.join(", "));
            let _ = writeln!(o, "fn runs{}(t: &str) -> bool {{ oneword(t) && (active(t) || KS{}.binary_search(&t).is_ok()) }}", i, i);
            let numkey = keys.iter().any(|k| crate::rt::canon_num(k).is_some());
            let _ = writeln!(o, "fn runsv{}(v: &V) -> bool {{ match v {{ V::N(x) => {}, V::T(t) => runs{}(t), V::X => runs{}(\"\\u{{1}}\") }} }}", i, if numkey { format!("runs{}(&fmt(*x))", i) } else { "false".to_string() }, i, i);
        }
        let lmaps = std::mem::take(&mut self.lmaps);
        for (i, m) in lmaps.iter().enumerate() {
            let mut arms = String::new();
            let mut seen = BTreeSet::new();
            for (j, (n, l)) in m.iter().enumerate() {
                if !seen.insert(n.clone()) {
                    continue;
                }
                let mut lv = Live::new();
                let x = self.ex(l, M::Val, &mut lv);
                let _ = write!(arms, "{:?} => {{ static C{}: OnceLock<L> = OnceLock::new(); C{}.get_or_init(|| {}).clone() }}\n", n, j, j, x);
            }
            let _ = writeln!(o, "fn ln{}(k: &str) -> L {{ use std::sync::OnceLock; match k {{\n{}_ => lnew() }} }}", i, arms);
        }
        let ls: Vec<String> = self.lits.iter().map(|s| format!("{:?}", s)).collect();
        let _ = writeln!(o, "static LIT_SRC: &[&str] = &[{}];", ls.join(", "));
        o.push_str("static LITS: std::sync::OnceLock<Vec<S>> = std::sync::OnceLock::new();\nfn lit(i: usize) -> S { LITS.get_or_init(|| LIT_SRC.iter().map(|s| Arc::from(*s)).collect())[i].clone() }\n\n");
        o.push_str(&fns);
        for d in &self.kdefs {
            o.push_str(d);
            o.push('\n');
        }
        let _ = writeln!(o, "fn s_main(io: &mut Io) {{\n{}\n}}\n", mb);
        o.push_str("fn main() {\n    let h = std::thread::Builder::new().stack_size(1 << 31).spawn(|| { let mut io = Io::new(); s_main(&mut io); io.exit(0.0); }).unwrap();\n    let _ = h.join();\n}\n");
        o
    }
}
