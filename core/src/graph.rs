// The sphere graph a zone runs: spheres, links, strikes, ratchets and the graph words.
// Part of every compiled program, so it depends only on the machine and json.
use crate::json::J;
use crate::rt::*;
use crate::machine::*;

pub const HOST_WORDS: &[&str] = &["move", "aim", "at", "pos", "hit", "reach", "emit", "self", "spawn", "join", "give", "zone", "host", "inbox", "schema"];

pub const K_SRC: u8 = 0;
pub const K_FN: u8 = 1;
pub const K_RATCHET: u8 = 2;
pub const K_OUT: u8 = 3;
pub const K_GATE: u8 = 4;
pub const KIND_NAMES: &[&str] = &["src", "fn", "ratchet", "out", "gate"];

pub const P_OUT: u8 = 0;
pub const P_LOOP: u8 = 1;
pub const P_EXIT: u8 = 2;
pub const PORT_NAMES: &[&str] = &["out", "loop", "exit"];

pub fn emits(k: u8) -> bool {
    k != K_OUT
}
pub fn accepts(k: u8) -> bool {
    k != K_SRC
}

/// Where a ray from o along unit d first meets the sphere (c, r), evaluated in this exact
/// order so every runtime agrees bit for bit.
pub fn ray_hit(o: [f64; 3], d: [f64; 3], c: [f64; 3], r: f64, reach: f64) -> Option<f64> {
    let m = [o[0] - c[0], o[1] - c[1], o[2] - c[2]];
    let b = m[0] * d[0] + m[1] * d[1] + m[2] * d[2];
    let q = m[0] * m[0] + m[1] * m[1] + m[2] * m[2] - r * r;
    if q > 0.0 && b > 0.0 {
        return None;
    }
    let disc = b * b - q;
    if disc < 0.0 {
        return None;
    }
    let s = disc.sqrt();
    let mut t = -b - s;
    if t < 0.0 {
        t = -b + s;
    }
    if t >= 0.0 && t <= reach {
        Some(t)
    } else {
        None
    }
}

pub fn norm3(a: [f64; 3]) -> [f64; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if l > 1e-12 && l.is_finite() {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        [1.0, 0.0, 0.0]
    }
}

#[derive(Clone, Debug)]
pub struct GSphere {
    pub id: u32,
    pub name: String,
    pub kind: u8,
    pub pos: [f64; 3],
    pub r: f64,
    pub aim: [f64; 3],
    pub reach: f64,
    pub ray: bool,
    pub params: Vec<(String, f64)>,
    pub muted: Vec<u32>,
    pub body: Sym,
    /// Which side of this zone a gate belongs to: 0 for an ordinary sphere, 1 outer, 2 inner.
    pub side: u8,
    pub zone_to: u32,
    /// A gate into an rpc zone, seen from outside: `name.` calls through it.
    pub call: bool,
    pub notch: u64,
    pub hit: Option<u32>,
    pub hit_t: f64,
    pub shown: Option<String>,
    pub error: Option<String>,
    pub moved: bool,
    /// In a ticked scope: what the sphere puts out this tick.
    pub state: Vec<Val>,
    /// On the grid: the cell it stands in.
    pub cell: Option<[i32; 3]>,
    /// The machine its body names, 1-based; 0 for none.
    pub machine: u8,
    /// A stacker's bars so far.
    pub hold: Vec<Item>,
    /// Items waiting for room on an outgoing track.
    pub outq: ItemQueue,
    /// A splitter's next turn.
    pub turn: u32,
}

impl GSphere {
    pub fn param(&self, k: &str, d: f64) -> f64 {
        self.params.iter().find(|p| p.0 == k).map(|p| p.1).unwrap_or(d)
    }
}

#[derive(Clone, Debug)]
pub struct GLink {
    pub id: u32,
    pub from: u32,
    pub to: u32,
    pub ray: bool,
    pub port: u8,
}

/// A pulse event: run this sphere's body on this line.
#[derive(Clone, Debug)]
pub struct Event {
    pub sphere: u32,
    pub line: Vec<Val>,
    pub via: u32,
}

/// A datum cube a zone holds: its value and parts live as names in the zone.
#[derive(Clone, Debug)]
pub struct GDatum {
    pub id: u32,
    pub name: String,
    pub value: f64,
    pub parts: Vec<(String, f64)>,
}

pub const PART_NAMES: [&str; 6] = ["sx", "sy", "sz", "rx", "ry", "rz"];
pub const PART_DEFAULT: [f64; 6] = [1.0, 1.0, 1.0, 0.0, 0.0, 0.0];

