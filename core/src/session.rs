// A running world: each zone's machine and graph, pulses, crossings, calls, joins, the clock,
// stepping and the reports a viewer reads. Compiled programs run the same session.
use crate::graph::*;
use crate::json::J;
use crate::part::*;
use crate::rt::*;
use crate::zone::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A unit of text the program runs, with the bricks its characters came from.
#[derive(Clone, Debug)]
pub struct Unit {
    pub text: String,
    /// (start, end, brick) in characters.
    pub map: Vec<(u32, u32, u32)>,
}

#[derive(Clone, Debug)]
pub struct ZoneSpec {
    pub id: u32,
    pub name: String,
    pub kind: String,
    pub unit: usize,
    pub graph: J,
    pub params: J,
    pub captures: Vec<(String, String)>,
    pub parent: Option<usize>,
}

impl ZoneSpec {
    pub fn param(&self, k: &str) -> Option<String> {
        match self.params.get(k) {
            J::Str(s) => Some(s.clone()),
            J::Null => None,
            v => Some(v.dump()),
        }
    }
}

/// What a world builds into: the texts and the graph table of every zone.
#[derive(Clone, Debug)]
pub struct Spec {
    pub units: Vec<Unit>,
    pub zones: Vec<ZoneSpec>,
    pub params: J,
}

