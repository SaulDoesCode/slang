// s → Rust. Every text the program can run is a chunk: a state machine that runs straight
// through its ops and returns to the machine whenever an op pushed a frame or must wait.
use crate::rt::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::fmt::Write as _;

pub const RT_SRC: &str = include_str!("rt.rs");
pub const GRAPH_SRC: &str = include_str!("graph.rs");
pub const MACHINE_SRC: &str = include_str!("machine.rs");
pub const ZONE_SRC: &str = include_str!("zone.rs");
pub const WIRE_SRC: &str = include_str!("wire.rs");
pub const JSON_SRC: &str = include_str!("json.rs");
pub const SESSION_SRC: &str = include_str!("session.rs");
pub const PART_SRC: &str = include_str!("part.rs");
pub const LANES_SRC: &str = include_str!("lanes.rs");

type St = Option<Vec<Option<String>>>;

struct Gen {
    it: Interner,
    srcs: Vec<String>,
    idx: HashMap<String, usize>,
    done: usize,
    fns: String,
    calls: Vec<String>,
    sites: usize,
    forks: String,
    nforks: usize,
}

/// The arms of one chunk under construction.
struct Arms {
    arms: Vec<String>,
    cur: String,
    /// The current arm ends with its own transition.
    closed: bool,
}

impl Arms {
    fn new() -> Arms {
        Arms { arms: Vec::new(), cur: String::new(), closed: false }
    }
    fn idx(&self) -> usize {
        self.arms.len()
    }
    /// Ends the current arm; unless it set its own transition, it falls through to the next.
    fn close(&mut self) {
        let i = self.arms.len();
        let mut a = std::mem::take(&mut self.cur);
        if !self.closed {
            let _ = writeln!(a, "                st = {};", i + 1);
        }
        self.arms.push(a);
        self.closed = false;
    }
    fn open_fresh(&mut self) {
        if !self.cur.is_empty() || self.closed {
            self.close();
        }
    }
}

fn f64_lit(x: f64) -> String {
    if x.is_nan() {
        "f64::NAN".into()
    } else if x == f64::INFINITY {
        "f64::INFINITY".into()
    } else if x == f64::NEG_INFINITY {
        "f64::NEG_INFINITY".into()
    } else {
        format!("{:?}", x)
    }
}

fn val_lit(v: &Val) -> String {
    match v {
        Val::N(x) => format!("Val::N({})", f64_lit(*x)),
        Val::L(s) => format!("Val::L({:?})", s),
        Val::R(s) | Val::K(s, _) => format!("Val::L({:?})", &**s),
        Val::W(s, _) => format!("Val::L({:?})", s),
    }
}

fn push_st(st: &mut St, v: Option<String>) {
    if let Some(s) = st {
        s.push(v);
    }
}

fn may_push(op: &Op) -> bool {
    match op {
        Op::Word(_, _, _) | Op::Cmd(_) | Op::Lookup(_) | Op::Call(_) | Op::CallPass(_) | Op::Lanes(_) => true,
        Op::Native(c) => c.runs_code(),
        Op::Cond(_, i) => may_push(i),
        _ => false,
    }
}

fn may_wait(op: &Op) -> bool {
    match op {
        Op::Lookup(_) => true,
        Op::Cmd(c) | Op::Native(c) => c.may_wait(),
        Op::Cond(_, i) => may_wait(i),
        _ => false,
    }
}

const IND: &str = "                ";
/// Chunks with more arms than this keep the plain loop and match.
const NEST_MAX: usize = 160;

fn goto_with(arm: &str, f: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(arm.len());
    let mut rest = arm;
    while let Some(p) = rest.find("@GOTO(") {
        out.push_str(&rest[..p]);
        let r = &rest[p + 6..];
        let q = r.find(")@").unwrap();
        out.push_str(&f(&r[..q]));
        rest = &r[q + 2..];
    }
    out.push_str(rest);
    out
}
fn goto_loop(arm: &str) -> String {
    goto_with(arm, |n| format!("st = {}; continue 'run;", n))
}
fn goto_nest(arm: &str) -> String {
    goto_with(arm, |n| format!("break 'a{};", n))
}

impl Gen {
    fn chunk(&mut self, src: &str) -> usize {
        if let Some(&i) = self.idx.get(src) {
            return i;
        }
        let i = self.srcs.len();
        self.srcs.push(src.to_string());
        self.idx.insert(src.to_string(), i);
        i
    }

