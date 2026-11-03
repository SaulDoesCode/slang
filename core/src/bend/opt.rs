// The residue folded before it is written: a function keeps the parameters it reads and the
// outputs some caller reads; statements whose values nothing reads go; a V parameter read only
// as a number or only as text is passed as one. Both writers, Bend and Rust, take what is left.
use super::ir::*;
use std::collections::{BTreeSet, HashMap};

type Live = BTreeSet<u32>;

struct Opt<'a> {
    funs: &'a [Option<Fun>],
    io: Vec<bool>,
    lanes: Vec<bool>,
    used: Vec<Vec<bool>>,
    need: Vec<Vec<bool>>,
    pk: HashMap<u32, Ty>,
    changed: bool,
}

fn vars(e: &Ex, live: &mut Live) {
    let mut v = Vec::new();
    e.vars(&mut v);
    live.extend(v);
}

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

impl<'a> Opt<'a> {
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

    /// V parameters read only as numbers, or only as text: every read is that kind, or passes
    /// the value straight on to a parameter of the same kind.
    fn kinds(&mut self, main: &Block) {
        fn reads(e: &Ex, out: &mut Vec<(u32, Option<Ty>)>) {
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
                        reads(x, out)
                    }
                }
                _ => {}
            }
        }
        fn walk(b: &Block, out: &mut Vec<(u32, Option<Ty>)>, pass: &mut Vec<(u32, u32)>, funs: &[Option<Fun>]) {
            let args = |f: usize, a: &[Ex], out: &mut Vec<(u32, Option<Ty>)>, pass: &mut Vec<(u32, u32)>| {
                let ps = &funs[f].as_ref().unwrap().params;
                for (j, x) in a.iter().enumerate() {
                    match x {
                        Ex::Var(v) if j < ps.len() => pass.push((*v, ps[j].0)),
                        _ => reads(x, out),
                    }
                }
            };
            for s in &b.stmts {
                match s {
                    Stmt::Spawn { body, .. } => walk(body, out, pass, funs),
                    Stmt::Join { .. } => {}
                    Stmt::Let(_, _, e) | Stmt::Write(e) => reads(e, out),
                    Stmt::Fx(_, a, b, _) => {
                        reads(a, out);
                        reads(b, out)
                    }
                    Stmt::ReadLine(..) | Stmt::Args(..) => {}
                    Stmt::Call { f, args: a, .. } => args(*f, a, out, pass),
                    Stmt::Map { f, xs, args: a, .. } => {
                        reads(xs, out);
                        args(*f, a, out, pass)
                    }
                    Stmt::Par { calls } => {
                        for (f, a, _) in calls {
                            args(*f, a, out, pass)
                        }
                    }
                    Stmt::Branch { c, t, e, .. } => {
                        reads(c, out);
                        walk(t, out, pass, funs);
                        walk(e, out, pass, funs);
                    }
                }
            }
            match &b.end {
                End::Yield(xs) => {
                    for x in xs {
                        reads(x, out)
                    }
                }
                End::Exit(x) | End::Fail(x) => reads(x, out),
                End::Never => {}
            }
        }
        let mut cand: BTreeSet<u32> = BTreeSet::new();
        for f in self.funs.iter().flatten() {
            for p in &f.params {
                if p.1 == Ty::V {
                    cand.insert(p.0);
                }
            }
        }
        let mut rs = Vec::new();
        let mut pass = Vec::new();
        for f in self.funs.iter().flatten() {
            walk(&f.body, &mut rs, &mut pass, self.funs);
        }
        walk(main, &mut rs, &mut pass, self.funs);
        let mut first: HashMap<u32, Ty> = HashMap::new();
        let mut bad: BTreeSet<u32> = BTreeSet::new();
        for (v, k) in &rs {
            if !cand.contains(v) {
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
        for (i, f) in self.funs.iter().enumerate() {
            if let (true, Some(f)) = (self.lanes[i], f) {
                if let Some(p) = f.params.last() {
                    bad.insert(p.0);
                }
            }
        }
        loop {
            let mut ch = false;
            for (v, p) in &pass {
                if !cand.contains(v) || bad.contains(v) {
                    continue;
                }
                let target = if !cand.contains(p) || bad.contains(p) { None } else { Some(first.get(p).copied().unwrap_or(Ty::F)) };
                let ok = match (target, first.get(v).copied()) {
                    (None, _) => false,
                    (Some(k), Some(k0)) => k == k0,
                    (Some(k), None) => {
                        first.insert(*v, k);
                        ch = true;
                        true
                    }
                };
                if !ok {
                    bad.insert(*v);
                    ch = true;
                }
            }
            if !ch {
                break;
            }
        }
        self.pk = cand.into_iter().filter(|v| !bad.contains(v)).map(|v| (v, first.get(&v).copied().unwrap_or(Ty::F))).collect();
    }

    fn ex(&self, e: &Ex) -> Ex {
        match e {
            Ex::App(P::VNum, a) if matches!(&a[0], Ex::Var(v) if self.pk.get(v) == Some(&Ty::F)) => a[0].clone(),
            Ex::App(P::VText, a) if matches!(&a[0], Ex::Var(v) if self.pk.get(v) == Some(&Ty::S)) => a[0].clone(),
            Ex::App(p, a) => Ex::App(*p, a.iter().map(|x| self.ex(x)).collect()),
            _ => e.clone(),
        }
    }
    /// An argument for parameter j of f, unboxed when the parameter is.
    fn arg(&self, f: usize, j: usize, a: &Ex) -> Ex {
        let p = self.funs[f].as_ref().unwrap().params[j].0;
        let a = self.ex(a);
        let unboxed = |a: &Ex, k: Ty| matches!(a, Ex::Var(q) if self.pk.get(q) == Some(&k));
        match self.pk.get(&p).copied() {
            Some(Ty::F) => match &a {
                Ex::App(P::VN, x) => x[0].clone(),
                _ if unboxed(&a, Ty::F) => a,
                _ => Ex::App(P::VNum, vec![a]),
            },
            Some(Ty::S) => match &a {
                Ex::App(P::VT, x) | Ex::App(P::VOf, x) => x[0].clone(),
                Ex::App(P::VN, x) => Ex::App(P::Fmt, vec![x[0].clone()]),
                _ if unboxed(&a, Ty::S) => a,
                _ => Ex::App(P::VText, vec![a]),
            },
            _ => a,
        }
    }

    fn mark(&mut self, f: usize, outs: &[(u32, Ty)], live: &Live) -> bool {
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
        any
    }
    fn args(&self, f: usize, args: &[Ex], upto: usize, live: &mut Live) -> Vec<Ex> {
        let mut out = Vec::new();
        for (j, a) in args.iter().enumerate().take(upto) {
            if self.used[f][j] {
                vars(a, live);
                out.push(self.arg(f, j, a));
            }
        }
        out
    }
    fn outs(&self, f: usize, outs: &[(u32, Ty)]) -> Vec<(u32, Ty)> {
        outs.iter().enumerate().filter(|(j, _)| self.need[f][*j]).map(|(_, o)| *o).collect()
    }

    /// A block read backward from what is read after it; yn says which values it gives back
    /// are read.
    fn block(&mut self, b: &Block, yn: &[bool], live: &mut Live) -> Block {
        let end = match &b.end {
            End::Yield(xs) => {
                let mut keep = Vec::new();
                for (j, x) in xs.iter().enumerate() {
                    if yn.get(j).copied().unwrap_or(false) {
                        vars(x, live);
                        keep.push(self.ex(x));
                    }
                }
                End::Yield(keep)
            }
            End::Exit(x) => {
                vars(x, live);
                End::Exit(self.ex(x))
            }
            End::Fail(x) => {
                vars(x, live);
                End::Fail(self.ex(x))
            }
            End::Never => End::Never,
        };
        let mut stmts = Vec::new();
        for s in b.stmts.iter().rev() {
            if let Some(s) = self.stmt(s, live) {
                stmts.push(s);
            }
        }
        stmts.reverse();
        Block { stmts, end }
    }

    fn stmt(&mut self, s: &Stmt, live: &mut Live) -> Option<Stmt> {
        Some(match s {
            Stmt::Join { outs, .. } => {
                for o in outs {
                    live.remove(&o.0);
                }
                s.clone()
            }
            Stmt::Spawn { id, body } => {
                let n = match &body.end {
                    End::Yield(xs) => xs.len(),
                    _ => 0,
                };
                let mut lb = Live::new();
                let b = self.block(body, &vec![true; n], &mut lb);
                live.extend(lb);
                Stmt::Spawn { id: *id, body: b }
            }
            Stmt::Let(v, t, e) => {
                if !live.remove(v) {
                    return None;
                }
                vars(e, live);
                Stmt::Let(*v, *t, self.ex(e))
            }
            Stmt::Write(e) => {
                vars(e, live);
                Stmt::Write(self.ex(e))
            }
            Stmt::ReadLine(a, b) => {
                live.remove(a);
                live.remove(b);
                s.clone()
            }
            Stmt::Args(a, b) => {
                live.remove(a);
                live.remove(b);
                s.clone()
            }
            Stmt::Fx(op, a, b, o) => {
                live.remove(o);
                vars(a, live);
                vars(b, live);
                Stmt::Fx(op, self.ex(a), self.ex(b), *o)
            }
            Stmt::Call { f, args, outs } => {
                let any = self.mark(*f, outs, live);
                if !any && !self.io[*f] {
                    return None;
                }
                let o = self.outs(*f, outs);
                for x in outs {
                    live.remove(&x.0);
                }
                let a = self.args(*f, args, args.len(), live);
                Stmt::Call { f: *f, args: a, outs: o }
            }
            Stmt::Par { calls } => {
                let mut cs = Vec::new();
                for (f, args, outs) in calls.iter().rev() {
                    if let Some(Stmt::Call { f, args, outs }) = self.stmt(&Stmt::Call { f: *f, args: args.clone(), outs: outs.clone() }, live) {
                        cs.push((f, args, outs));
                    }
                }
                if cs.is_empty() {
                    return None;
                }
                cs.reverse();
                Stmt::Par { calls: cs }
            }
            Stmt::Branch { c, t, e, outs } => {
                let pos: Vec<bool> = outs.iter().map(|o| live.contains(&o.0)).collect();
                if !pos.iter().any(|x| *x) && !self.block_io(t) && !self.block_io(e) {
                    return None;
                }
                for x in outs {
                    live.remove(&x.0);
                }
                let after = live.clone();
                let mut lt = after.clone();
                let tb = self.block(t, &pos, &mut lt);
                let mut le = after;
                let eb = self.block(e, &pos, &mut le);
                live.extend(lt);
                live.extend(le);
                vars(c, live);
                let o: Vec<(u32, Ty)> = outs.iter().zip(pos.iter()).filter(|(_, p)| **p).map(|(o, _)| *o).collect();
                Stmt::Branch { c: self.ex(c), t: tb, e: eb, outs: o }
            }
            Stmt::Map { f, xs, args, outs } => {
                let any = self.mark(*f, outs, live);
                if !any && !self.io[*f] {
                    return None;
                }
                let o = self.outs(*f, outs);
                for x in outs {
                    live.remove(&x.0);
                }
                let np = self.funs[*f].as_ref().unwrap().params.len();
                let a = self.args(*f, args, np - 1, live);
                vars(xs, live);
                Stmt::Map { f: *f, xs: self.ex(xs), args: a, outs: o }
            }
        })
    }

    fn pass(&mut self, main: &Block) -> (Vec<Option<Block>>, Block) {
        let mut bodies = Vec::new();
        for i in 0..self.funs.len() {
            match &self.funs[i] {
                Some(f) => {
                    let mut live = Live::new();
                    let yn = self.need[i].clone();
                    let b = self.block(&f.body, &yn, &mut live);
                    let np = f.params.len();
                    for (j, p) in f.params.iter().enumerate() {
                        let u = live.contains(&p.0) || (self.lanes[i] && j + 1 == np);
                        if u && !self.used[i][j] {
                            self.used[i][j] = true;
                            self.changed = true;
                        }
                    }
                    bodies.push(Some(b));
                }
                None => bodies.push(None),
            }
        }
        let mut live = Live::new();
        let m = self.block(main, &[], &mut live);
        (bodies, m)
    }
}