impl Spec {
    pub fn to_json(&self) -> J {
        J::obj()
            .with(
                "units",
                J::Arr(
                    self.units
                        .iter()
                        .map(|u| {
                            J::obj().with("text", u.text.clone().into()).with(
                                "map",
                                J::Arr(u.map.iter().map(|m| J::Arr(vec![J::Num(m.0 as f64), J::Num(m.1 as f64), J::Num(m.2 as f64)])).collect()),
                            )
                        })
                        .collect(),
                ),
            )
            .with(
                "zones",
                J::Arr(
                    self.zones
                        .iter()
                        .map(|z| {
                            J::obj()
                                .with("id", z.id.into())
                                .with("name", z.name.clone().into())
                                .with("kind", z.kind.clone().into())
                                .with("unit", z.unit.into())
                                .with("graph", z.graph.clone())
                                .with("params", z.params.clone())
                                .with("captures", J::Arr(z.captures.iter().map(|(a, b)| J::Arr(vec![a.clone().into(), b.clone().into()])).collect()))
                                .with("parent", z.parent.map(|p| J::Num(p as f64)).unwrap_or(J::Null))
                        })
                        .collect(),
                ),
            )
            .with("params", self.params.clone())
    }
    pub fn from_json(j: &J) -> Spec {
        Spec {
            units: j
                .get("units")
                .arr()
                .iter()
                .map(|u| Unit {
                    text: u.get("text").s(""),
                    map: u.get("map").arr().iter().map(|m| (m.arr()[0].u(0), m.arr()[1].u(0), m.arr()[2].u(0))).collect(),
                })
                .collect(),
            zones: j
                .get("zones")
                .arr()
                .iter()
                .map(|z| ZoneSpec {
                    id: z.get("id").u(0),
                    name: z.get("name").s("main"),
                    kind: z.get("kind").s("main"),
                    unit: z.get("unit").u(0) as usize,
                    graph: z.get("graph").clone(),
                    params: z.get("params").clone(),
                    captures: z.get("captures").arr().iter().map(|c| (c.arr()[0].s(""), c.arr()[1].s("copy"))).collect(),
                    parent: z.get("parent").num().map(|x| x as usize),
                })
                .collect(),
            params: j.get("params").clone(),
        }
    }
    /// The brick a character of a unit came from: the innermost range holding it.
    pub fn brick_at(&self, unit: usize, off: u32) -> Option<u32> {
        let u = self.units.get(unit)?;
        u.map.iter().filter(|m| off >= m.0 && off < m.1).min_by_key(|m| m.1 - m.0).map(|m| m.2)
    }
    /// Where a brick's text starts: (unit, offset), preferring the smallest unit holding it.
    pub fn brick_start(&self, brick: u32) -> Option<(usize, u32)> {
        let mut best: Option<(usize, u32, usize)> = None;
        for (i, u) in self.units.iter().enumerate() {
            for m in &u.map {
                if m.2 == brick {
                    let len = u.text.len();
                    if best.map_or(true, |b| len < b.2) {
                        best = Some((i, m.0, len));
                    }
                }
            }
        }
        best.map(|b| (b.0, b.1))
    }
    pub fn depth(&self, z: usize) -> usize {
        let mut d = 0;
        let mut c = self.zones[z].parent;
        while let Some(p) = c {
            d += 1;
            c = self.zones[p].parent;
            if d > 64 {
                break;
            }
        }
        d
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Unspawned,
    Main,
    Pulse,
    Idle,
    Failed,
    Exited,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Phase::Unspawned => "unspawned",
            Phase::Main => "main",
            Phase::Pulse => "pulse",
            Phase::Idle => "idle",
            Phase::Failed => "failed",
            Phase::Exited => "exited",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ZWait {
    /// A pulse into a call gate waits for its return.
    Call(u64, usize),
    /// `gate.` in code waits for its return.
    CallCode(u64),
    Join(usize),
}

pub struct Serving {
    pub id: u64,
    pub from: usize,
    pub gate: String,
    pub chain: Vec<u32>,
    pub ret: Vec<Vec<String>>,
}

pub struct ZoneRun {
    pub spec: usize,
    pub rt: Rt,
    pub phase: Phase,
    pub pulse: Vec<Event>,
    /// The event whose body is running.
    pub cur: Option<Event>,
    pub events: u64,
    pub budget: u64,
    pub tick: u64,
    pub inbox: VecDeque<(usize, Msg)>,
    pub wait: Option<ZWait>,
    pub serving: Option<Serving>,
    /// Captured names this zone holds, with their modes.
    pub held: Vec<(String, String)>,
    pub round_next: usize,
    /// Which incarnation this is, when the zone roots a partition.
    pub gen: u64,
    /// A live edit waiting for the zone's next event boundary: the graph the zone was built
    /// from, and the definitions of the new build.
    pub patch: Option<(J, Vec<(String, String)>)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StepKind {
    Token,
    Column,
    Event,
    All,
}

pub struct Session {
    pub spec: Spec,
    pub prog: &'static Program,
    pub zones: Vec<ZoneRun>,
    pub out: Arc<Mutex<Vec<u8>>>,
    pub std: bool,
    pub argv: Vec<String>,
    pub out_sent: usize,
    pub trace: Vec<J>,
    pub trace_sent: usize,
    pub errors: Vec<J>,
    pub errors_sent: usize,
    pub exit: Option<i32>,
    pub total_events: u64,
    pub halted: bool,
    pub lockstep: bool,
    pub outbox: Vec<(usize, usize, Msg)>,
    pub next_call: u64,
    pub rounds: u64,
    /// Zones running on other threads or machines.
    pub parts: Option<Box<Parts>>,
    /// Serving coupled worlds: the program does not end when it settles.
    pub serving: bool,
}

pub fn line_json(line: &[Val]) -> J {
    J::Arr(line.iter().map(|v| J::Str(v.text().into_owned())).collect())
}

fn texts(line: &[Val]) -> Vec<String> {
    line.iter().map(|v| v.text().into_owned()).collect()
}

fn vals(line: &[String]) -> Vec<Val> {
    line.iter().map(|s| Val::of(s.clone())).collect()
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(n).collect();
        t.push('…');
        t
    }
}

impl Session {
    pub fn new(spec: Spec, prog: &'static Program) -> Session {
        Session::with_out(spec, prog, false, Vec::new())
    }

    /// `std`: write output to standard output as it comes, as a program does.
    pub fn with_out(spec: Spec, prog: &'static Program, std: bool, argv: Vec<String>) -> Session {
        let mut s = Session::bare(spec, prog, std, argv);
        s.start_main(0);
        s
    }

    /// A session with every zone unspawned and main not started.
    pub fn bare(spec: Spec, prog: &'static Program, std: bool, argv: Vec<String>) -> Session {
        Session::bare_with(spec, prog, std, argv, Arc::new(Mutex::new(Vec::new())))
    }

    pub fn bare_with(spec: Spec, prog: &'static Program, std: bool, argv: Vec<String>, out: Arc<Mutex<Vec<u8>>>) -> Session {
        let lockstep = spec.params.get("lockstep").b(false) || spec.params.get("lockstep").num().map_or(false, |x| x != 0.0);
        let mut s = Session {
            spec,
            prog,
            zones: Vec::new(),
            out,
            std,
            argv,
            out_sent: 0,
            trace: Vec::new(),
            trace_sent: 0,
            errors: Vec::new(),
            errors_sent: 0,
            exit: None,
            total_events: 0,
            halted: false,
            lockstep,
            outbox: Vec::new(),
            next_call: 1,
            rounds: 0,
            parts: None,
            serving: false,
        };
        for zi in 0..s.spec.zones.len() {
            let z = s.fresh(zi);
            s.zones.push(z);
        }
        s
    }

    fn fresh(&self, zi: usize) -> ZoneRun {
        let z = &self.spec.zones[zi];
        let mut rt = Rt::with_out(self.prog, if self.std { Out::std() } else { Out::Buf(self.out.clone()) });
        rt.stdin = self.std && zi == 0;
        rt.set_argv(self.argv.clone());
        rt.nonblock = z.kind != "main";
        for (i, u) in self.spec.units.iter().enumerate() {
            rt.unit_ids.insert(u.text.clone(), i as u32);
        }
        let mut g = Graph::from_json(&z.graph, &mut rt.it);
        g.zone_name = z.name.clone();
        g.host = z.param("host").unwrap_or_else(|| "local".into());
        seed_datums(&mut rt, &g.datums, None);
        rt.g = Some(Box::new(g));
        let budget = z.params.get("budget").num().or_else(|| self.spec.params.get("budget").num()).unwrap_or(100000.0) as u64;
        ZoneRun {
            spec: zi,
            rt,
            phase: Phase::Unspawned,
            pulse: Vec::new(),
            cur: None,
            events: 0,
            budget,
            tick: 0,
            inbox: VecDeque::new(),
            wait: None,
            serving: None,
            held: Vec::new(),
            round_next: 0,
            gen: 0,
            patch: None,
        }
    }

    pub(crate) fn start_main(&mut self, z: usize) {
        let text = self.spec.units[self.spec.zones[z].unit].text.clone();
        self.zones[z].phase = Phase::Main;
        if let Some(Status::Error(e)) = self.zones[z].rt.guarded(|rt| rt.run_src(&text)) {
            self.error(z, e);
            self.zones[z].phase = Phase::Failed;
        }
    }

    fn zname(&self, z: usize) -> String {
        self.spec.zones[z].name.clone()
    }

    pub fn zone_named(&self, name: &str) -> Option<usize> {
        self.spec.zones.iter().position(|z| z.name == name)
    }

    /// The brick an error or a step stands on, from a machine's last site.
    fn site_brick(&mut self, z: usize) -> Option<u32> {
        let (unit, pc) = self.zones[z].rt.site;
        if unit == NO_UNIT {
            return None;
        }
        let text = self.spec.units.get(unit as usize)?.text.clone();
        let code = self.zones[z].rt.code_of(&text).ok()?;
        let off = *code.spans.get(pc as usize)?;
        self.spec.brick_at(unit as usize, off)
    }

    pub fn set_break(&mut self, brick: u32, on: bool) -> bool {
        let Some((unit, off)) = self.spec.brick_start(brick) else { return false };
        let text = self.spec.units[unit].text.clone();
        let mut done = false;
        for z in self.zones.iter_mut() {
            if let Ok(code) = z.rt.code_of(&text) {
                if let Some(pc) = code.spans.iter().position(|&s| s == off) {
                    let key = ((unit as u64) << 32) | pc as u64;
                    if on {
                        z.rt.breaks.insert(key);
                    } else {
                        z.rt.breaks.remove(&key);
                    }
                    done = true;
                }
            }
        }
        done
    }

    pub(crate) fn error(&mut self, z: usize, msg: String) {
        if z >= self.zones.len() {
            self.errors.push(J::obj().with("msg", msg.into()));
            return;
        }
        let brick = self.site_brick(z);
        let sphere = self.zones[z].cur.as_ref().map(|e| e.sphere);
        let mut e = J::obj().with("zone", self.zname(z).into()).with("msg", msg.clone().into());
        if let Some(b) = brick {
            e.set("brick", b.into());
        }
        if let Some(s) = sphere {
            e.set("sphere", s.into());
            if let Some(g) = self.zones[z].rt.g.as_mut() {
                if let Some(i) = g.idx(s) {
                    g.spheres[i].error = Some(msg);
                }
            }
        }
        self.errors.push(e);
    }

    pub(crate) fn note(&mut self, z: usize, what: &str, detail: J) {
        if z >= self.zones.len() {
            return;
        }
        let name = self.zname(z);
        self.trace.push(J::obj().with("n", J::Num(self.total_events as f64)).with("zone", name.into()).with(what, detail));
    }

    // live edits

    /// Takes a new build of the world while running. Each spawned zone takes its new graph
    /// and changed definitions at its next event boundary; a running event finishes as it
    /// began. The zones themselves must be unchanged.
    pub fn patch(&mut self, spec: Spec, defs: Vec<(usize, String, String)>) -> Result<(), String> {
        if self.parts.is_some() {
            return Err("zones on other threads take no live edits".into());
        }
        let same = spec.zones.len() == self.spec.zones.len()
            && spec.zones.iter().zip(self.spec.zones.iter()).all(|(a, b)| a.name == b.name && a.kind == b.kind && a.parent == b.parent && a.captures == b.captures && a.params == b.params);
        if !same {
            return Err("the zones changed".into());
        }
        let old = std::mem::replace(&mut self.spec, spec);
        for z in 0..self.zones.len() {
            let mine: Vec<(String, String)> = defs.iter().filter(|d| d.0 == z).map(|d| (d.1.clone(), d.2.clone())).collect();
            let zr = &mut self.zones[z];
            zr.rt.unit_ids.clear();
            for (i, u) in self.spec.units.iter().enumerate() {
                zr.rt.unit_ids.insert(u.text.clone(), i as u32);
            }
            if zr.phase == Phase::Unspawned {
                let g = Graph::from_json(&self.spec.zones[z].graph, &mut zr.rt.it);
                let was = Graph::from_json(&old.zones[z].graph, &mut zr.rt.it);
                seed_datums(&mut zr.rt, &g.datums, Some(&was.datums));
                if let Some(cur) = zr.rt.g.as_mut() {
                    cur.datums = g.datums;
                }
                continue;
            }
            let base = match zr.patch.take() {
                Some((b, _)) => b,
                None => old.zones[z].graph.clone(),
            };
            zr.patch = Some((base, mine));
        }
        Ok(())
    }

    /// Applies a zone's waiting live edit; the zone is between events.
    fn apply_patch(&mut self, z: usize) {
        let Some((base, defs)) = self.zones[z].patch.take() else { return };
        let gj = self.spec.zones[z].graph.clone();
        let zname = self.spec.zones[z].name.clone();
        let host = self.spec.zones[z].param("host").unwrap_or_else(|| "local".into());
        let zr = &mut self.zones[z];
        let mut g = Graph::from_json(&gj, &mut zr.rt.it);
        g.zone_name = zname;
        g.host = host;
        let spec_pose = |j: &J, id: u32| j.get("spheres").arr().iter().find(|s| s.get("id").u(0) == id).map(|s| (s.get("pos").dump(), s.get("aim").dump(), s.get("reach").dump()));
        let mut moved = false;
        if let Some(old) = zr.rt.g.as_ref() {
            for s in g.spheres.iter_mut() {
                let Some(o) = old.spheres.iter().find(|o| o.id == s.id && o.side == s.side) else { continue };
                s.notch = o.notch;
                s.state = o.state.clone();
                if s.machine == o.machine {
                    s.hold = o.hold.clone();
                    s.outq = o.outq.clone();
                    s.turn = o.turn;
                }
                s.shown = o.shown.clone();
                s.error = o.error.clone();
                if o.moved && spec_pose(&base, s.id) == spec_pose(&gj, s.id) {
                    s.pos = o.pos;
                    s.aim = o.aim;
                    s.reach = o.reach;
                    s.moved = true;
                    moved = true;
                }
            }
        }
        // items ride on along tracks that kept their cells
        if let Some(old) = zr.rt.g.as_ref() {
            g.next_item = old.next_item;
            for t in g.tracks.iter_mut() {
                if let Some(o) = old.tracks.iter().find(|o| o.id == t.id && o.cells == t.cells) {
                    t.items = o.items.clone();
                }
            }
        }
        if moved {
            g.refresh();
        }
        g.links_changed = true;
        let was = Graph::from_json(&base, &mut zr.rt.it);
        seed_datums(&mut zr.rt, &g.datums, Some(&was.datums));
        zr.rt.g = Some(Box::new(g));
        for (name, text) in defs {
            let k = zr.rt.it.sym(&name);
            if zr.rt.script_text(k).as_deref() != Some(text.as_str()) {
                if let Err(e) = zr.rt.def_script_text(&name, &text) {
                    let msg = format!("{}: {}", name, e);
                    self.error(z, msg);
                    return;
                }
            }
        }
        self.note(z, "patched", J::Bool(true));
    }

    // spawning, capturing and joining

    /// Starts a zone: definitions, library guards and captures come from its parent.
    pub fn spawn(&mut self, t: usize) -> Result<(), String> {
        if self.zones[t].phase != Phase::Unspawned {
            return Ok(());
        }
        let p = self.spec.zones[t].parent.unwrap_or(0);
        if !self.local(p) {
            // the parent runs elsewhere: it takes the captures and sends the image
            let w = self.owner_of(p);
            self.psend(w, Pm::SpawnReq { t: t as u32 });
            return Ok(());
        }
        if self.zones[p].phase == Phase::Unspawned {
            self.spawn(p)?;
        }
        let kind = self.spec.zones[t].kind.clone();
        let remote = kind == "net" || (kind == "rpc" && self.spec.zones[t].param("at").map_or(false, |a| a != "core")) || self.spec.zones[t].param("peer").is_some();
        let defs = self.zones[p].rt.defs_text();
        let guards: Vec<String> = {
            let rt = &self.zones[p].rt;
            rt.it
                .names
                .iter()
                .enumerate()
                .filter(|(i, n)| n.ends_with(".s") && (rt.mem.get(*i).map_or(false, |v| v.is_some()) || rt.nums.get(*i).map_or(false, |v| v.is_some())))
                .map(|(_, n)| n.to_string())
                .collect()
        };
        let caps_spec = self.spec.zones[t].captures.clone();
        // check every capture before taking any
        for (name, mode) in &caps_spec {
            let held = self.zones[p].held.iter().find(|h| &h.0 == name).map(|h| h.1.clone());
            if !reborrow_ok(held.as_deref(), mode) {
                return Err(format!("{}: cannot take {} as {} from a {} capture", self.zname(t), name, mode, held.unwrap_or_default()));
            }
            let has_h = has_handle(&self.zones[p].rt, name);
            if has_h && mode == "copy" {
                return Err(format!("{}: a handle cannot be copied: {}", self.zname(t), name));
            }
            if has_h && remote {
                return Err(format!("handle cannot leave host: {}", name));
            }
        }
        let mut caps = Vec::new();
        for (name, mode) in &caps_spec {
            let take = matches!(mode.as_str(), "move" | "mut");
            let st = take_stored(&mut self.zones[p].rt, name, take);
            let k = self.zones[p].rt.it.sym(name);
            match mode.as_str() {
                "move" => self.zones[p].rt.set_guard(k, G_MOVED),
                "ref" => self.zones[p].rt.set_guard(k, G_LENT_RO),
                "mut" => self.zones[p].rt.set_guard(k, G_LENT),
                _ => {}
            }
            caps.push((name.clone(), mode.clone(), st));
        }
        let msg = Msg::Spawn { defs, guards, caps };
        self.note(t, "spawn", J::Str(kind));
        if !self.local(t) {
            self.spawn_remote(t, msg);
            return Ok(());
        }
        self.apply_spawn(t, msg);
        Ok(())
    }

    pub(crate) fn apply_spawn(&mut self, t: usize, m: Msg) {
        let mut z = self.fresh(t);
        z.inbox = std::mem::take(&mut self.zones[t].inbox);
        z.gen = self.zones[t].gen;
        if let Msg::Spawn { defs, guards, caps } = m {
            for (k, v) in defs {
                z.rt.add_def(&k, &v);
            }
            for g in guards {
                let k = z.rt.it.sym(&g);
                z.rt.mset(k, Val::N(1.0));
            }
            for (name, mode, st) in caps {
                put_stored(&mut z.rt, &name, st);
                if mode == "ref" {
                    let k = z.rt.it.sym(&name);
                    z.rt.set_guard(k, G_BORROWED);
                }
                z.held.push((name, mode));
            }
        }
        self.zones[t] = z;
        if self.spec.zones[t].kind == "scope" {
            // a scope's words come first from its dialect's library
            let d = self.spec.zones[t].param("dialect").unwrap_or_default();
            let path = format!("std/{}.s", d);
            if !d.is_empty() && d != "s" && self.prog.lib.iter().any(|(p, _)| *p == path) {
                if let Some(Status::Error(e)) = self.zones[t].rt.guarded(|rt| rt.run_src(&format!("{}.", path))) {
                    self.error(t, e);
                }
                let _ = self.zones[t].rt.run(u64::MAX >> 8);
            }
        }
        self.start_main(t);
    }

    /// A scope whose dialect advances by ticks instead of pulses.
    pub fn ticked(&self, z: usize) -> bool {
        self.spec.zones[z].kind == "scope" && self.spec.zones[z].param("dialect").as_deref() == Some("logic")
    }

    /// One tick of a ticked scope: every sphere computes its output from what its inputs put
    /// out on the tick before, all at once. A source puts out its `on` param, or a square wave
    /// when it has `every`. A gate on the scope's wall sends its inputs out when they change.
    fn logic_tick(&mut self, z: usize) {
        let tick = self.zones[z].tick;
        let zr = &mut self.zones[z];
        let (n, links) = {
            let Some(g) = zr.rt.g.as_mut() else { return };
            for s in g.spheres.iter_mut() {
                // a gate starts with nothing, so what first reaches it is sent
                if s.state.is_empty() && s.kind != K_GATE {
                    s.state = vec![Val::N(s.param("init", 0.0))];
                }
            }
            let mut links: Vec<(u32, u32, u32)> = g.links.iter().map(|l| (l.id, l.from, l.to)).collect();
            links.sort();
            (g.spheres.len(), links)
        };
        let ins_of = |g: &crate::graph::Graph, id: u32| -> Vec<Val> {
            let mut v = Vec::new();
            for (_, f, t) in &links {
                if *t == id {
                    if let Some(i) = g.idx(*f) {
                        v.extend(g.spheres[i].state.iter().cloned());
                    }
                }
            }
            v
        };
        let texts = |v: &[Val]| v.iter().map(|x| x.text().into_owned()).collect::<Vec<_>>();
        let mut next: Vec<Option<Vec<Val>>> = vec![None; n];
        let mut outward: Vec<(String, Vec<Val>)> = Vec::new();
        for i in 0..n {
            let (kind, id, body, side, ins, every, on, name, was) = {
                let g = zr.rt.g.as_ref().unwrap();
                let s = &g.spheres[i];
                (s.kind, s.id, s.body, s.side, ins_of(g, s.id), s.param("every", 0.0), s.param("on", 0.0), s.name.clone(), texts(&s.state))
            };
            if kind == K_GATE {
                // inbound gates hold what crossed in; outbound ones send what reaches them
                if side == 2 && links.iter().any(|l| l.2 == id) && texts(&ins) != was {
                    next[i] = Some(ins.clone());
                    outward.push((name, ins));
                }
                continue;
            }
            if kind == K_SRC {
                let v = if every >= 1.0 { ((tick / every as u64) % 2) as f64 } else { on };
                next[i] = Some(vec![Val::N(v)]);
                continue;
            }
            if kind == K_OUT {
                next[i] = Some(ins);
                continue;
            }
            let rt = &mut zr.rt;
            rt.args = ins;
            let r = match rt.guarded(|rt| rt.call(body)) {
                Some(st) => st,
                None => rt.run(1 << 20),
            };
            let out = std::mem::take(&mut rt.args);
            rt.reset_frames();
            let g = rt.g.as_mut().unwrap();
            match r {
                Status::Error(e) => g.spheres[i].error = Some(e),
                _ => {
                    g.spheres[i].error = None;
                    next[i] = Some(out);
                }
            }
        }
        let zname = self.zname(z);
        let g = self.zones[z].rt.g.as_mut().unwrap();
        for (i, v) in next.into_iter().enumerate() {
            if let Some(v) = v {
                g.spheres[i].shown = Some(v.iter().map(|x| x.text().into_owned()).collect::<Vec<_>>().join(" "));
                g.spheres[i].state = v;
            }
        }
        let _ = zname;
        for (gate, line) in outward {
            let gi = self.zones[z].rt.g.as_ref().unwrap().named(&gate);
            if let Some(gi) = gi {
                let (side, to) = {
                    let s = &self.zones[z].rt.g.as_ref().unwrap().spheres[gi];
                    (s.side, s.zone_to as usize)
                };
                if !line.is_empty() {
                    self.cross(z, &gate, side, to, line, Vec::new());
                }
            }
        }
    }

    /// Ends a zone: `mut` captures move back, `ref` releases, the zone can spawn again.
    pub fn join(&mut self, t: usize) {
        if t == 0 || self.zones[t].phase == Phase::Unspawned {
            return;
        }
        if !self.local(t) {
            self.join_remote(t);
            return;
        }
        for c in 0..self.zones.len() {
            if self.spec.zones[c].parent == Some(t) && c != t {
                self.join(c);
            }
        }
        let p = self.spec.zones[t].parent.unwrap_or(0);
        let held = std::mem::take(&mut self.zones[t].held);
        let held = if !self.local(p) {
            // a partition root: its mut captures go back to the parent's partition
            let mut caps = Vec::new();
            for (name, mode) in held {
                if mode == "mut" {
                    let k2 = self.zones[t].rt.it.sym(&name);
                    self.zones[t].rt.set_guard(k2, G_NONE);
                    caps.push((name.clone(), take_stored(&mut self.zones[t].rt, &name, true)));
                }
            }
            let gen = self.zones[t].gen;
            let w = self.owner_of(p);
            self.psend(w, Pm::Joined { t: t as u32, gen, caps });
            Vec::new()
        } else {
            held
        };
        for (name, mode) in held {
            let k = self.zones[p].rt.it.sym(&name);
            match mode.as_str() {
                "mut" => {
                    let k2 = self.zones[t].rt.it.sym(&name);
                    self.zones[t].rt.set_guard(k2, G_NONE);
                    let st = take_stored(&mut self.zones[t].rt, &name, true);
                    self.zones[p].rt.set_guard(k, G_NONE);
                    put_stored(&mut self.zones[p].rt, &name, st);
                }
                "ref" => self.zones[p].rt.set_guard(k, G_NONE),
                _ => {}
            }
        }
        self.zones[t].rt.flush();
        self.note(t, "joined", J::Bool(true));
        let mut fresh = self.fresh(t);
        fresh.gen = self.zones[t].gen;
        // what out spheres showed stays on view; it is not state the zone can read
        if let (Some(old), Some(new)) = (self.zones[t].rt.g.as_ref(), fresh.rt.g.as_mut()) {
            for s in new.spheres.iter_mut() {
                if let Some(o) = old.spheres.iter().find(|o| o.id == s.id) {
                    s.shown = o.shown.clone();
                    s.error = o.error.clone();
                }
            }
        }
        self.zones[t] = fresh;
    }

    fn quiet(&self, z: usize) -> bool {
        if !self.local(z) {
            return self.remote_settled(z);
        }
        let zr = &self.zones[z];
        matches!(zr.phase, Phase::Idle | Phase::Failed | Phase::Exited | Phase::Unspawned) && zr.cur.is_none() && zr.pulse.is_empty() && zr.wait.is_none() && zr.rt.fs.is_empty()
    }

    pub(crate) fn settled(&self, z: usize) -> bool {
        if !self.local(z) {
            return self.remote_settled(z);
        }
        self.quiet(z) && self.zones[z].inbox.is_empty() && self.zones[z].serving.is_none()
    }

    // messages

    fn send(&mut self, from: usize, to: usize, m: Msg) {
        if self.lockstep {
            self.outbox.push((from, to, m));
        } else {
            self.deliver(from, to, m);
        }
    }

    pub(crate) fn deliver(&mut self, from: usize, to: usize, m: Msg) {
        if to >= self.zones.len() {
            // a return for a coupled world: its link belongs to the coordinator
            if self.is_worker() {
                self.psend(0, Pm::Deliver { from: from as u32, to: to as u32, m });
                return;
            }
            let w = self.owner_of(to);
            return self.to_caller(w, m);
        }
        if !self.local(to) {
            if !matches!(m, Msg::Ret { .. }) && self.zones[to].phase == Phase::Unspawned {
                if let Err(e) = self.spawn(to) {
                    self.error(from, e);
                    return;
                }
            }
            let w = self.owner_of(to);
            if !self.psend(w, Pm::Deliver { from: from as u32, to: to as u32, m }) && self.local(from) {
                let e = format!("zone lost: {}", self.zname(to));
                self.error(from, e);
            }
            return;
        }
        match m {
            Msg::Ret { id, line, err } => match self.zones[to].wait.clone() {
                Some(ZWait::Call(cid, gi)) if cid == id => {
                    self.zones[to].wait = None;
                    if let Some(e) = err {
                        self.error(to, e);
                        return;
                    }
                    let zr = &mut self.zones[to];
                    let g = zr.rt.g.as_ref().unwrap();
                    let outs = g.live_out(gi, P_OUT);
                    let line = vals(&line);
                    if !line.is_empty() {
                        for (lid, t) in outs.iter().rev() {
                            zr.pulse.push(Event { sphere: *t, line: line.clone(), via: *lid });
                        }
                    }
                    if zr.phase == Phase::Idle && !zr.pulse.is_empty() {
                        zr.phase = Phase::Pulse;
                    }
                }
                Some(ZWait::CallCode(cid)) if cid == id => {
                    self.zones[to].wait = None;
                    if let Some(e) = err {
                        self.fail_running(to, e);
                        return;
                    }
                    let zr = &mut self.zones[to];
                    if matches!(zr.rt.fs.last().map(|f| &f.body), Some(Body::Await(_))) {
                        zr.rt.fs.pop();
                    }
                    zr.rt.args = vals(&line);
                }
                _ => {}
            },
            m => {
                if self.zones[to].phase == Phase::Unspawned {
                    if let Err(e) = self.spawn(to) {
                        self.error(from, e);
                        return;
                    }
                }
                self.zones[to].inbox.push_back((from, m));
            }
        }
    }

    fn flush_outbox(&mut self) {
        let mut out = std::mem::take(&mut self.outbox);
        out.sort_by_key(|x| x.0);
        for (f, t, m) in out {
            self.deliver(f, t, m);
        }
    }

    /// Takes the next message, if the zone is quiet and its merge rule allows.
    fn take_message(&mut self, z: usize) -> Option<(usize, Msg)> {
        if !self.quiet(z) || self.zones[z].serving.is_some() {
            return None;
        }
        let merge = self.spec.zones[z].param("merge").unwrap_or_else(|| "arrival".into());
        if merge != "round" {
            return self.zones[z].inbox.pop_front();
        }
        let mut senders: Vec<usize> = self.neighbors(z);
        senders.sort();
        if senders.is_empty() {
            return self.zones[z].inbox.pop_front();
        }
        for _ in 0..senders.len() {
            let k = self.zones[z].round_next % senders.len();
            let s = senders[k];
            if let Some(pos) = self.zones[z].inbox.iter().position(|x| x.0 == s) {
                self.zones[z].round_next = k + 1;
                return self.zones[z].inbox.remove(pos);
            }
            if matches!(self.zones[s].phase, Phase::Unspawned | Phase::Failed | Phase::Exited) {
                self.zones[z].round_next = k + 1;
                continue;
            }
            return None;
        }
        None
    }

    fn neighbors(&self, z: usize) -> Vec<usize> {
        let mut v = Vec::new();
        if let Some(g) = self.zones[z].rt.g.as_ref() {
            for s in &g.spheres {
                if s.kind == K_GATE && s.side != 0 && !v.contains(&(s.zone_to as usize)) {
                    v.push(s.zone_to as usize);
                }
            }
        }
        v
    }

    fn handle(&mut self, z: usize, from: usize, m: Msg) {
        match m {
            Msg::Line { gate, line, given } => {
                for (n, st) in given {
                    put_stored(&mut self.zones[z].rt, &n, st);
                }
                self.emit_from_gate(z, &gate, vals(&line));
            }
            Msg::Call { id, gate, line, given, chain } => {
                for (n, st) in given {
                    put_stored(&mut self.zones[z].rt, &n, st);
                }
                self.zones[z].serving = Some(Serving { id, from, gate: gate.clone(), chain, ret: Vec::new() });
                self.emit_from_gate(z, &gate, vals(&line));
            }
            Msg::Join | Msg::Stop => self.join(z),
            Msg::Err(e) => self.error(z, e),
            _ => {}
        }
    }

    fn emit_from_gate(&mut self, z: usize, gate: &str, line: Vec<Val>) {
        let gi = match self.zones[z].rt.g.as_ref().unwrap().named(gate) {
            Some(i) => i,
            None => {
                let e = format!("no gate {} in zone {}", gate, self.zname(z));
                self.error(z, e);
                return;
            }
        };
        let mut line = line;
        if self.spec.zones[z].kind == "scope" {
            let body = self.zones[z].rt.g.as_ref().unwrap().spheres[gi].body;
            let rt = &mut self.zones[z].rt;
            rt.args = line;
            let r = match rt.guarded(|rt| rt.call(body)) {
                Some(st) => st,
                None => rt.run(1 << 20),
            };
            line = std::mem::take(&mut rt.args);
            rt.reset_frames();
            if let Status::Error(e) = r {
                self.error(z, e);
                return;
            }
        }
        if self.ticked(z) {
            let g = self.zones[z].rt.g.as_mut().unwrap();
            g.spheres[gi].shown = Some(line.iter().map(|x| x.text().into_owned()).collect::<Vec<_>>().join(" "));
            g.spheres[gi].state = line;
            return;
        }
        let zr = &mut self.zones[z];
        let outs = zr.rt.g.as_ref().unwrap().live_out(gi, P_OUT);
        if !line.is_empty() {
            for (lid, t) in outs.iter().rev() {
                zr.pulse.push(Event { sphere: *t, line: line.clone(), via: *lid });
            }
        }
        if zr.phase == Phase::Idle && !zr.pulse.is_empty() {
            zr.phase = Phase::Pulse;
        }
    }

    /// An rpc zone that served a call and is quiet again returns what reached the gate.
    fn finish_call(&mut self, z: usize) {
        if let Some(sv) = self.zones[z].serving.take() {
            let mut line = Vec::new();
            for l in sv.ret {
                line.extend(l);
            }
            self.send(z, sv.from, Msg::Ret { id: sv.id, line, err: None });
        }
    }

    fn chain_of(&self, z: usize) -> Vec<u32> {
        let mut c = self.zones[z].serving.as_ref().map(|s| s.chain.clone()).unwrap_or_default();
        c.push(z as u32);
        c
    }

    // pulses

    /// Queues a pulse from a sphere: sources start on an empty line.
    pub fn fire(&mut self, z: usize, sphere: u32, line: Vec<Val>) {
        if !self.local(z) {
            return;
        }
        if self.zones[z].phase == Phase::Unspawned {
            if let Err(e) = self.spawn(z) {
                self.error(z, e);
                return;
            }
        }
        self.zones[z].pulse.push(Event { sphere, line, via: 0 });
        if self.zones[z].phase == Phase::Idle {
            self.zones[z].phase = Phase::Pulse;
        }
    }

    pub fn fire_named(&mut self, name: &str) -> bool {
        for z in 0..self.zones.len() {
            let id = self.zones[z].rt.g.as_ref().and_then(|g| g.named(name).map(|i| g.spheres[i].id));
            if let Some(id) = id {
                self.fire(z, id, Vec::new());
                return true;
            }
        }
        false
    }

    fn fire_sources(&mut self, z: usize) {
        if self.ticked(z) {
            self.zones[z].phase = Phase::Idle;
            return;
        }
        let mut ids: Vec<u32> = match self.zones[z].rt.g.as_ref() {
            Some(g) => g.spheres.iter().filter(|s| s.kind == K_SRC && s.param("run", 1.0) != 0.0).map(|s| s.id).collect(),
            None => Vec::new(),
        };
        ids.sort();
        for id in ids.into_iter().rev() {
            self.zones[z].pulse.push(Event { sphere: id, line: Vec::new(), via: 0 });
        }
        self.zones[z].phase = if self.zones[z].pulse.is_empty() { Phase::Idle } else { Phase::Pulse };
    }

    /// Advances the clock one tick: sources with every = k fire on every k-th tick.
    pub fn tick(&mut self) {
        self.tick_parts();
        // scopes are always there: they spawn on the first tick if nothing spawned them yet
        for z in 1..self.zones.len() {
            if self.local(z) && self.spec.zones[z].kind == "scope" && self.zones[z].phase == Phase::Unspawned {
                if let Err(e) = self.spawn(z) {
                    self.error(z, e);
                }
            }
        }
        for z in 0..self.zones.len() {
            if !self.local(z) {
                continue;
            }
            if !matches!(self.zones[z].phase, Phase::Pulse | Phase::Idle) {
                continue;
            }
            self.zones[z].tick += 1;
            let t = self.zones[z].tick;
            if self.ticked(z) {
                self.logic_tick(z);
                continue;
            }
            self.machine_tick(z);
            let due: Vec<u32> = match self.zones[z].rt.g.as_ref() {
                Some(g) => g
                    .spheres
                    .iter()
                    .filter(|s| s.kind == K_SRC && s.param("every", 0.0) >= 1.0 && t % (s.param("every", 0.0) as u64) == 0)
                    .map(|s| s.id)
                    .collect(),
                None => Vec::new(),
            };
            for id in due.into_iter().rev() {
                self.fire(z, id, Vec::new());
            }
        }
    }

    /// Tracks and machines move one tick; what readers take goes out on their links.
    fn machine_tick(&mut self, z: usize) {
        let zr = &mut self.zones[z];
        let Some(g) = zr.rt.g.as_mut() else { return };
        if g.tracks.is_empty() {
            return;
        }
        let emits = crate::machine::tick(g);
        for (sid, line) in emits.into_iter().rev() {
            let Some(i) = g.idx(sid) else { continue };
            for (lid, to) in g.live_out(i, P_OUT).into_iter().rev() {
                zr.pulse.push(Event { sphere: to, line: line.clone(), via: lid });
            }
        }
        if !zr.pulse.is_empty() {
            zr.phase = Phase::Pulse;
        }
    }

    pub fn has_clock(&self) -> bool {
        self.zones.iter().any(|z| z.rt.g.as_ref().map_or(false, |g| !g.tracks.is_empty()))
            || (0..self.zones.len()).any(|z| self.ticked(z)) || self.zones.iter().any(|z| z.rt.g.as_ref().map_or(false, |g| g.spheres.iter().any(|s| s.kind == K_SRC && s.param("every", 0.0) >= 1.0)))
    }

    /// Starts the next pulse event's body; false when there is none.
    fn start_event(&mut self, z: usize) -> bool {
        let zname = self.zname(z);
        let zr = &mut self.zones[z];
        if zr.budget > 0 && zr.events >= zr.budget {
            if !zr.pulse.is_empty() {
                let n = zr.pulse.len();
                zr.pulse.clear();
                self.halted = true;
                self.trace.push(J::obj().with("zone", zname.into()).with("halt", J::Num(n as f64)).with("budget", J::Num(zr.budget as f64)));
            }
            zr.phase = Phase::Idle;
            return false;
        }
        let ev = match zr.pulse.pop() {
            Some(e) => e,
            None => {
                zr.phase = Phase::Idle;
                return false;
            }
        };
        let g = zr.rt.g.as_mut().unwrap();
        let i = match g.idx(ev.sphere) {
            Some(i) => i,
            None => return true,
        };
        zr.events += 1;
        self.total_events += 1;
        g.cur = Some(i);
        g.emitted.clear();
        g.given.clear();
        g.spheres[i].error = None;
        let notch = g.spheres[i].notch;
        let kind = g.spheres[i].kind;
        let body = g.spheres[i].body;
        zr.rt.frames.clear();
        // a machine does its work instead of a body: a writer turns the line into items
        if g.spheres[i].machine != 0 {
            if g.spheres[i].machine == crate::machine::M_WRITER {
                crate::machine::write(&mut g.spheres[i], &ev.line);
            }
            zr.rt.args = Vec::new();
            zr.cur = Some(ev);
            return true;
        }
        zr.rt.args = ev.line.clone();
        zr.cur = Some(ev);
        // a port into a scope converts on the scope's side, in the scope's words
        let side = g.spheres[i].side;
        let to = g.spheres[i].zone_to as usize;
        if kind == K_GATE && side == 1 && self.spec.zones.get(to).map_or(false, |z| z.kind == "scope") {
            return true;
        }
        let r = zr.rt.guarded(|rt| rt.call(body));
        if let Some(Status::Error(e)) = r {
            self.finish_event(z, Some(e));
            return true;
        }
        if kind == K_RATCHET {
            let d = zr.rt.fs.len().saturating_sub(1);
            zr.rt.set_local_num(":i", d, notch as f64);
        }
        true
    }

    /// Ends the running event: routes the remaining line by the sphere's kind.
    fn finish_event(&mut self, z: usize, err: Option<String>) {
        let ev = match self.zones[z].cur.take() {
            Some(e) => e,
            None => return,
        };
        if let Some(e) = err.clone() {
            self.zones[z].cur = Some(ev.clone());
            self.error(z, e);
            self.zones[z].cur = None;
        }
        let multi = self.spec.zones.len() > 1;
        let zname = self.zname(z);
        let line = std::mem::take(&mut self.zones[z].rt.args);
        let (i, kind, sid, port, emitted, given, gate_side, gate_to, gate_name) = {
            let g = self.zones[z].rt.g.as_mut().unwrap();
            let i = match g.idx(ev.sphere) {
                Some(i) => i,
                None => return,
            };
            g.cur = None;
            let emitted = std::mem::take(&mut g.emitted);
            let given = std::mem::take(&mut g.given);
            let s = &mut g.spheres[i];
            let mut port = P_OUT;
            if err.is_none() && s.kind == K_RATCHET {
                let n = s.param("n", 0.0).max(0.0).floor() as u64;
                if s.notch < n {
                    s.notch += 1;
                    port = P_LOOP;
                } else {
                    s.notch = 0;
                    port = P_EXIT;
                }
            }
            if s.kind == K_OUT && err.is_none() {
                s.shown = Some(trunc(&line.iter().map(|v| v.text().into_owned()).collect::<Vec<_>>().join(" "), 200));
            }
            (i, s.kind, s.id, port, emitted, given, s.side, s.zone_to as usize, s.name.clone())
        };
        let mut tr = J::obj()
            .with("n", J::Num(self.total_events as f64))
            .with("s", sid.into())
            .with("in", line_json(&ev.line))
            .with("out", line_json(&line))
            .with("port", PORT_NAMES[port as usize].into());
        if ev.via != 0 {
            tr.set("via", ev.via.into());
        }
        if let Some(e) = &err {
            tr.set("err", e.clone().into());
        }
        if multi {
            tr.set("zone", zname.into());
        }
        self.trace.push(tr);
        if err.is_some() || kind == K_OUT {
            return;
        }
        if kind == K_GATE && gate_side != 0 {
            self.cross(z, &gate_name, gate_side, gate_to, line, given);
            return;
        }
        let zr = &mut self.zones[z];
        let outs = zr.rt.g.as_ref().unwrap().live_out(i, port);
        let mut lines: Vec<Vec<Val>> = emitted;
        if !line.is_empty() {
            lines.push(line);
        }
        for l in lines.into_iter().rev() {
            for (lid, to) in outs.iter().rev() {
                zr.pulse.push(Event { sphere: *to, line: l.clone(), via: *lid });
            }
        }
    }

    /// A line at a gate: into the zone on the other side, as a crossing or a call.
    fn cross(&mut self, z: usize, gate: &str, side: u8, to: usize, line: Vec<Val>, given_names: Vec<String>) {
        // an rpc zone collects what reaches its gate during a call: that is the return
        if side == 2 {
            if let Some(sv) = self.zones[z].serving.as_mut() {
                if sv.gate == gate {
                    sv.ret.push(texts(&line));
                    return;
                }
            }
        }
        for n in &given_names {
            if self.zones[z].held.iter().any(|h| &h.0 == n && matches!(h.1.as_str(), "ref" | "mut")) {
                self.error(z, format!("borrowed: {}", n));
                return;
            }
        }
        let mut given = Vec::new();
        for n in given_names {
            given.push((n.clone(), take_stored(&mut self.zones[z].rt, &n, true)));
        }
        if line.is_empty() && given.is_empty() {
            return;
        }
        if self.zones[to].phase == Phase::Unspawned {
            if let Err(e) = self.spawn(to) {
                self.error(z, e);
                return;
            }
        }
        let rpc = side == 1 && self.spec.zones[to].kind == "rpc";
        if rpc {
            let chain = self.chain_of(z);
            if chain.contains(&(to as u32)) {
                let e = format!("rpc reentry: {}", self.zname(to));
                self.error(z, e);
                return;
            }
            let id = self.next_call;
            self.next_call += 1;
            let gi = self.zones[z].rt.g.as_ref().unwrap().named(gate).unwrap_or(0);
            self.zones[z].wait = Some(ZWait::Call(id, gi));
            self.send(z, to, Msg::Call { id, gate: gate.to_string(), line: texts(&line), given, chain });
        } else {
            self.send(z, to, Msg::Line { gate: gate.to_string(), line: texts(&line), given });
        }
    }

    /// Requests a running body made with spawn, join and calls in code.
    fn requests(&mut self, z: usize) {
        let reqs = match self.zones[z].rt.g.as_mut() {
            Some(g) => std::mem::take(&mut g.requests),
            None => return,
        };
        for (w, arg) in reqs {
            match w.as_str() {
                "spawn" => match self.zone_named(&arg) {
                    Some(t) => {
                        if let Err(e) = self.spawn(t) {
                            self.fail_running(z, e);
                        }
                    }
                    None => self.fail_running(z, format!("spawn: no zone named {}", arg)),
                },
                "join" => match self.zone_named(&arg) {
                    Some(t) if self.is_worker() && !self.local(t) => self.fail_running(z, format!("join: {} runs in another partition", arg)),
                    Some(t) if t != z && t != 0 => self.zones[z].wait = Some(ZWait::Join(t)),
                    _ => self.fail_running(z, format!("join: no zone named {} to join", arg)),
                },
                "call" => {
                    let to = {
                        let g = self.zones[z].rt.g.as_ref().unwrap();
                        match g.named(&arg) {
                            Some(i) => g.spheres[i].zone_to as usize,
                            None => {
                                self.fail_running(z, format!("{}.: no gate", arg));
                                continue;
                            }
                        }
                    };
                    if self.zones[to].phase == Phase::Unspawned {
                        if let Err(e) = self.spawn(to) {
                            self.fail_running(z, e);
                            continue;
                        }
                    }
                    let chain = self.chain_of(z);
                    if chain.contains(&(to as u32)) {
                        let e = format!("rpc reentry: {}", self.zname(to));
                        self.fail_running(z, e);
                        continue;
                    }
                    let id = self.next_call;
                    self.next_call += 1;
                    let line = texts(&self.zones[z].rt.args);
                    self.zones[z].rt.args.clear();
                    self.zones[z].wait = Some(ZWait::CallCode(id));
                    self.send(z, to, Msg::Call { id, gate: arg, line, given: Vec::new(), chain });
                }
                _ => {}
            }
        }
    }

    /// An error outside the machine, in the running body or main unit.
    pub(crate) fn fail_running(&mut self, z: usize, e: String) {
        self.zones[z].rt.reset_frames();
        self.zones[z].wait = None;
        if self.zones[z].cur.is_some() {
            self.finish_event(z, Some(e));
        } else {
            self.error(z, e);
            if self.zones[z].phase == Phase::Main {
                self.zones[z].phase = Phase::Failed;
            }
        }
    }

    /// Resolves waits that are ready: joins of settled zones.
    fn resolve_waits(&mut self) -> bool {
        let mut any = false;
        for z in 0..self.zones.len() {
            if let Some(ZWait::Join(t)) = self.zones[z].wait.clone() {
                if self.zones[t].phase == Phase::Unspawned || self.settled(t) {
                    self.join(t);
                    let zr = &mut self.zones[z];
                    zr.wait = None;
                    if matches!(zr.rt.fs.last().map(|f| &f.body), Some(Body::Await(_))) {
                        zr.rt.fs.pop();
                    }
                    any = true;
                }
            }
        }
        any
    }

    /// One unit of work for one zone: a machine slice, starting an event, or a message.
    /// Returns (progressed, a reason to stop the run).
    fn work(&mut self, z: usize, budget: u64, kind: StepKind) -> (bool, Option<&'static str>) {
        if !self.local(z) {
            return (false, None);
        }
        let phase = self.zones[z].phase;
        if matches!(phase, Phase::Unspawned | Phase::Failed | Phase::Exited) {
            return (false, None);
        }
        if self.zones[z].wait.is_some() {
            return (false, None);
        }
        let running = phase == Phase::Main || self.zones[z].cur.is_some() || !self.zones[z].rt.fs.is_empty();
        if !running && self.zones[z].patch.is_some() {
            self.apply_patch(z);
        }
        if !running {
            if !self.zones[z].pulse.is_empty() {
                if self.zones[z].phase == Phase::Idle {
                    self.zones[z].phase = Phase::Pulse;
                }
                let started = self.start_event(z);
                return (started, if started && kind == StepKind::Token { Some("step") } else { None });
            }
            if self.zones[z].phase == Phase::Pulse {
                self.zones[z].phase = Phase::Idle;
            }
            if self.zones[z].serving.is_some() && self.quiet(z) {
                self.finish_call(z);
                return (true, None);
            }
            if let Some((from, m)) = self.take_message(z) {
                self.handle(z, from, m);
                return (true, None);
            }
            return (false, None);
        }
        let st = self.zones[z].rt.run(budget);
        if !matches!(st, Status::Wait) && self.zones[z].rt.g.as_ref().map_or(false, |g| !g.requests.is_empty()) {
            self.requests(z);
        }
        match st {
            Status::Done => {
                if phase == Phase::Main && self.zones[z].cur.is_none() {
                    self.fire_sources(z);
                    let scopes: Vec<usize> = (0..self.zones.len()).filter(|&c| c != z && self.spec.zones[c].parent == Some(z) && self.spec.zones[c].kind == "scope" && self.zones[c].phase == Phase::Unspawned && self.local(c)).collect();
                    for c in scopes {
                        if let Err(e) = self.spawn(c) {
                            self.error(c, e);
                        }
                    }
                } else {
                    self.finish_event(z, None);
                    if kind == StepKind::Event {
                        return (true, Some("step"));
                    }
                }
            }
            Status::Ran => {}
            Status::Wait => {
                self.requests(z);
                if self.zones[z].wait.is_none() && matches!(self.zones[z].rt.wait, Some(Wait::Recv(_))) {
                    // a worker has not answered yet: let the other zones run meanwhile
                    return (false, None);
                }
            }
            Status::Break => return (true, Some("break")),
            Status::Error(e) => {
                if phase == Phase::Main && self.zones[z].cur.is_none() {
                    self.error(z, e);
                    self.zones[z].phase = Phase::Failed;
                } else {
                    self.finish_event(z, Some(e));
                }
                if kind != StepKind::All {
                    return (true, Some("step"));
                }
            }
            Status::Exit(c) => {
                self.exit = Some(c);
                for zz in self.zones.iter_mut() {
                    zz.phase = Phase::Exited;
                }
                self.halt_all();
                return (true, Some("exit"));
            }
        }
        (true, if kind == StepKind::Token { Some("step") } else { None })
    }

    /// Lockstep: a zone does one whole thing per round.
    fn work_round(&mut self, z: usize, budget: u64) -> (bool, Option<&'static str>) {
        let mut progressed = false;
        loop {
            let (p, stop) = self.work(z, budget, StepKind::All);
            if !p {
                break;
            }
            progressed = true;
            if stop.is_some() {
                return (true, stop);
            }
            let running = self.zones[z].cur.is_some() || self.zones[z].phase == Phase::Main || !self.zones[z].rt.fs.is_empty();
            if !running || self.zones[z].wait.is_some() {
                break;
            }
        }
        (progressed, None)
    }

    /// Runs up to `ops` machine operations across zones, in the way `kind` asks.
    pub fn run(&mut self, ops: u64, kind: StepKind) -> &'static str {
        let mut left = ops;
        let start = (self.zones[0].rt.site.0, self.col_of(0), self.total_events);
        loop {
            let mut progressed = false;
            if self.parts.is_some() {
                progressed = self.pump(None);
                if self.exit.is_some() {
                    return "exit";
                }
                if self.parts.as_ref().map_or(false, |p| p.halt) {
                    return "idle";
                }
            }
            progressed |= self.resolve_waits();
            for z in 0..self.zones.len() {
                if left == 0 {
                    return "ran";
                }
                let budget = match kind {
                    StepKind::Token | StepKind::Column => 1,
                    _ => left.min(1 << 14),
                };
                let before = self.zones[z].rt.ops;
                let (p, stop) = if self.lockstep && kind == StepKind::All { self.work_round(z, budget) } else { self.work(z, budget, kind) };
                if p {
                    progressed = true;
                    let used = self.zones.get(z).map_or(1, |zz| zz.rt.ops.wrapping_sub(before)).max(1);
                    left = left.saturating_sub(used.min(budget.max(1)));
                }
                if let Some(r) = stop {
                    return r;
                }
                if kind == StepKind::Column && p {
                    let now = (self.zones[0].rt.site.0, self.col_of(0), self.total_events);
                    if now != start {
                        return "step";
                    }
                }
            }
            if self.lockstep {
                self.rounds += 1;
                if !self.outbox.is_empty() {
                    self.flush_outbox();
                    progressed = true;
                }
            }
            if !progressed && self.resolve_waits() {
                continue;
            }
            if !progressed {
                if self.zones.iter().any(|z| matches!(z.rt.wait, Some(Wait::Recv(_))) && !z.rt.fs.is_empty()) {
                    if self.parts.is_some() {
                        self.pump(Some(std::time::Duration::from_micros(200)));
                    } else {
                        std::thread::sleep(std::time::Duration::from_micros(200));
                    }
                    continue;
                }
                if self.is_worker() {
                    return "idle";
                }
                if !self.parts_still() {
                    // partitions are working: wait for what they send
                    self.pump(Some(std::time::Duration::from_millis(50)));
                    continue;
                }
                if self.all_settled() && !self.has_clock() && !self.serving {
                    self.end_zones();
                }
                return "idle";
            }
        }
    }

    /// The program ends: every live zone is joined, innermost first.
    pub fn end_zones(&mut self) {
        let parts = self.parts.as_ref().map(|p| (p.owner.clone(), p.roots.clone()));
        let mut order: Vec<usize> = (1..self.zones.len())
            .filter(|&z| self.zones[z].phase != Phase::Unspawned)
            .filter(|&z| match &parts {
                // inside a partition, its root's join ends the rest
                Some((owner, roots)) => owner[z] == 0 || roots[owner[z] as usize] == z,
                None => true,
            })
            .collect();
        order.sort_by_key(|&z| (std::cmp::Reverse(self.spec.depth(z)), z));
        for z in order {
            self.join(z);
        }
    }

    fn all_settled(&self) -> bool {
        (0..self.zones.len()).all(|z| self.settled(z)) && self.outbox.is_empty()
    }

    /// Which line of its unit the machine is on: columns are lines.
    fn col_of(&mut self, z: usize) -> u32 {
        let (unit, pc) = self.zones[z].rt.site;
        if unit == NO_UNIT {
            return 0;
        }
        let text = match self.spec.units.get(unit as usize) {
            Some(u) => u.text.clone(),
            None => return 0,
        };
        let code = match self.zones[z].rt.code_of(&text) {
            Ok(c) => c,
            Err(_) => return 0,
        };
        let off = code.spans.get(pc as usize).copied().unwrap_or(0) as usize;
        text.chars().take(off).filter(|&c| c == '\n').count() as u32
    }

    pub fn idle(&self) -> bool {
        self.all_settled()
    }

    /// Runs until every zone is settled (the clock aside), or the op limit.
    pub fn run_to_end(&mut self, ops: u64) -> &'static str {
        let mut left = ops;
        loop {
            let chunk = left.min(1 << 20);
            let r = self.run(chunk, StepKind::All);
            left = left.saturating_sub(chunk);
            if r != "ran" || left == 0 {
                self.flush();
                return r;
            }
        }
    }