#[derive(Default)]
pub struct Graph {
    pub datums: Vec<GDatum>,
    pub tracks: Vec<GTrack>,
    pub next_item: u32,
    pub spheres: Vec<GSphere>,
    pub links: Vec<GLink>,
    pub next_link: u32,
    pub zone_name: String,
    pub host: String,
    /// The sphere whose body is running, by index.
    pub cur: Option<usize>,
    pub emitted: Vec<Vec<Val>>,
    pub dirty: bool,
    pub links_changed: bool,
    pub inbox: usize,
    /// Zone requests the scheduler handles: (word, argument).
    pub requests: Vec<(String, String)>,
    pub given: Vec<String>,
}

impl Graph {
    pub fn from_json(t: &J, it: &mut Interner) -> Graph {
        let mut g = Graph::default();
        for s in t.get("spheres").arr() {
            let kind = KIND_NAMES.iter().position(|k| *k == s.get("kind").s("fn")).unwrap_or(1) as u8;
            let name = s.get("name").s("");
            g.spheres.push(GSphere {
                id: s.get("id").u(0),
                body: it.sym(&format!("sphere:{}", name)),
                name,
                kind,
                pos: s.get("pos").v3([0.0; 3]),
                r: s.get("r").f(0.8),
                aim: norm3(s.get("aim").v3([1.0, 0.0, 0.0])),
                reach: s.get("reach").f(60.0),
                ray: s.get("strike").s("sticky") == "ray",
                params: s.get("params").entries().iter().filter_map(|(k, v)| v.num().map(|x| (k.clone(), x))).collect(),
                muted: s.get("muted").arr().iter().map(|x| x.u(0)).collect(),
                side: s.get("side").u(0) as u8,
                zone_to: s.get("zone_to").u(0),
                call: s.get("call").b(false),
                notch: 0,
                hit: None,
                hit_t: 0.0,
                shown: None,
                error: None,
                moved: false,
                state: Vec::new(),
                cell: if s.get("cell").is_null() { None } else { Some(s.get("cell").i3([0, 0, 0])) },
                machine: machine_of(&s.get("machine").s("")),
                hold: Vec::new(),
                outq: ItemQueue::new(),
                turn: 0,
            });
        }
        for l in t.get("links").arr() {
            g.links.push(GLink {
                id: l.get("id").u(0),
                from: l.get("from").u(0),
                to: l.get("to").u(0),
                ray: l.get("mode").s("sticky") == "ray",
                port: PORT_NAMES.iter().position(|p| *p == l.get("port").s("out")).unwrap_or(0) as u8,
            });
        }
        for d in t.get("datums").arr() {
            g.datums.push(GDatum {
                id: d.get("id").u(0),
                name: d.get("name").s(""),
                value: d.get("value").f(0.0),
                parts: d.get("parts").entries().iter().filter_map(|(k, v)| v.num().map(|x| (k.clone(), x))).collect(),
            });
        }
        for k in t.get("tracks").arr() {
            g.tracks.push(GTrack::from_json(k));
        }
        g.next_item = 1;
        g.next_link = t.get("next_link").u(0).max(g.links.iter().map(|l| l.id + 1).max().unwrap_or(1));
        g.zone_name = t.get("zone").s("main");
        g.host = t.get("host").s("local");
        g.refresh();
        g.links_changed = false;
        g
    }

    pub fn idx(&self, id: u32) -> Option<usize> {
        self.spheres.iter().position(|s| s.id == id)
    }
    pub fn named(&self, name: &str) -> Option<usize> {
        self.spheres.iter().position(|s| s.name == name)
    }

    /// The nearest accepting sphere on a sphere's ray. A gate looks only at its own side.
    pub fn hit_of(&self, i: usize) -> Option<(usize, f64)> {
        let s = &self.spheres[i];
        if !emits(s.kind) {
            return None;
        }
        let mut best: Option<(usize, f64)> = None;
        for (j, t) in self.spheres.iter().enumerate() {
            if j == i || !accepts(t.kind) {
                continue;
            }
            if s.kind == K_GATE && t.kind == K_GATE {
                continue;
            }
            if let Some(th) = ray_hit(s.pos, s.aim, t.pos, t.r, s.reach) {
                if best.map_or(true, |b| th < b.1) {
                    best = Some((j, th));
                }
            }
        }
        best
    }