    fn drain(&mut self) -> Result<(), String> {
        while self.done < self.srcs.len() {
            let i = self.done;
            self.done += 1;
            let src = self.srcs[i].clone();
            let code = parse(&src, &mut self.it).map_err(|e| format!("chunk {}: {}", i, e))?;
            let ops = &code.ops;
            let mut a = Arms::new();
            let mut st: St = Some(Vec::new());
            let n = ops.len();
            let mut k = 0;
            while k < n {
                if let Some(m) = self.fuse(&ops[k..], &mut a) {
                    k += m;
                    st = Some(Vec::new());
                    continue;
                }
                self.emit(&ops[k], k + 1 == n, &mut st, &mut a);
                k += 1;
            }
            if !a.cur.is_empty() || a.closed {
                a.close();
            }
            let last = a.arms.len();
            let end = format!("Flow::Yield({})", last);
            if last == 0 || last > NEST_MAX {
                let mut body = String::new();
                for (j, arm) in a.arms.iter().enumerate() {
                    let arm = goto_loop(arm);
                    let _ = write!(body, "            {} => {{\n{}            }}\n", j, arm);
                }
                let body = body.replace(&end, "Flow::Yield(END)").replace("@RESTART@", "st = 0; continue 'run;");
                let _ = write!(
                    self.fns,
                    "fn c{}(rt: &mut Rt, mut st: u32) -> Flow {{\n    let base = rt.fs.len();\n    'run: loop {{\n        match st {{\n{}            _ => return Flow::Done,\n        }}\n    }}\n}}\n\n",
                    i, body
                );
            } else {
                // a switch with fallthrough: block 'aJ ends where arm J starts, so every
                // transition is a forward break and only the entry dispatches on st
                let mut f = String::new();
                let _ = writeln!(f, "fn c{}(rt: &mut Rt, mut st: u32) -> Flow {{\n    let base = rt.fs.len();\n    'top: loop {{", i);
                for j in (0..=last).rev() {
                    let _ = writeln!(f, "    'a{}: {{", j);
                }
                let arms: Vec<String> = (0..last).map(|j| format!("{} => break 'a{},", j, j)).collect();
                let _ = writeln!(f, "        match st {{ {} _ => return Flow::Done }}", arms.join(" "));
                for (j, arm) in a.arms.iter().enumerate() {
                    let _ = writeln!(f, "    }}");
                    let fall = format!("{}st = {};\n", IND, j + 1);
                    let arm = arm.replace(&fall, "");
                    let arm = goto_nest(&arm);
                    f.push_str(&arm);
                }
                let _ = writeln!(f, "    }}\n    return Flow::Done;\n    }}\n}}\n");
                let f = f.replace(&end, "Flow::Yield(END)").replace("@RESTART@", "st = 0; continue 'top;");
                self.fns.push_str(&f);
            }
        }
        Ok(())
    }

    fn local_any(&self, ops: &[Op]) -> bool {
        ops.iter().any(|op| match op {
            Op::Word(_, k, _) | Op::Lookup(k) | Op::SetNum(k) | Op::Inc(k) | Op::Dec(k) => self.it.is_local(*k),
            Op::Push(v) => is_local_name(&v.text()),
            _ => false,
        })
    }