    pub fn flush(&mut self) {
        for z in self.zones.iter_mut() {
            z.rt.flush();
        }
    }

    pub fn output(&self) -> String {
        String::from_utf8_lossy(&self.out.lock().unwrap()).into_owned()
    }

    /// What changed since the last report, for a viewer.
    pub fn report(&mut self, watch: bool) -> J {
        let out = {
            let b = self.out.lock().unwrap();
            let s = String::from_utf8_lossy(&b[self.out_sent.min(b.len())..]).into_owned();
            self.out_sent = b.len();
            s
        };
        let trace: Vec<J> = self.trace[self.trace_sent..].to_vec();
        self.trace_sent = self.trace.len();
        let errors: Vec<J> = self.errors[self.errors_sent..].to_vec();
        self.errors_sent = self.errors.len();
        let mut zones = Vec::new();
        for z in 0..self.zones.len() {
            let brick = self.site_brick(z);
            let zr = &self.zones[z];
            let phase = if zr.wait.is_some() { "waiting" } else { zr.phase.name() };
            let mut o = J::obj()
                .with("name", self.spec.zones[zr.spec].name.clone().into())
                .with("kind", self.spec.zones[zr.spec].kind.clone().into())
                .with("phase", phase.into())
                .with("events", J::Num(zr.events as f64))
                .with("queued", J::Num(zr.pulse.len() as f64))
                .with("inbox", J::Num(zr.inbox.len() as f64));
            if let Some(b) = brick {
                o.set("brick", b.into());
            }
            if let Some(e) = &zr.cur {
                o.set("sphere", e.sphere.into());
            }
            if !zr.held.is_empty() {
                o.set("held", J::Arr(zr.held.iter().map(|(n, m)| J::Arr(vec![n.clone().into(), m.clone().into()])).collect()));
            }
            if watch {
                let a = &zr.rt.args;
                let n = a.len();
                let mut v: Vec<J> = Vec::new();
                for (i, x) in a.iter().enumerate() {
                    if n > 64 && i >= 32 && i < n - 32 {
                        if i == 32 {
                            v.push(J::Str(format!("… {} more …", n - 64)));
                        }
                        continue;
                    }
                    v.push(J::Str(trunc(&x.text(), 256)));
                }
                o.set("line", J::Arr(v));
                o.set("frames", J::Arr(zr.rt.frames.iter().map(|f| line_json(f)).collect()));
                o.set("ow", zr.rt.ow.into());
                o.set("depth", J::Num(zr.rt.fs.len() as f64));
            }
            if let Some(g) = zr.rt.g.as_ref() {
                o.set("graph", g.to_json());
            }
            let ds = datum_values(&mut self.zones[z].rt);
            if !ds.is_empty() {
                o.set("datums", J::Arr(ds));
            }
            zones.push(o);
        }
        J::obj()
            .with("out", out.into())
            .with("trace", J::Arr(trace))
            .with("errors", J::Arr(errors))
            .with("zones", J::Arr(zones))
            .with("idle", self.idle().into())
            .with("halted", self.halted.into())
            .with("exit", self.exit.map(|c| J::Num(c as f64)).unwrap_or(J::Null))
    }