/// Folds the residue in place: functions, main, and the types of the parameters it unboxes.
pub fn run(funs: &mut Vec<Option<Fun>>, main: &mut Block, tys: &mut Vec<Ty>) {
    let n = funs.len();
    let mut ms = Vec::new();
    maps_in(main, &mut ms);
    for f in funs.iter().flatten() {
        maps_in(&f.body, &mut ms);
    }
    let lanes: Vec<bool> = (0..n).map(|i| ms.contains(&i)).collect();
    let (bodies, m, used, need, pk) = {
        let fr: &[Option<Fun>] = &funs[..];
        let mut o = Opt {
            funs: fr,
            io: vec![false; n],
            lanes,
            used: fr.iter().map(|f| vec![false; f.as_ref().map_or(0, |f| f.params.len())]).collect(),
            need: fr.iter().map(|f| vec![false; f.as_ref().map_or(0, |f| f.outs.len())]).collect(),
            pk: HashMap::new(),
            changed: false,
        };
        loop {
            let mut ch = false;
            for i in 0..n {
                if let Some(f) = &o.funs[i] {
                    if !o.io[i] && o.block_io(&f.body) {
                        o.io[i] = true;
                        ch = true;
                    }
                }
            }
            if !ch {
                break;
            }
        }
        o.kinds(main);
        let r = loop {
            o.changed = false;
            let r = o.pass(main);
            if !o.changed {
                break r;
            }
        };
        (r.0, r.1, o.used, o.need, o.pk)
    };
    for (v, t) in &pk {
        tys[*v as usize] = *t;
    }
    for (i, (f, b)) in funs.iter_mut().zip(bodies).enumerate() {
        if let (Some(f), Some(b)) = (f.as_mut(), b) {
            f.body = b;
            f.params = f.params.iter().enumerate().filter(|(j, _)| used[i][*j]).map(|(_, p)| (p.0, pk.get(&p.0).copied().unwrap_or(p.1))).collect();
            f.outs = f.outs.iter().enumerate().filter(|(j, _)| need[i][*j]).map(|(_, t)| *t).collect();
        }
    }
    *main = m;
}