    /// Guarded superinstruction for `operands... command` on an empty stack.
    /// The fast path computes directly; the slow path is the plain op sequence.
    fn fuse(&mut self, ops: &[Op], a: &mut Arms) -> Option<usize> {
        let m = ops.iter().take(4).take_while(|op| matches!(op, Op::Push(_) | Op::Word(..) | Op::Lookup(_))).count();
        let c = ops.get(m)?;
        let numeric = |c: &Op| match c {
            Op::Cmd(Cmd::Add | Cmd::Sub | Cmd::Mul | Cmd::Div | Cmd::Pow | Cmd::Mod) => true,
            Op::Cmd(Cmd::NumEq | Cmd::Lt | Cmd::Gt | Cmd::Le | Cmd::Ge) => true,
            Op::SetNum(_) | Op::Inc(_) | Op::Dec(_) | Op::Cmd(Cmd::SetN) => true,
            _ => false,
        };
        let textual = matches!(c, Op::Cmd(Cmd::Eq | Cmd::Ne | Cmd::AndEq | Cmd::AndNe));
        if !numeric(c) && !textual {
            return None;
        }
        if self.local_any(&ops[..=m]) {
            return None;
        }
        let static_sym = |op: &Op, it: &mut Interner| -> Option<Sym> {
            match op {
                Op::Word(_, k, _) => Some(*k),
                Op::Push(v) => Some(it.sym(&v.text())),
                _ => None,
            }
        };
        let arity = match c {
            Op::SetNum(_) | Op::Inc(_) | Op::Dec(_) => (1, 1),
            Op::Cmd(Cmd::SetN) => (2, 2),
            Op::Cmd(Cmd::Add | Cmd::Sub | Cmd::Mul | Cmd::Div | Cmd::Pow | Cmd::Mod) => (2, 3),
            _ => (2, 2),
        };
        if m < arity.0 || m > arity.1 {
            return None;
        }
        let dest = match c {
            Op::SetNum(k) | Op::Inc(k) | Op::Dec(k) => *k,
            Op::Cmd(Cmd::SetN) => static_sym(&ops[1], &mut self.it)?,
            Op::Cmd(_) if m == 3 => static_sym(&ops[2], &mut self.it)?,
            _ => S_EQ,
        };
        if self.it.is_local(dest) {
            return None;
        }
        let nvals = match c {
            Op::Cmd(Cmd::SetN) => 1,
            _ if m == 3 => 2,
            _ => m,
        };
        let words: Vec<String> = ops[..m]
            .iter()
            .filter_map(|op| match op {
                Op::Word(t, k, _) => Some(format!("(Sym({}), {:?})", k.0, &**t)),
                _ => None,
            })
            .collect();
        self.sites += 1;
        let cmds: Vec<String> = match c {
            Op::Cmd(cmd) => vec![format!("Cmd::{:?}", cmd)],
            _ => vec![],
        };
        a.open_fresh();
        let fast = a.idx();
        let mut f = String::new();
        let _ = writeln!(f, "{}'fast: {{", IND);
        let _ = writeln!(
            f,
            "{}    if !(rt.args.is_empty() && rt.unreplaced({}, &[{}], &[{}])) {{ break 'fast; }}",
            IND,
            self.sites,
            words.join(", "),
            cmds.join(", ")
        );
        for (i, op) in ops[..nvals].iter().enumerate() {
            let e = if textual {
                match op {
                    Op::Push(v) => val_lit(v),
                    Op::Word(t, k, None) => format!("Val::W({:?}, Sym({}))", &**t, k.0),
                    Op::Word(_, _, Some(x)) => format!("Val::N({})", f64_lit(*x)),
                    Op::Lookup(k) => format!("match rt.val_of(Sym({})) {{ Some(v) => v, None => break 'fast }}", k.0),
                    _ => unreachable!(),
                }
            } else {
                match op {
                    Op::Push(v) => f64_lit(v.num()),
                    Op::Word(t, _, None) => f64_lit(parse_num(t)),
                    Op::Word(_, _, Some(x)) => f64_lit(*x),
                    Op::Lookup(k) => format!("match rt.num_of(Sym({})) {{ Some(x) => x, None => break 'fast }}", k.0),
                    _ => unreachable!(),
                }
            };
            let ty = if textual { "Val" } else { "f64" };
            let _ = writeln!(f, "{}    let a{}: {} = {};", IND, i, ty, e);
        }
        let d = dest.0;
        let body = match c {
            Op::Cmd(Cmd::Add) => format!("rt.nset(Sym({}), a0 + a1);", d),
            Op::Cmd(Cmd::Sub) => format!("rt.nset(Sym({}), a0 - a1);", d),
            Op::Cmd(Cmd::Mul) => format!("rt.nset(Sym({}), a0 * a1);", d),
            Op::Cmd(Cmd::Div) => format!("rt.nset(Sym({}), a0 / a1);", d),
            Op::Cmd(Cmd::Pow) => format!("rt.nset(Sym({}), a0.powf(a1));", d),
            Op::Cmd(Cmd::Mod) => format!("rt.nset(Sym({}), a0.rem_euclid(a1));", d),
            Op::Cmd(Cmd::NumEq) => "rt.ow = a0 == a1;".into(),
            Op::Cmd(Cmd::Lt) => "rt.ow = a0 < a1;".into(),
            Op::Cmd(Cmd::Gt) => "rt.ow = a0 > a1;".into(),
            Op::Cmd(Cmd::Le) => "rt.ow = a0 <= a1;".into(),
            Op::Cmd(Cmd::Ge) => "rt.ow = a0 >= a1;".into(),
            Op::Cmd(Cmd::Eq) => "rt.ow = val_eq(&a0, &a1);".into(),
            Op::Cmd(Cmd::Ne) => "rt.ow = !val_eq(&a0, &a1);".into(),
            Op::Cmd(Cmd::AndEq) => "rt.ow = rt.ow && val_eq(&a0, &a1);".into(),
            Op::Cmd(Cmd::AndNe) => "rt.ow = rt.ow && !val_eq(&a0, &a1);".into(),
            Op::Cmd(Cmd::SetN) | Op::SetNum(_) => format!("rt.nset(Sym({}), a0);", d),
            Op::Inc(_) => format!("rt.add_num(Sym({}), a0);", d),
            Op::Dec(_) => format!("rt.add_num(Sym({}), -a0);", d),
            _ => unreachable!(),
        };
        let _ = writeln!(f, "{}    {}", IND, body);
        let _ = writeln!(f, "{}    @GOTO(@AFTER@)@", IND);
        let _ = writeln!(f, "{}}}", IND);
        let _ = writeln!(f, "{}st = {};", IND, fast + 1);
        a.cur = f;
        a.closed = true;
        a.close();
        let mut st: St = Some(Vec::new());
        for op in &ops[..=m] {
            self.emit(op, false, &mut st, a);
        }
        if !a.cur.is_empty() || a.closed {
            a.close();
        }
        let after = a.idx();
        a.arms[fast] = a.arms[fast].replace("@AFTER@", &after.to_string());
        Some(m + 1)
    }

    fn hint(&mut self, st: &St) -> Option<usize> {
        let v = st.as_ref()?;
        let mut parts = Vec::with_capacity(v.len());
        for x in v {
            parts.push(x.as_deref()?);
        }
        let body = parts.join(" ").replace('|', "~");
        if parse(&body, &mut self.it).is_err() {
            return None;
        }
        Some(self.chunk(&body))
    }

