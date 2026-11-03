// The core thread: owns every world, its history and its running session. Requests come in
// as JSON and go back as JSON; nothing here is shared with the web threads.
use s3_core::build::build;
use s3_core::compile::compile_spec;
use s3_core::json::J;
use s3_core::ops::{apply_op, import_text};
use s3_core::rt::Program;
use s3_core::session::{Session, StepKind};
use s3_core::world::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

pub static INTERP: Program = Program::with_lib(s3_core::LIB);

pub type Req = (J, Sender<J>);

struct Doc {
    world: World,
    undo: Vec<Vec<Edit>>,
    redo: Vec<Vec<Edit>>,
    path: PathBuf,
    session: Option<Session>,
    /// The console's own machine when no session runs, with its output.
    repl: Option<(s3_core::rt::Rt, Arc<Mutex<Vec<u8>>>)>,
}

#[derive(Clone, Default)]
struct Job {
    done: bool,
    ok: bool,
    log: String,
    out: String,
}

pub struct Core {
    dir: PathBuf,
    levels: PathBuf,
    docs: HashMap<String, Doc>,
    jobs: HashMap<u64, Arc<Mutex<Job>>>,
    next_job: u64,
}

fn err(m: impl Into<String>) -> J {
    J::obj().with("error", J::Str(m.into()))
}

fn clean_name(n: &str) -> Option<String> {
    let n = n.trim();
    if n.is_empty() || n.len() > 64 || !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        None
    } else {
        Some(n.to_string())
    }
}

fn world_names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map_or(false, |x| x == "json"))
                .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

impl Core {
    pub fn new(dir: PathBuf, levels: PathBuf) -> Core {
        let _ = std::fs::create_dir_all(&dir);
        Core { dir, levels, docs: HashMap::new(), jobs: HashMap::new(), next_job: 1 }
    }