    /// A name's value in a zone, for tests and the inspector: (mem, number).
    pub fn peek(&mut self, zone: &str, name: &str) -> (Option<String>, Option<f64>) {
        let Some(z) = self.zone_named(zone) else { return (None, None) };
        let rt = &mut self.zones[z].rt;
        let k = rt.it.sym(name);
        let i = k.0 as usize;
        (rt.mem.get(i).and_then(|v| v.as_ref()).map(|v| v.text().into_owned()), rt.nums.get(i).copied().flatten())
    }
}

/// Sets a zone's datum names from the world: all of them at the start, and on a live edit
/// those whose record changed, so values code wrote stay where the player did not touch.
fn seed_datums(rt: &mut Rt, ds: &[GDatum], was: Option<&[GDatum]>) {
    for d in ds {
        let old = was.and_then(|w| w.iter().find(|o| o.id == d.id));
        if let Some(o) = old {
            if o.name == d.name && o.value == d.value && o.parts == d.parts {
                continue;
            }
        }
        let k = rt.it.sym(&d.name);
        rt.nset(k, d.value);
        for (i, p) in PART_NAMES.iter().enumerate() {
            let k = rt.it.sym(&format!("{}/{}", d.name, p));
            let x = d.parts.iter().find(|q| q.0 == *p).map(|q| q.1).unwrap_or(PART_DEFAULT[i]);
            if d.parts.iter().any(|q| q.0 == *p) || old.map_or(false, |o| o.parts.iter().any(|q| q.0 == *p)) || rt.nums.get(k.0 as usize).copied().flatten().is_some() {
                rt.nset(k, x);
            }
        }
    }
}