    /// Recomputes every ray; a new strike makes a link in the striking sphere's mode.
    pub fn refresh(&mut self) {
        for i in 0..self.spheres.len() {
            let h = self.hit_of(i);
            let hid = h.map(|(j, _)| self.spheres[j].id);
            let s = &mut self.spheres[i];
            s.hit = hid;
            s.hit_t = h.map(|x| x.1).unwrap_or(0.0);
            s.muted.retain(|m| Some(*m) == hid);
            if let Some(t) = hid {
                let from = s.id;
                let muted = s.muted.contains(&t);
                let ray = s.ray;
                let ratchet = s.kind == K_RATCHET;
                if !muted && !self.links.iter().any(|l| l.from == from && l.to == t) {
                    let port = if !ratchet {
                        P_OUT
                    } else if self.links.iter().any(|l| l.from == from && l.port == P_LOOP) {
                        P_EXIT
                    } else {
                        P_LOOP
                    };
                    let id = self.next_link;
                    self.next_link += 1;
                    self.links.push(GLink { id, from, to: t, ray, port });
                    self.links_changed = true;
                }
            }
        }
        self.dirty = false;
    }

    pub fn live(&self, l: &GLink) -> bool {
        if !l.ray {
            return true;
        }
        self.idx(l.from).map_or(false, |i| self.spheres[i].hit == Some(l.to))
    }

    /// Live links leaving a sphere on a port, lowest id first.
    pub fn live_out(&self, i: usize, port: u8) -> Vec<(u32, u32)> {
        let s = &self.spheres[i];
        let ratchet = s.kind == K_RATCHET;
        let mut v: Vec<(u32, u32)> = self
            .links
            .iter()
            .filter(|l| l.from == s.id && (!ratchet || l.port == port) && self.live(l))
            .map(|l| (l.id, l.to))
            .collect();
        v.sort();
        v
    }

    pub fn to_json(&self) -> J {
        J::obj()
            .with(
                "spheres",
                J::Arr(
                    self.spheres
                        .iter()
                        .map(|s| {
                            let mut o = J::obj()
                                .with("id", s.id.into())
                                .with("name", s.name.clone().into())
                                .with("pos", s.pos.into())
                                .with("aim", s.aim.into())
                                .with("reach", s.reach.into())
                                .with("notch", J::Num(s.notch as f64))
                                .with("hit", s.hit.map(|h| J::Num(h as f64)).unwrap_or(J::Null));
                            if let Some(t) = &s.shown {
                                o.set("shown", t.clone().into());
                            }
                            if let Some(e) = &s.error {
                                o.set("error", e.clone().into());
                            }
                            o
                        })
                        .collect(),
                ),
            )
            .with(
                "links",
                J::Arr(
                    self.links
                        .iter()
                        .map(|l| {
                            J::obj()
                                .with("id", l.id.into())
                                .with("from", l.from.into())
                                .with("to", l.to.into())
                                .with("mode", (if l.ray { "ray" } else { "sticky" }).into())
                                .with("port", PORT_NAMES[l.port as usize].into())
                                .with("live", self.live(l).into())
                        })
                        .collect(),
                ),
            )
            .with("tracks", tracks_json(self))
            .with("machines", machines_json(self))
    }
}

fn need_graph(rt: &mut Rt, w: &str) -> usize {
    match rt.g.as_ref().and_then(|g| g.cur) {
        Some(i) => i,
        None => rt.fail(format!("{}: only inside a sphere body", w)),
    }
}

fn pop_n(rt: &mut Rt, w: &str) -> f64 {
    match rt.args.pop() {
        Some(v) => v.num(),
        None => rt.fail(format!("{}: needs more values on the line", w)),
    }
}

fn pop_t(rt: &mut Rt, w: &str) -> String {
    match rt.args.pop() {
        Some(v) => v.text().into_owned(),
        None => rt.fail(format!("{}: needs a name on the line", w)),
    }
}