    pub fn serve(mut self, rx: Receiver<Req>) {
        while let Ok((req, tx)) = rx.recv() {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.handle(&req)));
            let out = match r {
                Ok(j) => j,
                Err(_) => err("internal error; the world on disk is unchanged since the last edit"),
            };
            let _ = tx.send(out);
        }
    }

    fn doc(&mut self, req: &J) -> Result<&mut Doc, J> {
        let name = clean_name(&req.get("name").s("")).ok_or_else(|| err("a world name is letters, digits, _ and -"))?;
        if !self.docs.contains_key(&name) {
            let path = self.dir.join(format!("{}.json", name));
            let world = match std::fs::read_to_string(&path) {
                Ok(t) => {
                    let j = J::parse(&t).map_err(|e| err(format!("{}: {}", path.display(), e)))?;
                    World::from_json(&j).map_err(|e| err(format!("{}: {}", path.display(), e)))?
                }
                Err(_) => {
                    let w = World::starter(&name);
                    let _ = std::fs::write(&path, w.to_json().pretty());
                    w
                }
            };
            self.docs.insert(name.clone(), Doc { world, undo: Vec::new(), redo: Vec::new(), path, session: None, repl: None });
        }
        Ok(self.docs.get_mut(&name).unwrap())
    }

    /// A running session takes the edit live, or ends with the reason it cannot.
    fn live(d: &mut Doc, ch: J) -> J {
        let Some(s) = d.session.as_mut() else { return ch };
        let r = match build(&d.world) {
            Ok(spec) => s.patch(spec, s3_core::build::defs(&d.world)),
            Err(ps) => Err(ps.first().map(|p| p.msg.clone()).unwrap_or_else(|| "the world does not build".into())),
        };
        match r {
            Ok(()) => ch.with("live", true.into()),
            Err(e) => {
                d.session = None;
                ch.with("stopped", format!("an edit ended the run: {}", e).into())
            }
        }
    }

    fn save(d: &Doc) {
        let tmp = d.path.with_extension("json.tmp");
        if std::fs::write(&tmp, d.world.to_json().pretty()).is_ok() {
            let _ = std::fs::rename(&tmp, &d.path);
        }
    }

    pub fn handle(&mut self, req: &J) -> J {
        let cmd = req.get("cmd").s("");
        match self.handle_cmd(&cmd, req) {
            Ok(j) => j,
            Err(e) => e,
        }
    }

    fn handle_cmd(&mut self, cmd: &str, req: &J) -> Result<J, J> {
        Ok(match cmd {
            "list" => {
                let levels: Vec<J> = world_names(&self.levels)
                    .into_iter()
                    .map(|n| {
                        let t = std::fs::read_to_string(self.levels.join(format!("{}.json", n))).unwrap_or_default();
                        let j = J::parse(&t).unwrap_or(J::Null);
                        J::obj().with("name", n.into()).with("title", j.get("goal").get("title").clone()).with("order", j.get("goal").get("order").clone())
                    })
                    .collect();
                J::obj().with("worlds", world_names(&self.dir).into()).with("levels", J::Arr(levels))
            }
            "palette" => s3_core::docs::palette(),
            "lex" => s3_core::query::lex(&req.get("text").s("")),
            "typed" => s3_core::query::typed(&req.get("word").s("")),
            "open" => {
                if let Some(level) = req.get("level").str() {
                    let lv = clean_name(level).ok_or_else(|| err("bad level name"))?;
                    let name = clean_name(&req.get("name").s("")).ok_or_else(|| err("bad world name"))?;
                    let path = self.dir.join(format!("{}.json", name));
                    if !path.exists() || req.get("reset").b(false) {
                        let t = std::fs::read_to_string(self.levels.join(format!("{}.json", lv))).map_err(|e| err(format!("level {}: {}", lv, e)))?;
                        std::fs::write(&path, t).map_err(|e| err(e.to_string()))?;
                        self.docs.remove(&name);
                    }
                }
                let d = self.doc(req)?;
                J::obj().with("world", d.world.to_json()).with("undo", d.undo.len().into()).with("redo", d.redo.len().into())
            }
            "import" => {
                let name = clean_name(&req.get("name").s("")).ok_or_else(|| err("bad world name"))?;
                let w = import_text(&name, &req.get("text").s("")).map_err(err)?;
                let path = self.dir.join(format!("{}.json", name));
                std::fs::write(&path, w.to_json().pretty()).map_err(|e| err(e.to_string()))?;
                self.docs.remove(&name);
                J::obj().with("name", name.into())
            }
            "edit" => {
                let d = self.doc(req)?;
                let (a, res) = apply_op(&mut d.world, req.get("edit")).map_err(err)?;
                let ch = d.world.changes_json(&a);
                d.undo.push(a.inverse);
                if d.undo.len() > 10000 {
                    d.undo.remove(0);
                }
                d.redo.clear();
                Core::save(d);
                let ch = Core::live(d, ch);
                ch.with("result", res).with("undo", d.undo.len().into()).with("redo", 0usize.into())
            }
            "undo" | "redo" => {
                let d = self.doc(req)?;
                let edits = if cmd == "undo" { d.undo.pop() } else { d.redo.pop() };
                let Some(edits) = edits else { return Ok(J::obj().with("put", J::Arr(vec![])).with("del", J::Arr(vec![]))) };
                let a = d.world.apply_raw(edits);
                let ch = d.world.changes_json(&a);
                if cmd == "undo" {
                    d.redo.push(a.inverse);
                } else {
                    d.undo.push(a.inverse);
                }
                Core::save(d);
                let ch = Core::live(d, ch);
                ch.with("undo", d.undo.len().into()).with("redo", d.redo.len().into())
            }
            "resolve" => {
                let d = self.doc(req)?;
                s3_core::query::resolve(&d.world, req.get("brick").u(0))
            }
            "complete" => {
                let d = self.doc(req)?;
                let zone = d.world.bricks.get(&req.get("brick").u(0)).and_then(|b| d.world.zone_of_cell(b.cell)).map(|z| z.id);
                J::obj().with("items", s3_core::query::complete(&d.world, &req.get("prefix").s(""), zone))
            }
            "text" => {
                let d = self.doc(req)?;
                match build(&d.world) {
                    Ok(spec) => J::obj().with(
                        "zones",
                        J::Arr(spec.zones.iter().map(|z| J::obj().with("name", z.name.clone().into()).with("text", spec.units[z.unit].text.clone().into())).collect()),
                    ),
                    Err(ps) => J::obj().with("problems", J::Arr(ps.iter().map(|p| p.to_json()).collect())),
                }
            }
            "world" => {
                let d = self.doc(req)?;
                J::obj().with("text", d.world.to_json().pretty().into())
            }
            "eval" => {
                // the console: s run in the session's main zone, or in a machine of its own
                let d = self.doc(req)?;
                let text = req.get("text").s("");
                let budget = 50_000_000u64;
                let run = |rt: &mut s3_core::rt::Rt| -> (String, J) {
                    let st = match rt.guarded(|rt| rt.run_src(&text)) {
                        Some(st) => st,
                        None => rt.run(budget),
                    };
                    rt.flush();
                    let status = match st {
                        s3_core::rt::Status::Done => "ok".to_string(),
                        s3_core::rt::Status::Ran => {
                            rt.reset_frames();
                            format!("stopped after {} operations", budget)
                        }
                        s3_core::rt::Status::Error(e) => e,
                        s3_core::rt::Status::Exit(c) => format!("exit {}", c),
                        _ => "waiting".to_string(),
                    };
                    (status, J::Arr(rt.args.iter().map(|v| J::Str(v.text().into_owned())).collect()))
                };
                if let Some(s) = d.session.as_mut() {
                    if !s.zones[0].rt.fs.is_empty() {
                        return Err(err("main is running: pause or wait, then run it"));
                    }
                    let (st, line) = run(&mut s.zones[0].rt);
                    s.report(true).with("status", st.into()).with("line", line)
                } else {
                    if d.repl.is_none() {
                        let buf = Arc::new(Mutex::new(Vec::new()));
                        let rt = s3_core::rt::Rt::with_out(&INTERP, s3_core::rt::Out::Buf(buf.clone()));
                        d.repl = Some((rt, buf));
                    }
                    let (rt, buf) = d.repl.as_mut().unwrap();
                    let (st, line) = run(rt);
                    let out = String::from_utf8_lossy(&std::mem::take(&mut *buf.lock().unwrap())).into_owned();
                    J::obj().with("out", out.into()).with("status", st.into()).with("line", line)
                }
            }
            "worldtext" => {
                let d = self.doc(req)?;
                J::obj().with("text", s3_core::textform::world_text(&d.world).into())
            }
            "fromtext" => {
                // the world rebuilt from its text, as one undo step
                let d = self.doc(req)?;
                let n = s3_core::textform::world_from_text(&req.get("text").s("")).map_err(err)?;
                let mut edits: Vec<Edit> = Vec::new();
                for id in d.world.links.keys().chain(d.world.bricks.keys()).chain(d.world.spheres.keys()).chain(d.world.plates.keys()).chain(d.world.zones.keys()) {
                    edits.push(Edit::Del(*id));
                }
                for id in d.world.spaces.keys().filter(|&&x| x != WORLD) {
                    edits.push(Edit::Del(*id));
                }
                for x in n.spaces.values().filter(|x| x.id != WORLD) {
                    edits.push(Edit::Put(Rec::Space(x.clone())));
                }
                edits.extend(n.zones.values().map(|x| Edit::Put(Rec::Zone(x.clone()))));
                edits.extend(n.plates.values().map(|x| Edit::Put(Rec::Plate(x.clone()))));
                edits.extend(n.spheres.values().map(|x| Edit::Put(Rec::Sphere(x.clone()))));
                edits.extend(n.bricks.values().map(|x| Edit::Put(Rec::Brick(x.clone()))));
                edits.extend(n.links.values().map(|x| Edit::Put(Rec::Link(x.clone()))));
                edits.push(Edit::Meta(J::obj().with("params", n.params.clone()).with("goal", n.goal.clone())));
                let a = d.world.apply(edits).map_err(err)?;
                let ch = d.world.changes_json(&a);
                d.undo.push(a.inverse);
                d.redo.clear();
                Core::save(d);
                let ch = Core::live(d, ch);
                ch.with("world", d.world.to_json()).with("undo", d.undo.len().into()).with("redo", 0usize.into())
            }
            "run" => {
                let d = self.doc(req)?;
                let mut spec = build(&d.world).map_err(|ps| J::obj().with("problems", J::Arr(ps.iter().map(|p| p.to_json()).collect())))?;
                if req.get("lockstep").b(false) {
                    spec.params.set("lockstep", J::Bool(true));
                }
                let mut s = Session::new(spec, &INTERP);
                for b in req.get("breaks").arr() {
                    s.set_break(b.u(0), true);
                }
                let r = s.report(true);
                d.session = Some(s);
                r.with("stop", "ready".into())
            }
            "step" | "poll" => {
                let d = self.doc(req)?;
                let s = d.session.as_mut().ok_or_else(|| err("not running"))?;
                let kind = match req.get("kind").s("all").as_str() {
                    "token" => StepKind::Token,
                    "column" => StepKind::Column,
                    "event" => StepKind::Event,
                    _ => StepKind::All,
                };
                for _ in 0..req.get("ticks").u(0).min(100) {
                    s.tick();
                }
                let ops = req.get("ops").num().map(|x| x as u64).unwrap_or(200_000);
                let stop = s.run(ops, kind);
                let clock = s.has_clock();
                s.report(req.get("watch").b(true)).with("stop", stop.into()).with("clock", clock.into())
            }
            "fire" => {
                let d = self.doc(req)?;
                let s = d.session.as_mut().ok_or_else(|| err("not running"))?;
                let name = d.world.spheres.get(&req.get("sphere").u(0)).map(|s| s.name.clone()).unwrap_or_default();
                J::obj().with("ok", s.fire_named(&name).into())
            }
            "break" => {
                let d = self.doc(req)?;
                let ok = match d.session.as_mut() {
                    Some(s) => s.set_break(req.get("brick").u(0), req.get("on").b(true)),
                    None => true,
                };
                J::obj().with("ok", ok.into())
            }
            "stop" => {
                let d = self.doc(req)?;
                d.session = None;
                J::obj().with("ok", true.into())
            }
            "keep" => {
                // runtime positions and aims become an edit
                let d = self.doc(req)?;
                let s = d.session.as_mut().ok_or_else(|| err("not running"))?;
                let mut edits = Vec::new();
                for z in &s.zones {
                    if let Some(g) = z.rt.g.as_ref() {
                        for gs in &g.spheres {
                            if let Some(ws) = d.world.spheres.get(&gs.id) {
                                if ws.kind != SKind::Gate && (ws.pos != gs.pos || ws.aim != gs.aim || ws.reach != gs.reach) {
                                    let mut n = ws.clone();
                                    n.pos = gs.pos;
                                    n.aim = gs.aim;
                                    n.reach = gs.reach;
                                    edits.push(Edit::Put(Rec::Sphere(n)));
                                }
                            }
                        }
                    }
                }
                let a = d.world.apply(edits).map_err(err)?;
                let ch = d.world.changes_json(&a);
                d.undo.push(a.inverse);
                Core::save(d);
                ch
            }
            "compile" => {
                let d = self.doc(req)?;
                let spec = build(&d.world).map_err(|ps| J::obj().with("problems", J::Arr(ps.iter().map(|p| p.to_json()).collect())))?;
                let units: Vec<String> = spec.units.iter().map(|u| u.text.clone()).collect();
                let main = spec.units[spec.zones[0].unit].text.clone();
                let rs = compile_spec(&spec.to_json().dump(), &units, &main, s3_core::LIB, None).map_err(err)?;
                J::obj().with("rs", rs.into())
            }
            "build" => {
                let (rs, name) = {
                    let d = self.doc(req)?;
                    let spec = build(&d.world).map_err(|ps| J::obj().with("problems", J::Arr(ps.iter().map(|p| p.to_json()).collect())))?;
                    let units: Vec<String> = spec.units.iter().map(|u| u.text.clone()).collect();
                    let main = spec.units[spec.zones[0].unit].text.clone();
                    (compile_spec(&spec.to_json().dump(), &units, &main, s3_core::LIB, None).map_err(err)?, d.world.name.clone())
                };
                let id = self.next_job;
                self.next_job += 1;
                let job = Arc::new(Mutex::new(Job::default()));
                self.jobs.insert(id, job.clone());
                let dir = self.dir.join(".build");
                std::thread::spawn(move || run_build(job, dir, name, rs));
                J::obj().with("job", J::Num(id as f64))
            }
            "job" => {
                let j = self.jobs.get(&(req.get("job").u(0) as u64)).ok_or_else(|| err("no such job"))?;
                let j = j.lock().unwrap().clone();
                J::obj().with("done", j.done.into()).with("ok", j.ok.into()).with("log", j.log.into()).with("out", j.out.into())
            }
            "check" => {
                let d = self.doc(req)?;
                check_goal(&d.world)
            }
            other => return Err(err(format!("unknown command '{}'", other))),
        })
    }
}