    /// The Rust statement for one op; `tail` ops return their flow.
    fn op_code(&mut self, op: &Op, tail: bool, st: &mut St) -> String {
        let mut o = String::new();
        match op {
            Op::Push(v) => {
                let _ = write!(o, "rt.push({});", val_lit(v));
                let t = v.text().into_owned();
                if !matches!(v, Val::N(_)) && !t.trim().is_empty() && parse(&t, &mut self.it).is_ok() {
                    self.chunk(&t);
                }
                push_st(st, Some(t));
            }
            Op::Interp(parts) => {
                let _ = write!(o, "{{ let mut s = String::new();");
                for part in parts {
                    match part {
                        Part::T(t) => {
                            let _ = write!(o, " s.push_str({:?});", t);
                        }
                        Part::V(k) => {
                            let _ = write!(o, " rt.interp(&mut s, Sym({}));", k.0);
                        }
                    }
                }
                let _ = write!(o, " rt.push_string(s); }}");
                push_st(st, None);
            }
            Op::Word(t, k, n) => {
                match n {
                    Some(x) => {
                        let _ = write!(o, "rt.word_n({}, {:?});", f64_lit(*x), &**t);
                    }
                    None => {
                        let _ = write!(o, "rt.word({:?}, Sym({}));", &**t, k.0);
                    }
                }
                push_st(st, Some(t.to_string()));
            }
            Op::Cmd(c) | Op::Native(c) => {
                let m = if matches!(op, Op::Cmd(_)) { "cmd_site" } else { "cmd" };
                let _ = write!(o, "if rt.{}(Cmd::{:?}) {{ return Flow::Done; }}", m, c);
                *st = if c.runs_code() || c.keeps_stack() { None } else { Some(Vec::new()) };
            }
            Op::Pop(k) => {
                let _ = write!(o, "rt.pop_to(Sym({}));", k.0);
                *st = None;
            }
            Op::Lookup(k) => {
                let _ = write!(o, "if rt.lookup(Sym({})) {{ return Flow::Done; }}", k.0);
                *st = None;
            }
            Op::Def(k) => {
                let h = match self.hint(st) {
                    Some(i) => format!("Some({})", i),
                    None => "None".into(),
                };
                let _ = write!(o, "rt.def_script(Sym({}), {});", k.0, h);
                *st = Some(Vec::new());
            }
            Op::SetMem(k) | Op::SetNum(k) | Op::Inc(k) | Op::Dec(k) => {
                let m = match op {
                    Op::SetMem(_) => "set_mem",
                    Op::SetNum(_) => "set_num",
                    Op::Inc(_) => "inc",
                    _ => "dec",
                };
                let _ = write!(o, "rt.{}(Sym({}));", m, k.0);
                *st = Some(Vec::new());
            }
            Op::Call(k) => {
                self.calls.push(self.it.name(*k).to_string());
                if tail {
                    if !self.it.is_local(*k) {
                        let _ = write!(o, "if rt.self_tail(Sym({}), c{}, base) {{ @RESTART@ }} ", k.0, self.done - 1);
                    }
                    let _ = write!(o, "return rt.tail_site(Sym({}));", k.0);
                } else {
                    let _ = write!(o, "rt.call_site(Sym({}));", k.0);
                }
                *st = None;
            }
            Op::CallPass(k) => {
                self.calls.push(self.it.name(*k).to_string());
                if tail {
                    let _ = write!(o, "return rt.tail_pass(Sym({}));", k.0);
                } else {
                    let _ = write!(o, "rt.call_pass(Sym({}));", k.0);
                }
                *st = None;
            }
            Op::Lanes(f) => {
                let i = self.nforks;
                self.nforks += 1;
                for t in &f.texts {
                    self.chunk(t);
                }
                if !f.shared.trim().is_empty() {
                    self.chunk(&f.shared);
                }
                let kernel = match &f.vprog {
                    Some(p) => {
                        self.forks.push_str(&crate::lanes::kernel_rust(p, i));
                        format!("Some(k{})", i)
                    }
                    None => "None".into(),
                };
                let texts: Vec<String> = f.texts.iter().map(|t| format!("{:?}", &**t)).collect();
                let beads = match &f.beads {
                    Some(b) => format!("Some(&[{}])", b.iter().map(|l| format!("&[{}]", l.iter().map(|x| f64_lit(*x)).collect::<Vec<_>>().join(", "))).collect::<Vec<_>>().join(", ")),
                    None => "None".into(),
                };
                let _ = writeln!(
                    self.forks,
                    "static FK{}: ForkSpec = ForkSpec {{ at: {}, unit: {}, texts: &[{}], spread: {}, shared: {:?}, beads: {}, code: {}, kernel: {} }};\n",
                    i,
                    f.at,
                    self.done - 1,
                    texts.join(", "),
                    match f.spread { Some(k) => format!("Some({})", k.0), None => "None".into() },
                    &*f.shared,
                    beads,
                    f.code,
                    kernel
                );
                let _ = write!(o, "rt.lanes_static(&FK{});", i);
                *st = None;
            }
            Op::Cond(t, inner) => {
                let mut ist = st.clone();
                let ic = self.op_code(inner, tail, &mut ist);
                let _ = write!(o, "if {}rt.ow {{ {} }}", if *t { "" } else { "!" }, ic);
                *st = None;
            }
        }
        o
    }

    fn emit(&mut self, op: &Op, tail: bool, st: &mut St, a: &mut Arms) {
        let w = may_wait(op);
        if w {
            a.open_fresh();
        }
        let here = a.idx();
        let code = self.op_code(op, tail, st);
        let _ = writeln!(a.cur, "{}{}", IND, code);
        if w {
            let _ = writeln!(a.cur, "{}if rt.wait.is_some() {{ return Flow::Wait({}); }}", IND, here);
        }
        if may_push(op) {
            let _ = writeln!(a.cur, "{}if rt.fs.len() != base {{ return Flow::Yield({}); }}", IND, here + 1);
            a.close();
        }
    }
}

