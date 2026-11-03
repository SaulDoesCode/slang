// The residue written as Bend: pure functions in direct style, functions with effects as IO
// blocks that continue in a new definition wherever a call gives back more than one value.
use super::ir::*;
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// Lanes a parallel half runs in order instead of halving again.
pub const PAR_RUN: usize = 4096;

pub const PRELUDE: &str = include_str!("prelude.bend");
pub const READ_LINE_C: &str = include_str!("s_read_line.c");
pub const READ_LINE_JS: &str = include_str!("s_read_line.js");
pub const FX_C: &str = include_str!("s_fx.c");
pub const FX_JS: &str = include_str!("s_fx.js");

pub struct Emit<'a> {
    pub lmaps: Vec<Vec<(String, Ex)>>,
    funs: &'a [Option<&'a Fun>],
    tys: &'a [Ty],
    keysets: &'a [Vec<String>],
    io: Vec<bool>,
    recs: Vec<Vec<Ty>>,
    defs: Vec<String>,
    maps: Vec<usize>,
    maps_all: Vec<usize>,
    used: Vec<Vec<bool>>,
    n: usize,
}

fn lit_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            '\r' => o.push_str("\\r"),
            '\0' => o.push_str("\\0"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{{{:x}}}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// A text looked up among keys by halving: NAME(+k: String) -> TY, the key's value, else def.
fn search(name: &str, mut keys: Vec<(String, String)>, def: &str, ty: &str) -> String {
    keys.sort_by(|a, b| a.0.cmp(&b.0));
    keys.dedup_by(|a, b| a.0 == b.0);
    let mut defs = String::new();
    fn node(name: &str, keys: &[(String, String)], lo: usize, hi: usize, def: &str, ty: &str, defs: &mut String) -> String {
        if lo >= hi {
            return def.to_string();
        }
        if hi - lo == 1 {
            let _ = writeln!(defs, "@unsafe def {}.e{}(c: Bool) -> {}:\n  match c:\n    case True{{}}:\n      {}\n    case False{{}}:\n      {}\n", name, lo, ty, keys[lo].1, def);
            return format!("{}.e{}(String.eq(k, {}))", name, lo, lit_str(&keys[lo].0));
        }
        let mid = (lo + hi) / 2;
        let a = node(name, keys, lo, mid, def, ty, defs);
        let b = node(name, keys, mid, hi, def, ty, defs);
        let _ = writeln!(defs, "@unsafe def {}.b{}_{}(c: Bool, +k: String) -> {}:\n  match c:\n    case True{{}}:\n      {}\n    case False{{}}:\n      {}\n", name, lo, hi, ty, a, b);
        format!("{}.b{}_{}(String.is_lt(k, {}), k)", name, lo, hi, lit_str(&keys[mid].0))
    }
    let n = keys.len();
    let root = node(name, &keys, 0, n, def, ty, &mut defs);
    let _ = writeln!(defs, "@unsafe def {}(+k: String) -> {}:\n  {}\n", name, ty, root);
    defs
}

fn lit_num(x: f64) -> String {
    if x.is_nan() {
        return "F32.div(0.0, 0.0)".to_string();
    }
    if x.is_infinite() {
        return if x > 0.0 { "F32.div(1.0, 0.0)".to_string() } else { "F32.neg(F32.div(1.0, 0.0))".to_string() };
    }
    if x < 0.0 || (x == 0.0 && x.is_sign_negative()) {
        return format!("F32.neg({})", lit_num(-x));
    }
    let s = format!("{:?}", x);
    if s.contains('e') {
        format!("{:.1}", x)
    } else if s.contains('.') {
        s
    } else {
        format!("{}.0", s)
    }
}

thread_local! {
    /// Which parameters each function reads, once known: code that calls a function passes only those.
    static USED: std::cell::RefCell<Vec<Vec<bool>>> = std::cell::RefCell::new(Vec::new());
}
fn kept_arg(f: usize, j: usize) -> bool {
    USED.with(|u| u.borrow().get(f).and_then(|x| x.get(j)).copied().unwrap_or(true))
}

fn block_vars(b: &Block, used: &mut BTreeSet<u32>, def: &mut BTreeSet<u32>) {
    for s in &b.stmts {
        stmt_vars(s, used, def);
    }
    end_vars(&b.end, used);
}
fn ex_used(e: &Ex, used: &mut BTreeSet<u32>) {
    let mut v = Vec::new();
    e.vars(&mut v);
    used.extend(v);
}
fn end_vars(e: &End, used: &mut BTreeSet<u32>) {
    match e {
        End::Yield(xs) => {
            for x in xs {
                ex_used(x, used)
            }
        }
        End::Exit(x) | End::Fail(x) => ex_used(x, used),
        End::Never => {}
    }
}
fn stmt_vars(s: &Stmt, used: &mut BTreeSet<u32>, def: &mut BTreeSet<u32>) {
    match s {
        Stmt::Spawn { body, .. } => block_vars(body, used, def),
        Stmt::Join { outs, .. } => def.extend(outs.iter().map(|o| o.0)),
        Stmt::Let(v, _, e) => {
            ex_used(e, used);
            def.insert(*v);
        }
        Stmt::Write(e) => ex_used(e, used),
        Stmt::Fx(_, a, b, o) => {
            ex_used(a, used);
            ex_used(b, used);
            def.insert(*o);
        }
        Stmt::ReadLine(a, b) | Stmt::Args(a, b) => {
            def.insert(*a);
            def.insert(*b);
        }
        Stmt::Call { f, args, outs } => {
            for (j, a) in args.iter().enumerate() {
                if kept_arg(*f, j) {
                    ex_used(a, used)
                }
            }
            def.extend(outs.iter().map(|o| o.0));
        }
        Stmt::Branch { c, t, e, outs } => {
            ex_used(c, used);
            block_vars(t, used, def);
            block_vars(e, used, def);
            def.extend(outs.iter().map(|o| o.0));
        }
        Stmt::Map { f, xs, args, outs } => {
            ex_used(xs, used);
            for (j, a) in args.iter().enumerate() {
                if kept_arg(*f, j) {
                    ex_used(a, used)
                }
            }
            def.extend(outs.iter().map(|o| o.0));
        }
        Stmt::Par { calls } => {
            for (_, args, outs) in calls {
                for a in args {
                    ex_used(a, used)
                }
                def.extend(outs.iter().map(|o| o.0));
            }
        }
    }
}
/// The variables a piece of code reads that it does not bind.
fn free(stmts: &[Stmt], end: &End) -> Vec<u32> {
    let mut used = BTreeSet::new();
    let mut def = BTreeSet::new();
    for s in stmts {
        stmt_vars(s, &mut used, &mut def);
    }
    end_vars(end, &mut used);
    used.difference(&def).cloned().collect()
}

impl<'a> Emit<'a> {
    pub fn new(funs: &'a [Option<&'a Fun>], tys: &'a [Ty], keysets: &'a [Vec<String>]) -> Emit<'a> {
        Emit { lmaps: Vec::new(), funs, tys, keysets, io: vec![false; funs.len()], recs: Vec::new(), defs: Vec::new(), maps: Vec::new(), maps_all: Vec::new(), used: Vec::new(), n: 0 }
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
    /// Which parameters each function reads: a parameter only passed on to a parameter that
    /// is not read is not read either.
    fn uses(&mut self) {
        let n = self.funs.len();
        let mut used: Vec<Vec<bool>> = (0..n).map(|i| vec![false; self.funs[i].map_or(0, |f| f.params.len())]).collect();
        let mut edges: Vec<(usize, usize, usize, usize)> = Vec::new();
        fn walk(b: &Block, ps: &[u32], i: usize, used: &mut Vec<Vec<bool>>, edges: &mut Vec<(usize, usize, usize, usize)>) {
            let mark = |e: &Ex, used: &mut Vec<Vec<bool>>| {
                let mut v = Vec::new();
                e.vars(&mut v);
                for x in v {
                    if let Some(p) = ps.iter().position(|q| *q == x) {
                        used[i][p] = true;
                    }
                }
            };
            let pass = |f: usize, args: &[Ex], used: &mut Vec<Vec<bool>>, edges: &mut Vec<(usize, usize, usize, usize)>| {
                for (j, a) in args.iter().enumerate() {
                    match a {
                        Ex::Var(x) if ps.contains(x) => edges.push((i, ps.iter().position(|q| q == x).unwrap(), f, j)),
                        _ => mark(a, used),
                    }
                }
            };
            for st in &b.stmts {
                match st {
                    Stmt::Spawn { body, .. } => walk(body, ps, i, used, edges),
                    Stmt::Join { .. } => {}
                    Stmt::Let(_, _, e) | Stmt::Write(e) => mark(e, used),
                    Stmt::Fx(_, a, b, _) => {
                        mark(a, used);
                        mark(b, used);
                    }
                    Stmt::ReadLine(..) | Stmt::Args(..) => {}
                    Stmt::Call { f, args, .. } => pass(*f, args, used, edges),
                    Stmt::Map { f, xs, args, .. } => {
                        mark(xs, used);
                        pass(*f, args, used, edges);
                    }
                    Stmt::Branch { c, t, e, .. } => {
                        mark(c, used);
                        walk(t, ps, i, used, edges);
                        walk(e, ps, i, used, edges);
                    }
                    Stmt::Par { calls } => {
                        for (f, args, _) in calls {
                            pass(*f, args, used, edges);
                        }
                    }
                }
            }
            match &b.end {
                End::Yield(xs) => {
                    for x in xs {
                        mark(x, used)
                    }
                }
                End::Exit(x) | End::Fail(x) => mark(x, used),
                End::Never => {}
            }
        }
        for i in 0..n {
            if let Some(f) = self.funs[i] {
                let ps: Vec<u32> = f.params.iter().map(|p| p.0).collect();
                walk(&f.body, &ps, i, &mut used, &mut edges);
            }
        }
        loop {
            let mut changed = false;
            for &(i, p, f, j) in &edges {
                let callee = used.get(f).and_then(|u| u.get(j)).copied().unwrap_or(true);
                if callee && !used[i][p] {
                    used[i][p] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        // a lane's item always stays: the map passes it
        for i in 0..n {
            if let Some(l) = used[i].last_mut() {
                if self.maps_all.contains(&i) {
                    *l = true;
                }
            }
        }
        USED.with(|u| *u.borrow_mut() = used.clone());
        self.used = used;
    }
    fn kept(&self, f: usize, args: &[Ex]) -> Vec<String> {
        args.iter().enumerate().filter(|(j, _)| self.used.get(f).and_then(|u| u.get(*j)).copied().unwrap_or(true)).map(|(_, e)| self.ex(e)).collect()
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

    fn rec(&mut self, tys: &[Ty]) -> String {
        let i = match self.recs.iter().position(|r| r == tys) {
            Some(i) => i,
            None => {
                self.recs.push(tys.to_vec());
                self.recs.len() - 1
            }
        };
        format!("O{}", i)
    }
    fn out_ty(&mut self, tys: &[Ty]) -> String {
        match tys.len() {
            0 => "Unit".to_string(),
            1 => tys[0].bend().to_string(),
            _ => self.rec(tys),
        }
    }
    fn tuple(&mut self, tys: &[Ty], xs: &[String]) -> String {
        match xs.len() {
            0 => "Unit{}".to_string(),
            1 => xs[0].clone(),
            _ => format!("{}{{{}}}", self.rec(tys), xs.join(", ")),
        }
    }
    fn fresh(&mut self, p: &str) -> String {
        self.n += 1;
        format!("{}{}", p, self.n)
    }

    pub fn ex(&self, e: &Ex) -> String {
        match e {
            Ex::Var(v) => format!("v{}", v),
            Ex::Num(x) => lit_num(*x),
            Ex::Str(s) => lit_str(s),
            Ex::Bool(b) => if *b { "True{}".into() } else { "False{}".into() },
            Ex::Strs(xs) => format!("{{[{}] : List<&2, String>}}", xs.iter().map(|x| lit_str(x)).collect::<Vec<_>>().join(", ")),
            Ex::App(p, a) => {
                let x: Vec<String> = a.iter().map(|e| self.ex(e)).collect();
                let f = |n: &str| format!("{}({})", n, x.join(", "));
                match p {
                    P::Add => f("F32.add"),
                    P::Sub => f("F32.sub"),
                    P::Mul => f("F32.mul"),
                    P::Div => f("F32.div"),
                    P::Pow => f("F32.pow"),
                    P::Mod => f("S.rem"),
                    P::Lt => f("F32.is_lt"),
                    P::Gt => f("F32.is_gt"),
                    P::Le => f("F32.is_le"),
                    P::Ge => f("F32.is_ge"),
                    P::NEq => f("F32.is_eq"),
                    P::Not => f("Bool.not"),
                    P::And => f("Bool.and"),
                    P::VEq => f("V.eq"),
                    P::SEq => f("String.eq"),
                    P::Fmt => f("S.fmt"),
                    P::Parse => f("S.parse"),
                    P::VN => format!("N{{{}}}", x[0]),
                    P::VT => format!("T{{{}}}", x[0]),
                    P::VX => "X{}".to_string(),
                    P::IsX => f("V.absent"),
                    P::VOf => f("V.of"),
                    P::VText => f("V.text"),
                    P::VNum => f("V.num"),
                    P::Cat => format!("({})", x.join(" ++ ")),
                    P::SLen => f("S.slen"),
                    P::LNew => "S.lnew()".to_string(),
                    P::LPush => f("S.lpush"),
                    P::LPop => f("S.lpop"),
                    P::LLast => f("S.llast"),
                    P::LGet => f("S.lget"),
                    P::LHas => f("S.lhas"),
                    P::LSet => f("S.lset"),
                    P::LLen => f("S.llen"),
                    P::LCat => format!("S.lcat({}, {})", x[1], x[0]),
                    P::LTake => f("S.ltake"),
                    P::LDrop => f("S.ldrop"),
                    P::SplitWs => f("S.splitws"),
                    P::SplitChars => f("S.splitchars"),
                    P::SplitBy => f("S.splitby"),
                    P::Join => f("S.join"),
                    P::Runs => {
                        let k = match &a[1] {
                            Ex::Num(k) => *k as usize,
                            _ => 0,
                        };
                        format!("S.runs{}({})", k, x[0])
                    }
                    P::If => f("S.pickv"),
                    P::FxOk => f("S.fx_ok"),
                    P::FxVal => f("S.fx_val"),
                    P::ShRc => f("S.sh_rc"),
                    P::ShOut => f("S.sh_out"),
                    P::LNamed => {
                        let mi = match &a[0] {
                            Ex::Num(k) => *k as usize,
                            _ => 0,
                        };
                        let mut m = format!("S.ln{}({})", mi, x[1]);
                        let mut i = 2;
                        while i + 1 < x.len() {
                            m = format!("S.pickl(String.eq({}, {}), {}, {})", x[1], x[i], x[i + 1], m);
                            i += 2;
                        }
                        m
                    }
                }
            }
        }
    }

    fn params(&self, vs: &[u32]) -> String {
        vs.iter().map(|v| format!("+v{}: {}", v, self.tys[*v as usize].bend())).collect::<Vec<_>>().join(", ")
    }
    fn args_of(&self, vs: &[u32]) -> String {
        vs.iter().map(|v| format!("v{}", v)).collect::<Vec<_>>().join(", ")
    }

    /// A call written as an expression, and what it binds.
    fn call_expr(&mut self, s: &Stmt) -> (String, Vec<(u32, Ty)>, bool) {
        match s {
            Stmt::Call { f, args, outs } => {
                let a: Vec<String> = self.kept(*f, args);
                (format!("f{}({})", f, a.join(", ")), outs.clone(), self.io[*f])
            }
            Stmt::Branch { c, t, e, outs } => {
                let io = self.block_io(t) || self.block_io(e);
                let mut live: Vec<u32> = free(&[], &End::Yield(vec![]));
                let mut used = BTreeSet::new();
                let mut def = BTreeSet::new();
                block_vars(t, &mut used, &mut def);
                block_vars(e, &mut used, &mut def);
                live.extend(used.difference(&def).cloned());
                let name = self.fresh("h");
                let tys: Vec<Ty> = outs.iter().map(|o| o.1).collect();
                let rt = self.out_ty(&tys);
                let mut d = String::new();
                let _ = writeln!(d, "@unsafe def {}(c: Bool{}{}) -> {}:", name, if live.is_empty() { "" } else { ", " }, self.params(&live), if io { format!("IO({})", rt) } else { rt.clone() });
                d.push_str("  match c:\n    case True{}:\n");
                let tb = self.body(t, &tys, io, 6);
                d.push_str(&tb);
                d.push_str("    case False{}:\n");
                let eb = self.body(e, &tys, io, 6);
                d.push_str(&eb);
                self.defs.push(d);
                let call = format!("{}({}{}{})", name, self.ex(c), if live.is_empty() { "" } else { ", " }, self.args_of(&live));
                (call, outs.clone(), io)
            }
            Stmt::Map { f, xs, args, outs } => {
                let io = self.io[*f];
                let name = format!("M{}", f);
                if !self.maps.contains(f) {
                    self.maps.push(*f);
                    let fun = self.funs[*f].unwrap();
                    let ps: Vec<u32> = fun.params[..fun.params.len() - 1].iter().enumerate().filter(|(j, _)| self.used[*f][*j]).map(|(_, p)| p.0).collect();
                    let k = fun.outs.len();
                    let lt = vec![Ty::L; k];
                    let rt = self.out_ty(&lt);
                    let xt = self.out_ty(&fun.outs);
                    let pl = self.params(&ps);
                    let pa = self.args_of(&ps);
                    let sep = if ps.is_empty() { "" } else { ", " };
                    let empty: Vec<String> = (0..k).map(|_| "S.lnew()".to_string()).collect();
                    let empty = self.tuple(&lt, &empty);
                    let mut d = String::new();
                    let _ = writeln!(d, "@unsafe def {}(xs: List<&2, V>{}{}) -> {}:", name, sep, pl, if io { format!("IO({})", rt) } else { rt.clone() });
                    d.push_str("  match xs:\n    case Nil{}:\n");
                    if io {
                        let _ = writeln!(d, "      do IO<{}>:\n        return {}", rt, empty);
                    } else {
                        let _ = writeln!(d, "      {}", empty);
                    }
                    d.push_str("    case Con{+h, t}:\n");
                    let join = match k {
                        0 => "Unit{}".to_string(),
                        _ => format!("{}.a(r, x)", name),
                    };
                    if io {
                        let _ = writeln!(d, "      do IO<{}>:", rt);
                        let _ = writeln!(d, "        r : {} <- {}(t{}{})", rt, name, sep, pa);
                        let _ = writeln!(d, "        x : {} <- f{}({}{}h)", xt, f, pa, sep);
                        let _ = writeln!(d, "        return {}", join);
                    } else if k == 0 {
                        let _ = writeln!(d, "      Unit{{}}");
                    } else {
                        let _ = writeln!(d, "      {}.a({}(t{}{}), f{}({}{}h))", name, name, sep, pa, f, pa, sep);
                    }
                    if k == 1 {
                        let _ = writeln!(d, "\n@unsafe def {}.a(r: SL, x: V) -> SL:\n  S.lpush(r, x)", name);
                    } else if k > 1 {
                        let rs: Vec<String> = (0..k).map(|i| format!("+a{}", i)).collect();
                        let ra: Vec<String> = (0..k).map(|i| format!("a{}", i)).collect();
                        let rp: Vec<String> = (0..k).map(|i| format!("+a{}: SL", i)).collect();
                        let xs: Vec<String> = (0..k).map(|i| format!("+b{}", i)).collect();
                        let pushed: Vec<String> = (0..k).map(|i| format!("S.lpush(a{}, b{})", i, i)).collect();
                        let pushed = self.tuple(&lt, &pushed);
                        let _ = writeln!(d, "\n@unsafe def {}.a(r: {}, x: {}) -> {}:\n  {}{{{}}} = r\n  {}.b(x, {})", name, rt, xt, rt, rt, rs.join(", "), name, ra.join(", "));
                        let _ = writeln!(d, "\n@unsafe def {}.b(x: {}, {}) -> {}:\n  {}{{{}}} = x\n  {}", name, xt, rp.join(", "), rt, xt, xs.join(", "), pushed);
                    }
                    if !io && k >= 1 {
                        // the list cut once into runs of PAR_RUN held in a balanced tree; the two
                        // halves of each node map as a parallel call; each output joined in one pass
                        let _ = writeln!(d, "\ntype {n}T is Data:\n  {n}L{{r: {rt}}}\n  {n}N{{a: {n}T, b: {n}T}}", n = name, rt = rt);
                        let _ = writeln!(d, "\n@unsafe def {n}.tp(t: ST{sep}{pl}) -> {n}T:\n  match t:\n    case SLeaf{{+c, xs}}:\n      {n}L{{{n}(xs{sep}{pa})}}\n    case SNode{{a, b}}:\n      ra rb = {n}.tp(a{sep}{pa}) {n}.tp(b{sep}{pa})\n      {n}N{{ra, rb}}", n = name, sep = sep, pl = pl, pa = pa);
                        for o in 0..k {
                            let field = if k == 1 { "r".to_string() } else { format!("{}.g{}(r)", name, o) };
                            let _ = writeln!(d, "\n@unsafe def {n}.f{o}(t: {n}T, acc: SL) -> SL:\n  match t:\n    case {n}L{{+r}}:\n      S.lcat({f}, acc)\n    case {n}N{{a, b}}:\n      {n}.f{o}(a, {n}.f{o}(b, acc))", n = name, o = o, f = field);
                            if k > 1 {
                                let fs: Vec<String> = (0..k).map(|i| if i == o { "+x".to_string() } else { "_".to_string() }).collect();
                                let _ = writeln!(d, "\n@unsafe def {n}.g{o}(r: {rt}) -> SL:\n  {rt}{{{fs}}} = r\n  x", n = name, o = o, rt = rt, fs = fs.join(", "));
                            }
                        }
                        let alls: Vec<String> = (0..k).map(|o| format!("{}.f{}(t, S.lnew())", name, o)).collect();
                        let alls = self.tuple(&lt, &alls);
                        let _ = writeln!(d, "\n@unsafe def {n}.all(+t: {n}T) -> {rt}:\n  {a}", n = name, rt = rt, a = alls);
                    }
                    self.defs.push(d);
                }
                let a: Vec<String> = self.kept(*f, args);
                let k = self.funs[*f].unwrap().outs.len();
                let call = if !io && k >= 1 {
                    format!("{}.all({}.tp(S.tree(S.chunks(S.items({}))){}{}))", name, name, self.ex(xs), if a.is_empty() { "" } else { ", " }, a.join(", "))
                } else {
                    format!("{}(S.items({}){}{})", name, self.ex(xs), if a.is_empty() { "" } else { ", " }, a.join(", "))
                };
                (call, outs.clone(), io)
            }
            _ => unreachable!(),
        }
    }

    /// A block as a body at the given indent: pure, or an IO block.
    fn body(&mut self, b: &Block, outs: &[Ty], io: bool, ind: usize) -> String {
        if io {
            self.io_body(&b.stmts, &b.end, outs, ind)
        } else {
            self.pure_body(b, outs, ind)
        }
    }

    fn tail_of(stmts: &[Stmt], end: &End) -> bool {
        let last = match stmts.last() {
            Some(s @ (Stmt::Call { .. } | Stmt::Branch { .. })) => s,
            _ => return false,
        };
        let outs = match last {
            Stmt::Call { outs, .. } | Stmt::Branch { outs, .. } => outs,
            _ => unreachable!(),
        };
        match end {
            End::Yield(xs) => xs.len() == outs.len() && xs.iter().zip(outs.iter()).all(|(x, o)| matches!(x, Ex::Var(v) if *v == o.0)),
            End::Never => true,
            _ => false,
        }
    }

    fn pure_body(&mut self, b: &Block, outs: &[Ty], ind: usize) -> String {
        self.pure_stmts(&b.stmts, &b.end, outs, ind)
    }
    /// Pure statements: lets, and calls; a call that gives back several values continues in a
    /// new definition that takes them apart, since Bend matches only on parameters.
    fn pure_stmts(&mut self, stmts: &[Stmt], end: &End, outs: &[Ty], ind: usize) -> String {
        let pad = " ".repeat(ind);
        let mut o = String::new();
        let n = stmts.len();
        let tail = Emit::tail_of(stmts, end);
        for (i, s) in stmts.iter().enumerate() {
            match s {
                Stmt::Let(v, t, e) => {
                    let _ = writeln!(o, "{}+v{} = {{{} : {}}}", pad, v, self.ex(e), t.bend());
                }
                Stmt::Call { .. } | Stmt::Branch { .. } | Stmt::Par { .. } | Stmt::Map { .. } => {
                    let (call, bs, _) = match s {
                        Stmt::Par { calls } if calls.len() == 1 => {
                            let (f, args, outs) = &calls[0];
                            self.call_expr(&Stmt::Call { f: *f, args: args.clone(), outs: outs.clone() })
                        }
                        Stmt::Par { .. } => unreachable!(),
                        _ => self.call_expr(s),
                    };
                    if i == n - 1 && tail {
                        let _ = writeln!(o, "{}{}", pad, call);
                        return o;
                    }
                    match bs.len() {
                        0 => {}
                        1 => {
                            let _ = writeln!(o, "{}+v{} = {}", pad, bs[0].0, call);
                        }
                        _ => {
                            let tys: Vec<Ty> = bs.iter().map(|x| x.1).collect();
                            let r = self.rec(&tys);
                            let rest = &stmts[i + 1..];
                            let live: Vec<u32> = free(rest, end).into_iter().filter(|v| !bs.iter().any(|b| b.0 == *v)).collect();
                            let k = self.fresh("k");
                            let rt = self.out_ty(outs);
                            let _ = writeln!(o, "{}{}({}{}{})", pad, k, call, if live.is_empty() { "" } else { ", " }, self.args_of(&live));
                            let mut d = String::new();
                            let _ = writeln!(d, "@unsafe def {}(p: {}{}{}) -> {}:", k, r, if live.is_empty() { "" } else { ", " }, self.params(&live), rt);
                            let names: Vec<String> = bs.iter().map(|x| format!("+v{}", x.0)).collect();
                            let _ = writeln!(d, "  {}{{{}}} = p", r, names.join(", "));
                            let rest_body = self.pure_stmts(rest, end, outs, 2);
                            d.push_str(&rest_body);
                            self.defs.push(d);
                            return o;
                        }
                    }
                }
                _ => unreachable!("an effect in a pure body"),
            }
        }
        match end {
            End::Yield(xs) => {
                let es: Vec<String> = xs.iter().map(|e| self.ex(e)).collect();
                let t = self.tuple(outs, &es);
                let _ = writeln!(o, "{}{}", pad, t);
            }
            _ => unreachable!("an end of the program in a pure body"),
        }
        o
    }

    /// Statements as a do block; a call that gives back several values continues in a new
    /// definition that takes them apart.
    fn io_body(&mut self, stmts: &[Stmt], end: &End, outs: &[Ty], ind: usize) -> String {
        let pad = " ".repeat(ind);
        let pad2 = " ".repeat(ind + 2);
        let rt = self.out_ty(outs);
        let mut o = format!("{}do IO<{}>:\n", pad, rt);
        let n = stmts.len();
        let tail = Emit::tail_of(stmts, end);
        for (i, s) in stmts.iter().enumerate() {
            match s {
                Stmt::Spawn { .. } | Stmt::Join { .. } => unreachable!("workers are written in place for Bend"),
                Stmt::Let(v, t, e) => {
                    let _ = writeln!(o, "{}+v{} : {} = {}", pad2, v, t.bend(), self.ex(e));
                }
                Stmt::Write(e) => {
                    let _ = writeln!(o, "{}IO.write({})", pad2, self.ex(e));
                }
                Stmt::Fx(op, a, b, v) => {
                    let _ = writeln!(o, "{}+v{} : String <- S.fx(\"{}\", {}, {})", pad2, v, op, self.ex(a), self.ex(b));
                }
                Stmt::ReadLine(a, b) => {
                    let _ = writeln!(o, "{}+r{} : String <- S.read_line()", pad2, a);
                    let _ = writeln!(o, "{}+v{} : V = S.rl_val(r{})", pad2, a, a);
                    let _ = writeln!(o, "{}+v{} : F32 = S.rl_eof(r{})", pad2, b, a);
                }
                Stmt::Args(a, b) => {
                    let _ = writeln!(o, "{}r{} : List<String> <- IO.args()", pad2, a);
                    let _ = writeln!(o, "{}+v{} : SL = S.args(r{})", pad2, a, a);
                    let _ = writeln!(o, "{}+v{} : F32 = S.llen(v{})", pad2, b, a);
                }
                Stmt::Call { .. } | Stmt::Branch { .. } | Stmt::Par { .. } | Stmt::Map { .. } => {
                    let (call, bs, cio) = match s {
                        Stmt::Par { calls } if calls.len() == 1 => {
                            let (f, args, outs) = &calls[0];
                            self.call_expr(&Stmt::Call { f: *f, args: args.clone(), outs: outs.clone() })
                        }
                        Stmt::Par { .. } => unreachable!(),
                        _ => self.call_expr(s),
                    };
                    let tys: Vec<Ty> = bs.iter().map(|x| x.1).collect();
                    if i == n - 1 && tail {
                        let callee_rt = self.out_ty(&tys);
                        if matches!(end, End::Never) && callee_rt != rt {
                            if cio {
                                let _ = writeln!(o, "{}{}", pad2, call);
                            }
                            let _ = writeln!(o, "{}IO.die({}, 0, \"\")", pad2, rt);
                        } else if cio {
                            let _ = writeln!(o, "{}{}", pad2, call);
                        } else {
                            let _ = writeln!(o, "{}return {}", pad2, call);
                        }
                        return o;
                    }
                    let bind = if cio { "<-" } else { "=" };
                    match bs.len() {
                        0 => {
                            if cio {
                                let _ = writeln!(o, "{}{}", pad2, call);
                            }
                        }
                        1 => {
                            let _ = writeln!(o, "{}+v{} : {} {} {}", pad2, bs[0].0, tys[0].bend(), bind, call);
                        }
                        _ => {
                            let r = self.rec(&tys);
                            let p = self.fresh("p");
                            let rest = &stmts[i + 1..];
                            let live: Vec<u32> = free(rest, end).into_iter().filter(|v| !bs.iter().any(|b| b.0 == *v)).collect();
                            let k = self.fresh("k");
                            let _ = writeln!(o, "{}{} : {} {} {}", pad2, p, r, bind, call);
                            let _ = writeln!(o, "{}{}({}{}{})", pad2, k, p, if live.is_empty() { "" } else { ", " }, self.args_of(&live));
                            let mut d = String::new();
                            let _ = writeln!(d, "@unsafe def {}(p: {}{}{}) -> IO({}):", k, r, if live.is_empty() { "" } else { ", " }, self.params(&live), rt);
                            let names: Vec<String> = bs.iter().map(|x| format!("+v{}", x.0)).collect();
                            let _ = writeln!(d, "  {}{{{}}} = p", r, names.join(", "));
                            let rest_body = self.io_body(rest, end, outs, 2);
                            d.push_str(&rest_body);
                            self.defs.push(d);
                            return o;
                        }
                    }
                }
            }
        }
        match end {
            End::Yield(xs) => {
                let es: Vec<String> = xs.iter().map(|e| self.ex(e)).collect();
                let t = self.tuple(outs, &es);
                let _ = writeln!(o, "{}return {}", pad2, t);
            }
            End::Exit(e) => {
                let _ = writeln!(o, "{}IO.exit(F32.to_u32({}))", pad2, self.ex(e));
                let _ = writeln!(o, "{}IO.die({}, 0, \"\")", pad2, rt);
            }
            End::Fail(e) => {
                let _ = writeln!(o, "{}IO.die({}, 1, (\"\\ns: \" ++ {}))", pad2, rt, self.ex(e));
            }
            End::Never => {
                let _ = writeln!(o, "{}IO.die({}, 0, \"\")", pad2, rt);
            }
        }
        o
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
        self.uses();
        let mut fns = String::new();
        for i in 0..self.funs.len() {
            if let Some(f) = self.funs[i] {
                let io = self.io[i];
                let rt = self.out_ty(&f.outs);
                let params: Vec<u32> = f.params.iter().enumerate().filter(|(j, _)| self.used[i][*j]).map(|(_, p)| p.0).collect();
                let mut d = String::new();
                let _ = writeln!(d, "# {}", f.what.replace('\n', " "));
                let _ = writeln!(d, "@unsafe def f{}({}) -> {}:", i, self.params(&params), if io { format!("IO({})", rt) } else { rt });
                let b = self.body(&f.body, &f.outs, io, 2);
                d.push_str(&b);
                fns.push_str(&d);
                fns.push('\n');
            }
        }
        let mb = self.io_body(&main.stmts, &main.end, &[], 2);
        let mut o = String::new();
        let _ = writeln!(o, "# generated by s3 from {}: the residue of the machine's run at build time\n", title);
        let cmds: Vec<String> = crate::rt::CMD_WORDS.iter().map(|w| lit_str(w)).collect();
        o.push_str(&PRELUDE.replace("CMDS", &format!("[{}]", cmds.join(", "))).replace("PARRUN", &format!("{}.0", PAR_RUN)));
        o.push('\n');
        for (i, r) in self.recs.iter().enumerate() {
            let _ = writeln!(o, "type O{} is Data:", i);
            let fs: Vec<String> = r.iter().enumerate().map(|(j, t)| format!("x{}: {}", j, t.bend())).collect();
            let _ = writeln!(o, "  O{}{{{}}}\n", i, fs.join(", "));
        }
        for (i, ks) in self.keysets.iter().enumerate() {
            let mut keys: Vec<(String, String)> = ks.iter().map(|k| (k.clone(), "True{}".to_string())).collect();
            for w in crate::rt::CMD_WORDS {
                keys.push((w.to_string(), "True{}".to_string()));
            }
            o.push_str(&search(&format!("S.ks{}", i), keys, "False{}", "Bool"));
            let _ = writeln!(o, "@unsafe def S.runs{}(+t: String) -> Bool:\n  Bool.and(S.oneword(t), Bool.or(S.active(t), S.ks{}(t)))\n", i, i);
        }
        for (i, m) in self.lmaps.iter().enumerate() {
            let keys: Vec<(String, String)> = m.iter().map(|(n, l)| (n.clone(), self.ex(l))).collect();
            o.push_str(&search(&format!("S.ln{}", i), keys, "S.lnew()", "SL"));
        }
        o.push_str(&fns);
        for d in &self.defs {
            o.push_str(d);
            o.push('\n');
        }
        o.push_str("@unsafe def main() -> IO(Unit):\n");
        o.push_str(&mb);
        o
    }
}