/// Compiles with rustc and runs the program, in the background.
fn run_build(job: Arc<Mutex<Job>>, dir: PathBuf, name: String, rs: String) {
    let _ = std::fs::create_dir_all(&dir);
    let base = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect::<String>();
    let base = if base.is_empty() { "world".to_string() } else { base };
    let rsp = dir.join(format!("{}.rs", base));
    let bin = dir.join(&base);
    let set = |f: &dyn Fn(&mut Job)| f(&mut job.lock().unwrap());
    if let Err(e) = std::fs::write(&rsp, rs) {
        set(&|j| {
            j.done = true;
            j.log = e.to_string();
        });
        return;
    }
    set(&|j| j.log = "rustc -O …\n".into());
    let r = std::process::Command::new("rustc")
        .args(["-O", "--edition", "2021", "--crate-name", "s_program", "-o"])
        .arg(&bin)
        .arg(&rsp)
        .output();
    match r {
        Err(e) => set(&|j| {
            j.done = true;
            j.log.push_str(&format!("rustc is not on the path: {}\nThe Rust file is at {}", e, rsp.display()));
        }),
        Ok(o) if !o.status.success() => {
            let t = String::from_utf8_lossy(&o.stderr).into_owned();
            set(&|j| {
                j.done = true;
                j.log.push_str(&t);
            })
        }
        Ok(_) => {
            set(&|j| j.log.push_str(&format!("built {}\nrunning…\n", bin.display())));
            let o = std::process::Command::new(&bin).stdin(std::process::Stdio::null()).output();
            match o {
                Ok(o) => {
                    let out = String::from_utf8_lossy(&o.stdout).into_owned();
                    let e = String::from_utf8_lossy(&o.stderr).into_owned();
                    set(&|j| {
                        j.done = true;
                        j.ok = o.status.success();
                        j.out = out.clone();
                        j.log.push_str(&e);
                        j.log.push_str(&format!("exit {}\n", o.status.code().unwrap_or(-1)));
                    })
                }
                Err(e) => set(&|j| {
                    j.done = true;
                    j.log.push_str(&e.to_string());
                }),
            }
        }
    }
}