/// main: path and text of the entry file. embeds: (path, text) pairs compiled in and
/// served by `t`, `***` and `name.` in place of reading the path at run time.
/// Every `name.` whose name is a file on disk or in `lib` is embedded as well, transitively.
/// extra: further texts to compile as chunks (s³ units and boxes).
/// native: Rust source defining `pub static NATIVES: &[(&str, NativeFn)]`, placed in module `ext`.
pub struct Compiled {
    pub fns: String,
    pub syms: Vec<String>,
    pub srcs: Vec<String>,
    pub files: Vec<(String, usize)>,
}

pub fn compile_units(
    main: (&str, &str),
    embeds: &[(String, String)],
    extra: &[String],
    lib: &[(&str, &str)],
) -> Result<Compiled, String> {
    let mut g = Gen {
        it: Interner::new(),
        srcs: Vec::new(),
        idx: HashMap::new(),
        done: 0,
        fns: String::new(),
        calls: Vec::new(),
        sites: 0,
        forks: String::new(),
        nforks: 0,
    };
    parse(main.1, &mut g.it).map_err(|e| format!("{}: {}", main.0, e))?;
    g.chunk(main.1);
    let mut files: Vec<(String, usize)> = Vec::new();
    for (path, text) in embeds {
        parse(text, &mut g.it).map_err(|e| format!("{}: {}", path, e))?;
        files.push((path.clone(), g.chunk(text)));
    }
    for t in extra {
        if parse(t, &mut g.it).is_ok() {
            g.chunk(t);
        }
    }
    loop {
        g.drain()?;
        let calls = std::mem::take(&mut g.calls);
        let mut added = false;
        for name in calls {
            if files.iter().any(|f| f.0 == name) {
                continue;
            }
            let p = name.strip_prefix("./").unwrap_or(&name);
            let base = std::path::Path::new(main.0).parent().map(|d| d.join(&name));
            let disk = if std::path::Path::new(&name).is_file() {
                Some(std::path::PathBuf::from(&name))
            } else {
                base.filter(|q| q.is_file())
            };
            let text = if let Some(q) = disk {
                std::fs::read(q).ok().map(|b| String::from_utf8_lossy(&b).into_owned())
            } else {
                lib.iter().find(|f| f.0 == p).map(|f| f.1.to_string())
            };
            if let Some(text) = text {
                parse(&text, &mut g.it).map_err(|e| format!("{}: {}", name, e))?;
                files.push((name.clone(), g.chunk(&text)));
                added = true;
            }
        }
        if !added && g.done == g.srcs.len() {
            break;
        }
    }
    let fns = format!("use lanes::{{vx, ForkSpec}};\n\n{}{}", g.fns, g.forks);
    Ok(Compiled { fns, syms: g.it.names.iter().map(|n| n.to_string()).collect(), srcs: g.srcs, files })
}

/// The runtime modules a compiled program carries.
pub fn runtime_mods() -> String {
    let mut out = String::new();
    for (name, src) in [("rt", RT_SRC), ("graph", GRAPH_SRC), ("machine", MACHINE_SRC), ("zone", ZONE_SRC), ("wire", WIRE_SRC), ("json", JSON_SRC), ("session", SESSION_SRC), ("part", PART_SRC), ("lanes", LANES_SRC)] {
        let _ = write!(out, "pub mod {} {{\n{}\n}}\n\n", name, src);
    }
    out.push_str("pub use rt::*;\n\n");
    out
}

pub fn tables(c: &Compiled) -> String {
    let mut out = String::new();
    out.push_str(&c.fns);
    out.push_str("static SYMS: &[&str] = &[\n");
    for n in &c.syms {
        let _ = writeln!(out, "    {:?},", n);
    }
    out.push_str("];\n\nstatic CHUNKS: &[(&str, Chunk)] = &[\n");
    for (i, s) in c.srcs.iter().enumerate() {
        let _ = writeln!(out, "    ({:?}, c{}),", s, i);
    }
    out.push_str("];\n\nstatic FILES: &[(&str, usize)] = &[\n");
    for (p, i) in &c.files {
        let _ = writeln!(out, "    ({:?}, {}),", p, i);
    }
    out.push_str("];\n\n");
    out
}

pub fn compile(
    main: (&str, &str),
    embeds: &[(String, String)],
    lib: &[(&str, &str)],
    native: Option<&str>,
) -> Result<String, String> {
    emit(main, embeds, lib, native, false, None)
}

/// The same program as `compile`, linked against the prebuilt s3-core library in place of
/// carrying the runtime's source: build it with `rustc --extern s3_core=<libs3_core.rlib>`.
pub fn compile_linked(
    main: (&str, &str),
    embeds: &[(String, String)],
    lib: &[(&str, &str)],
    native: Option<&str>,
) -> Result<String, String> {
    emit(main, embeds, lib, native, true, None)
}