/// The graph words: they take their arguments from the top of the line and leave the rest.
pub fn host_word(rt: &mut Rt, h: u8) {
    let w = HOST_WORDS[h as usize];
    match w {
        "move" => {
            let i = need_graph(rt, w);
            let dz = pop_n(rt, w);
            let dy = pop_n(rt, w);
            let dx = pop_n(rt, w);
            let g = rt.g.as_mut().unwrap();
            if g.spheres[i].kind == K_GATE {
                rt.fail("move: a gate does not move".into());
            }
            let s = &mut g.spheres[i];
            s.pos = [s.pos[0] + dx, s.pos[1] + dy, s.pos[2] + dz];
            s.moved = true;
            g.refresh();
        }
        "aim" => {
            let i = need_graph(rt, w);
            let z = pop_n(rt, w);
            let y = pop_n(rt, w);
            let x = pop_n(rt, w);
            let g = rt.g.as_mut().unwrap();
            if g.spheres[i].kind == K_GATE {
                rt.fail("aim: a gate does not turn".into());
            }
            g.spheres[i].aim = norm3([x, y, z]);
            g.spheres[i].moved = true;
            g.refresh();
        }
        "at" => {
            let i = need_graph(rt, w);
            let name = pop_t(rt, w);
            let g = rt.g.as_mut().unwrap();
            let j = match g.named(&name) {
                Some(j) => j,
                None => rt.fail(format!("at: no sphere named '{}' here", name)),
            };
            let g = rt.g.as_mut().unwrap();
            if g.spheres[i].kind == K_GATE {
                rt.fail("at: a gate does not turn".into());
            }
            let (a, b) = (g.spheres[i].pos, g.spheres[j].pos);
            g.spheres[i].aim = norm3([b[0] - a[0], b[1] - a[1], b[2] - a[2]]);
            g.spheres[i].moved = true;
            g.refresh();
        }
        "reach" => {
            let i = need_graph(rt, w);
            let r = pop_n(rt, w);
            if !(r > 0.0) {
                rt.fail("reach: must be positive".into());
            }
            let g = rt.g.as_mut().unwrap();
            g.spheres[i].reach = r;
            g.spheres[i].moved = true;
            g.refresh();
        }
        "pos" => {
            let i = need_graph(rt, w);
            let p = rt.g.as_ref().unwrap().spheres[i].pos;
            for x in p {
                rt.args.push(Val::N(x));
            }
        }
        "hit" => {
            let i = need_graph(rt, w);
            let g = rt.g.as_ref().unwrap();
            let v = match g.spheres[i].hit.and_then(|h| g.idx(h)) {
                Some(j) => Val::of(g.spheres[j].name.clone()),
                None => Val::L("404"),
            };
            rt.args.push(v);
        }
        "self" => {
            let i = need_graph(rt, w);
            let n = rt.g.as_ref().unwrap().spheres[i].name.clone();
            rt.args.push(Val::of(n));
        }
        "emit" => {
            need_graph(rt, w);
            let line = rt.args.clone();
            rt.g.as_mut().unwrap().emitted.push(line);
        }
        "schema" => {
            let name = pop_t(rt, w);
            for (i, p) in PART_NAMES.iter().enumerate() {
                let k = rt.it.sym(&format!("{}/{}", name, p));
                if let Some(x) = rt.nums.get(k.0 as usize).copied().flatten() {
                    if x != PART_DEFAULT[i] {
                        rt.args.push(Val::of(format!("{}/{}", name, p)));
                    }
                }
            }
        }
        "zone" => {
            let n = rt.g.as_ref().unwrap().zone_name.clone();
            rt.args.push(Val::of(n));
        }
        "host" => {
            let n = rt.g.as_ref().unwrap().host.clone();
            rt.args.push(Val::of(n));
        }
        "inbox" => {
            let n = rt.g.as_ref().unwrap().inbox;
            rt.args.push(Val::N(n as f64));
        }
        "spawn" | "join" => {
            let name = pop_t(rt, w);
            rt.g.as_mut().unwrap().requests.push((w.to_string(), name));
            if w == "join" {
                rt.fs.push(Frame { body: Body::Await(1), pc: 0, bound: rt.bound });
            }
        }
        "give" => {
            let name = pop_t(rt, w);
            let g = rt.g.as_mut().unwrap();
            if g.cur.map_or(true, |i| g.spheres[i].kind != K_GATE) {
                rt.fail("give: only in a gate body".into());
            }
            rt.g.as_mut().unwrap().given.push(name);
        }
        _ => rt.fail(format!("{}: not available here", w)),
    }
}

/// `name.` where no script is called name: a sphere's body runs as a plain call.
pub fn call_hook(rt: &mut Rt, k: Sym) -> Option<Option<Frame>> {
    let name = rt.name(k);
    let g = rt.g.as_ref()?;
    let i = g.named(&name)?;
    if g.spheres[i].kind == K_GATE && g.spheres[i].call {
        let bound = rt.bound;
        rt.g.as_mut().unwrap().requests.push(("call".to_string(), name));
        return Some(Some(Frame { body: Body::Await(2), pc: 0, bound }));
    }
    let body = g.spheres[i].body;
    let sc = rt.scripts.get(body.0 as usize).and_then(|x| x.clone());
    match sc {
        Some((Script::U(u), b)) => Some(Some(Frame { body: Body::Ops(u), pc: 0, bound: b })),
        Some((Script::C(f), b)) => Some(Some(Frame { body: Body::Chunk(f), pc: 0, bound: b })),
        _ => None,
    }
}