/// A level's goal: the output it must print, and par counts for the stars.
pub fn check_goal(w: &World) -> J {
    let goal = &w.goal;
    let spec = match build(w) {
        Ok(s) => s,
        Err(ps) => return J::obj().with("ok", false.into()).with("why", ps.iter().map(|p| p.msg.clone()).collect::<Vec<_>>().join("; ").into()),
    };
    let mut s = Session::new(spec, &INTERP);
    s.run_to_end(50_000_000);
    for _ in 0..goal.get("ticks").u(0).min(1000) {
        s.tick();
        s.run_to_end(50_000_000);
    }
    let out = s.output();
    let mut shown = J::obj();
    for z in &s.zones {
        if let Some(g) = z.rt.g.as_ref() {
            for sp in &g.spheres {
                if let Some(t) = &sp.shown {
                    shown.set(&sp.name, t.clone().into());
                }
            }
        }
    }
    let mut ok = true;
    let mut why = Vec::new();
    if let Some(want) = goal.get("output").str() {
        if out.trim() != want.trim() {
            ok = false;
            why.push(format!("the program printed {:?}, the goal is {:?}", out.trim(), want.trim()));
        }
    }
    for (name, want) in goal.get("shown").entries() {
        let got = shown.get(name).s("");
        if got != want.s("") {
            ok = false;
            why.push(format!("{} shows {:?}, the goal is {:?}", name, got, want.s("")));
        }
    }
    // datums: the value a cube holds when the program is done, and any parts named
    let rep = s.report(false);
    let mut held: Vec<(u32, J)> = Vec::new();
    for z in rep.get("zones").arr() {
        for d in z.get("datums").arr() {
            held.push((d.get("id").u(0), d.clone()));
        }
    }
    for (name, want) in goal.get("datums").entries() {
        let Some(rec) = w.datum_named(name) else {
            ok = false;
            why.push(format!("there is no cube named {}", name));
            continue;
        };
        let now = held.iter().find(|h| h.0 == rec.id).map(|h| h.1.clone()).unwrap_or_else(|| J::obj().with("value", J::Num(rec.value as f64)));
        let wants: Vec<(String, f64)> = match want {
            J::Num(x) => vec![("value".into(), *x)],
            o => o.entries().iter().filter_map(|(k, v)| v.num().map(|x| (k.clone(), x))).collect(),
        };
        for (k, x) in wants {
            let got = if k == "value" { now.get("value").num().unwrap_or(0.0) } else { now.get("parts").get(&k).num().unwrap_or(if k.starts_with('s') { 1.0 } else { 0.0 }) };
            if got != x {
                ok = false;
                why.push(if k == "value" { format!("{} holds {} (#{:06x}), the goal is {} (#{:06x})", name, got, got as u32, x, x as u32) } else { format!("{}/{} is {}, the goal is {}", name, k, got, x) });
            }
        }
    }
    if !s.errors.is_empty() && !goal.get("errors_ok").b(false) {
        ok = false;
        why.push(format!("error: {}", s.errors[0].get("msg").s("")));
    }
    let bricks = w.bricks.len();
    let spheres = w.spheres.len();
    let par = goal.get("par");
    // one star for solving; the other two are shared out over the par counts that are met
    let pars: Vec<bool> = [("bricks", bricks as f64), ("spheres", spheres as f64), ("events", s.total_events as f64)]
        .iter()
        .filter_map(|(k, v)| par.get(k).num().map(|p| *v <= p))
        .collect();
    let met = pars.iter().filter(|x| **x).count();
    let stars = if !ok { 0 } else if pars.is_empty() { 3 } else { 1 + (2 * met + pars.len() / 2) / pars.len() };
    J::obj()
        .with("ok", ok.into())
        .with("why", why.join("\n").into())
        .with("output", out.into())
        .with("shown", shown)
        .with("bricks", bricks.into())
        .with("spheres", spheres.into())
        .with("events", J::Num(s.total_events as f64))
        .with("stars", J::Num(stars as f64))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(name: &str) -> World {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../levels").join(name);
        World::from_json(&J::parse(&std::fs::read_to_string(p).unwrap()).unwrap()).unwrap()
    }

    #[test]
    fn the_datum_chapter_checks_the_cube() {
        let mut w = level("11-datum.json");
        let r = check_goal(&w);
        assert!(!r.get("ok").b(true), "{}", r.dump());
        assert!(r.get("why").s("").contains("#000000"), "{}", r.dump());
        let id = w.datum_named("c1").unwrap().id;
        // orange by tiles: ±x all on, +y 1010, −y 0101
        let mut tiles = Vec::new();
        for face in 0..2 {
            for t in 0..4 {
                tiles.push((face, t % 2, t / 2));
            }
        }
        tiles.extend([(2, 0, 0), (2, 0, 1), (3, 1, 0), (3, 1, 1)]);
        for (f, u, v) in tiles {
            apply_op(&mut w, &J::parse(&format!(r#"{{"op":"tile","id":{},"face":{},"u":{},"v":{}}}"#, id, f, u, v)).unwrap()).unwrap();
        }
        assert_eq!(w.datum_named("c1").unwrap().value, 0xffa500);
        let r = check_goal(&w);
        assert!(r.get("ok").b(false), "{}", r.dump());
        assert_eq!(r.get("stars").num(), Some(3.0));
    }
}