/// `compile`, folded when `fold` is on: the program starts where it first reaches outside.
/// Also says what was folded.
pub fn compile_with(main: (&str, &str), embeds: &[(String, String)], lib: &[(&str, &str)], native: Option<&str>, fold: bool) -> Result<(String, String), String> {
    let f = if fold && native.is_none() { self::fold(main, lib) } else { None };
    let note = f.as_ref().map(|f| f.note.clone()).unwrap_or_default();
    Ok((emit(main, embeds, lib, native, false, f.as_ref())?, note))
}

/// `compile_linked`, folded when `fold` is on.
pub fn compile_linked_with(main: (&str, &str), embeds: &[(String, String)], lib: &[(&str, &str)], native: Option<&str>, fold: bool) -> Result<(String, String), String> {
    let f = if fold && native.is_none() { self::fold(main, lib) } else { None };
    let note = f.as_ref().map(|f| f.note.clone()).unwrap_or_default();
    Ok((emit(main, embeds, lib, native, true, f.as_ref())?, note))
}

fn emit(main: (&str, &str), embeds: &[(String, String)], lib: &[(&str, &str)], native: Option<&str>, linked: bool, fold: Option<&Folded>) -> Result<String, String> {
    if let Some(w) = fold.and_then(|f| f.whole.as_ref()) {
        return Ok(w.clone());
    }
    let c = match fold {
        Some(f) => compile_units((main.0, &f.rest), embeds, &f.units, lib)?,
        None => compile_units(main, embeds, &[], lib)?,
    };
    let mut out = String::new();
    let _ = writeln!(out, "// generated by s3 from {}{}{}", main.0, if linked { ", linked against s3-core" } else { "" }, if fold.is_some() { ", folded" } else { "" });
    out.push_str("#![allow(dead_code, unused_imports, unused_variables, unused_mut, unreachable_code, unused_labels, unused_assignments, clippy::all)]\n\n");
    if linked {
        out.push_str("use s3_core::rt::*;\nuse s3_core::lanes;\n\n");
    } else {
        out.push_str(&runtime_mods());
    }
    out.push_str("// program\n\n");
    out.push_str(&tables(&c));
    match native {
        Some(src) => {
            out.push_str("mod ext {\n    #![allow(unused_imports)]\n    use super::*;\n\n");
            out.push_str(src);
            out.push_str("\n}\n\nstatic PROGRAM: Program = Program::with_natives(SYMS, CHUNKS, FILES, ext::NATIVES);\n\n");
        }
        None => out.push_str("static PROGRAM: Program = Program::new(SYMS, CHUNKS, FILES);\n\n"),
    }
    match fold {
        Some(f) => {
            out.push_str(&f.fold);
            out.push_str("\nfn main() {\n    s_main(&PROGRAM, Entry::Folded(&FOLD), std::env::args().skip(1).collect())\n}\n");
        }
        None => out.push_str("fn main() {\n    s_main(&PROGRAM, Entry::Chunk(0), std::env::args().skip(1).collect())\n}\n"),
    }
    Ok(out)
}

/// The prebuilt s3-core library next to a binary of this workspace (the server, a test):
/// the rlib and the folder of its dependencies, when both are there.
pub fn rlib_near(exe: &std::path::Path) -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let mut dir = exe.parent()?.to_path_buf();
    for _ in 0..3 {
        // the newest deps/libs3_core-<hash>.rlib with its .rmeta beside it: a cargo that leaves
        // metadata stubs in rlibs keeps the metadata there; else target/<profile>/libs3_core.rlib
        let deps = dir.join("deps");
        let mut best: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
        if let Ok(rd) = std::fs::read_dir(&deps) {
            for e in rd.flatten() {
                let p = e.path();
                let n = e.file_name().to_string_lossy().into_owned();
                if n.starts_with("libs3_core-") && n.ends_with(".rlib") && p.with_extension("rmeta").is_file() {
                    if let Ok(t) = std::fs::metadata(&p).and_then(|m| m.modified()) {
                        if best.as_ref().map_or(true, |b| t > b.0) {
                            best = Some((t, p));
                        }
                    }
                }
            }
        }
        if let Some((_, r)) = best {
            return Some((r, deps));
        }
        let top = dir.join("libs3_core.rlib");
        if top.is_file() {
            return Some((top, deps));
        }
        dir = dir.parent()?.to_path_buf();
    }
    None
}

/// rustc arguments that link a program against the s3-core library rlib_near found: the rlib,
/// its .rmeta when there is one (it holds the metadata when the rlib is a stub), and its deps.
pub fn link_args(rlib: &std::path::Path, deps: &std::path::Path) -> Vec<String> {
    let mut a = vec!["--extern".to_string(), format!("s3_core={}", rlib.display())];
    let meta = rlib.with_extension("rmeta");
    if meta.is_file() {
        a.push("--extern".to_string());
        a.push(format!("s3_core={}", meta.display()));
    }
    a.push("-L".to_string());
    a.push(format!("dependency={}", deps.display()));
    a
}