/// A zone's datums as code has left them: value and present parts, for the studio.
fn datum_values(rt: &mut Rt) -> Vec<J> {
    let Some(g) = rt.g.as_ref() else { return Vec::new() };
    let ds: Vec<(u32, String)> = g.datums.iter().map(|d| (d.id, d.name.clone())).collect();
    let mut out = Vec::new();
    for (id, name) in ds {
        let k = rt.it.sym(&name);
        let v = rt.nums.get(k.0 as usize).copied().flatten().unwrap_or(0.0);
        let v = if v.is_finite() { (v.floor() as i64).rem_euclid(1 << 24) as f64 } else { 0.0 };
        let mut parts = J::obj();
        for (i, p) in PART_NAMES.iter().enumerate() {
            let k = rt.it.sym(&format!("{}/{}", name, p));
            if let Some(x) = rt.nums.get(k.0 as usize).copied().flatten() {
                if x != PART_DEFAULT[i] && x.is_finite() {
                    parts.set(p, J::Num(x));
                }
            }
        }
        out.push(J::obj().with("id", id.into()).with("value", J::Num(v)).with("parts", parts));
    }
    out
}

/// Runs a spec headless to the end and returns its output: the command line and tests.
pub fn run_headless(spec: Spec, prog: &'static Program, ticks: u64) -> (String, Session) {
    let mut s = Session::new(spec, prog);
    s.run_to_end(u64::MAX);
    for _ in 0..ticks {
        s.tick();
        s.run_to_end(u64::MAX);
    }
    (s.output(), s)
}