/// A world as one Rust program: every unit compiled, the spec embedded, a session as main.
pub fn compile_spec(spec_json: &str, units: &[String], main_unit: &str, lib: &[(&str, &str)], native: Option<&str>) -> Result<String, String> {
    let c = compile_units(("world", main_unit), &[], units, lib)?;
    let mut out = String::new();
    out.push_str("// generated by s3 from a world\n");
    out.push_str("#![allow(dead_code, unused_imports, unused_variables, unused_mut, unreachable_code, unused_labels, unused_assignments, clippy::all)]\n\n");
    out.push_str(&runtime_mods());
    out.push_str("// program\n\n");
    out.push_str(&tables(&c));
    let _ = writeln!(out, "static SPEC: &str = {:?};\n", spec_json);
    match native {
        Some(src) => {
            out.push_str("mod ext {\n    #![allow(unused_imports)]\n    use super::*;\n\n");
            out.push_str(src);
            out.push_str("\n}\n\nstatic PROGRAM: Program = Program::with_natives(SYMS, CHUNKS, FILES, ext::NATIVES);\n\n");
        }
        None => out.push_str("static PROGRAM: Program = Program::new(SYMS, CHUNKS, FILES);\n\n"),
    }
    out.push_str(MAIN_WORLD);
    Ok(out)
}

const MAIN_WORLD: &str = r#"fn main() {
    CLI.store(true, std::sync::atomic::Ordering::Relaxed);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let h = std::thread::Builder::new().stack_size(MAIN_STACK).spawn(move || {
        let spec = session::Spec::from_json(&json::J::parse(SPEC).expect("embedded spec"));
        part::world_main(spec, &PROGRAM, args, "s")
    }).expect("spawn");
    std::process::exit(h.join().unwrap_or(101))
}
"#;

// ---------- folding: what a program does before it reaches outside, done at build time ----------

static FOLDP: Program = Program::with_lib(crate::LIB);
/// Ops and time a build spends folding.
const FOLD_OPS: u64 = 400_000_000;
const FOLD_MS: u128 = 4000;
/// A folded state bigger than this is cheaper to compute again at run time.
const FOLD_BYTES: usize = 8 << 20;

/// A program folded at build time: the rest of its text, the `static FOLD`, the texts its
/// folded scripts and replacements run, and a line saying what was folded.
pub struct Folded {
    pub rest: String,
    pub fold: String,
    pub units: Vec<String>,
    pub note: String,
    /// A program that never reached outside: its whole Rust source, with no machine in it.
    pub whole: Option<String>,
}

fn fold_machine(text: &str, files: &FxMap<String, String>, buf: &Arc<Mutex<Vec<u8>>>) -> Rt {
    let mut rt = Rt::with_out(&FOLDP, Out::Buf(buf.clone()));
    rt.fold = true;
    rt.stdin = false;
    rt.file_texts = Some(files.clone());
    rt.unit_ids.insert(text.to_string(), 0);
    rt
}

/// Runs a folding machine until it stops: its status, and whether it ran out of ops or time.
fn fold_run(rt: &mut Rt, text: &str) -> (Status, bool) {
    if let Some(st) = rt.guarded(|rt| rt.run_src(text)) {
        return (st, false);
    }
    let t = std::time::Instant::now();
    let mut ops = 0u64;
    loop {
        let st = rt.run(1 << 20);
        ops += 1 << 20;
        match st {
            Status::Ran if ops < FOLD_OPS && t.elapsed().as_millis() < FOLD_MS => continue,
            Status::Ran => return (st, true),
            st => return (st, false),
        }
    }
}

fn fv(v: &Val) -> String {
    match v {
        Val::N(x) => format!("FV::N({})", f64_lit(*x)),
        Val::L(s) => format!("FV::T({:?})", s),
        Val::R(s) => format!("FV::T({:?})", &**s),
        Val::W(s, _) => format!("FV::W({:?})", s),
        Val::K(s, _) => format!("FV::W({:?})", &**s),
    }
}

/// Folds main: runs it at build time until it first reaches outside the machine (input,
/// files, the shell, threads, the network, random numbers, the arguments) or runs out of
/// time, and returns the state there with the rest of the text. None when nothing folds.
pub fn fold(main: (&str, &str), lib: &[(&str, &str)]) -> Option<Folded> {
    let text = main.1;
    // the files the compiled program would carry, read once
    let c = compile_units(main, &[], &[], lib).ok()?;
    let files: FxMap<String, String> = c.files.iter().map(|(n, i)| (n.clone(), c.srcs[*i].clone())).collect();
    let buf = Arc::new(Mutex::new(Vec::new()));
    let mut rt = fold_machine(text, &files, &buf);
    let (st, out_of_time) = fold_run(&mut rt, text);
    rt.flush();
    let out = String::from_utf8_lossy(&buf.lock().unwrap()).into_owned();
    let end = match (&st, &rt.fold_hit, out_of_time) {
        (Status::Done, None, false) => Some("    std::process::exit(0)".to_string()),
        (Status::Exit(c), None, _) => Some(format!("    std::process::exit({})", c)),
        (Status::Error(e), None, _) => Some(format!("    eprintln!(\"\\ns: {{}}\", {:?});\n    std::process::exit(1)", e)),
        _ => None,
    };
    if let Some(end) = end {
        // the program never reached outside: it is what it printed and how it ended, and the
        // binary is that, with no machine in it
        if out.len() > FOLD_BYTES {
            return None;
        }
        let whole = format!(
            "// generated by s3 from {}, folded whole: {} ops at build time\nuse std::io::Write;\n\nstatic OUT: &str = {:?};\n\nfn main() {{\n    let mut o = std::io::stdout().lock();\n    let _ = o.write_all(OUT.as_bytes());\n    let _ = o.flush();\n    drop(o);\n{}\n}}\n",
            main.0, rt.ops, out, end
        );
        return Some(Folded { rest: String::new(), fold: String::new(), units: Vec::new(), note: format!("folded whole: {} ops at build time", rt.ops), whole: Some(whole) });
    }
    let k = rt.fold_top;
    let why = rt.fold_hit.clone().unwrap_or_else(|| "the time for folding".to_string());
    if k == 0 {
        return None;
    }
    // the same run again, stopped before op k of the program's own text
    let buf = Arc::new(Mutex::new(Vec::new()));
    let mut rt = fold_machine(text, &files, &buf);
    rt.breaks.insert(k as u64);
    let (st, _) = fold_run(&mut rt, text);
    rt.flush();
    if !matches!(st, Status::Break) || rt.fs.len() != 1 || rt.fold_hit.is_some() {
        return None;
    }
    let code = rt.code_of(text).ok()?;
    let at = *code.spans.get(k as usize)? as usize;
    let rest: String = text.chars().skip(at).collect();
    // lanes held at the join are not carried: a rest that reads them is not folded
    if rt.joined.is_some() || rt.view.is_some() {
        let lane_words = ["lanes", "lane", "endlane", "collect", "into", "reduce", "lanewise"];
        if rest.split_whitespace().any(|w| lane_words.contains(&w)) {
            return None;
        }
    }
    if !rt.files.is_empty() {
        return None;
    }
    let out = String::from_utf8_lossy(&buf.lock().unwrap()).into_owned();
    let name = |rt: &Rt, i: usize| rt.it.name(Sym(i as u32)).to_string();
    let keep = |i: usize| i != S_ARGS.0 as usize && i != S_ARGC.0 as usize;
    let mut units = Vec::new();
    let mut s = String::new();
    let _ = write!(s, "static FOLD: Fold = Fold {{\n    out: {:?},\n    end: None,\n    mem: &[", out);
    for (i, v) in rt.mem.iter().enumerate() {
        if let (Some(v), true) = (v, keep(i)) {
            let _ = write!(s, "({:?}, {}), ", name(&rt, i), fv(v));
        }
    }
    s.push_str("],\n    nums: &[");
    for (i, x) in rt.nums.iter().enumerate() {
        if let (Some(x), true) = (x, keep(i)) {
            let _ = write!(s, "({:?}, {}), ", name(&rt, i), f64_lit(*x));
        }
    }
    s.push_str("],\n    lists: &[");
    for (i, l) in rt.lists.iter().enumerate() {
        if let (Some(l), true) = (l, keep(i)) {
            match l {
                List::N(xs) => {
                    let _ = write!(s, "({:?}, FL::N(&[{}])), ", name(&rt, i), xs.iter().map(|x| f64_lit(*x)).collect::<Vec<_>>().join(", "));
                }
                List::V(vs) => {
                    let _ = write!(s, "({:?}, FL::V(&[{}])), ", name(&rt, i), vs.iter().map(fv).collect::<Vec<_>>().join(", "));
                }
            }
        }
    }
    s.push_str("],\n    scripts: &[");
    for (i, sc) in rt.scripts.iter().enumerate() {
        if let Some((sc, gen)) = sc {
            match sc {
                Script::U(c) => {
                    units.push(c.src.to_string());
                    let _ = write!(s, "({:?}, {:?}, {}), ", name(&rt, i), &*c.src, gen);
                }
                _ => return None,
            }
        }
    }
    s.push_str("],\n    repl: &[");
    let fresh = Rt::with_out(&FOLDP, Out::Buf(Arc::new(Mutex::new(Vec::new()))));
    for (key, layers) in rt.repl.iter() {
        let same = fresh.repl.get(key).map_or(false, |f| f.len() == layers.len() && layers.iter().all(|m| matches!(m.code, Some(Script::N(_)))));
        if same {
            continue;
        }
        let _ = write!(s, "({:?}, &[", &**key);
        for m in layers {
            let code = match &m.code {
                None => "FC::None".to_string(),
                Some(Script::U(c)) => {
                    units.push(c.src.to_string());
                    format!("FC::Src({:?})", &*c.src)
                }
                Some(Script::N(_)) => "FC::Native".to_string(),
                Some(Script::C(_)) => return None,
            };
            let _ = write!(s, "FM {{ val: {}, code: {}, gen: {} }}, ", fv(&m.val), code, m.gen);
        }
        s.push_str("]), ");
    }
    let _ = write!(s, "],\n    gen: {},\n    ow: {},\n    repl_num: {},\n    frames: &[", rt.gen, rt.ow, rt.repl_num);
    for fr in &rt.frames {
        let _ = write!(s, "&[{}], ", fr.iter().map(fv).collect::<Vec<_>>().join(", "));
    }
    let _ = write!(s, "],\n    args: &[{}],\n}};\n", rt.args.iter().map(fv).collect::<Vec<_>>().join(", "));
    if s.len() > FOLD_BYTES {
        return None;
    }
    let line = text.chars().take(at).filter(|&c| c == '\n').count() + 1;
    Some(Folded { rest, fold: s, units, whole: None, note: format!("folded to line {}: {} ops at build time; the program reaches outside at {}", line, rt.ops, why) })
}
