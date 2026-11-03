// Tracks and machines: items ride tracks a fixed number of cells per tick, queueing
// without overlap; a machine standing past a track's end takes its front item, works on
// it, and puts what comes out onto the tracks that start beside it. Part of every
// compiled program, so it depends only on the machine, the graph and json.
use crate::graph::*;
use crate::json::J;
use crate::rt::*;
use std::collections::VecDeque;

pub const MACHINE_NAMES: &[&str] = &["conveyor", "rotator", "splitter", "merger", "cutter", "stacker", "painter", "reader", "writer", "bin"];
pub const M_ROTATOR: u8 = 2;
pub const M_SPLITTER: u8 = 3;
pub const M_CUTTER: u8 = 5;
pub const M_STACKER: u8 = 6;
pub const M_PAINTER: u8 = 7;
pub const M_READER: u8 = 8;
pub const M_WRITER: u8 = 9;
pub const M_BIN: u8 = 10;

/// The machine a body word names, 1-based; 0 is none.
pub fn machine_of(word: &str) -> u8 {
    MACHINE_NAMES.iter().position(|m| *m == word).map(|i| i as u8 + 1).unwrap_or(0)
}

/// What rides a track: a datum cube's 24 bits, one byte bar of a channel (0 red, 1 green,
/// 2 blue), or a value as text.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Cube(u32),
    Bar(u8, u8),
    Val(String),
}

impl Item {
    pub fn to_json(&self) -> J {
        match self {
            Item::Cube(v) => J::obj().with("k", "cube".into()).with("v", J::Num(*v as f64)),
            Item::Bar(c, b) => J::obj().with("k", "bar".into()).with("ch", J::Num(*c as f64)).with("v", J::Num(*b as f64)),
            Item::Val(s) => J::obj().with("k", "val".into()).with("t", s.clone().into()),
        }
    }
    /// The item as a value on a wire.
    pub fn value(&self) -> Val {
        match self {
            Item::Cube(v) => Val::N(*v as f64),
            Item::Bar(_, b) => Val::N(*b as f64),
            Item::Val(s) => Val::of(s.clone()),
        }
    }
    /// A value from a wire as an item: whole numbers in 24 bits are cubes, the rest text.
    pub fn of(v: &Val) -> Item {
        if let Val::N(x) = v {
            if *x >= 0.0 && *x <= 16777215.0 && x.fract() == 0.0 {
                return Item::Cube(*x as u32);
            }
        }
        Item::Val(v.text().into_owned())
    }
}

#[derive(Clone, Debug)]
pub struct GItem {
    pub id: u32,
    pub item: Item,
    /// Index of the cell it stands on.
    pub pos: u32,
}

#[derive(Clone, Debug)]
pub struct GTrack {
    pub id: u32,
    pub name: String,
    pub cells: Vec<[i32; 3]>,
    pub speed: u32,
    /// Front first.
    pub items: Vec<GItem>,
}

impl GTrack {
    pub fn from_json(t: &J) -> GTrack {
        GTrack {
            id: t.get("id").u(0),
            name: t.get("name").s(""),
            cells: t.get("cells").arr().iter().map(|c| c.i3([0, 0, 0])).collect(),
            speed: t.get("speed").u(1).max(1),
            items: Vec::new(),
        }
    }
    /// The cell past its end, where what it carries goes next.
    pub fn next_cell(&self) -> Option<[i32; 3]> {
        let n = self.cells.len();
        if n < 2 {
            return None;
        }
        let (a, b) = (self.cells[n - 2], self.cells[n - 1]);
        Some([2 * b[0] - a[0], 2 * b[1] - a[1], 2 * b[2] - a[2]])
    }
    fn start_free(&self) -> bool {
        !self.items.iter().any(|i| i.pos == 0)
    }
}

fn beside(a: [i32; 3], b: [i32; 3]) -> bool {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs() == 1
}

/// Turns a cube a quarter about +y: the faces +x, +z, −x, −z go round, so red and blue
/// trade places, the low nibble of red becoming the high nibble of blue.
pub fn rotate_cube(v: u32) -> u32 {
    let (r, g, b) = ((v >> 16) & 255, (v >> 8) & 255, v & 255);
    let (rh, rl, bh, bl) = (r >> 4, r & 15, b >> 4, b & 15);
    let nb = (rh << 4) | rl;
    let nr = (bl << 4) | bh;
    (nr << 16) | (g << 8) | nb
}

/// Whether a machine takes this item now: it has room, and a stacker only takes a
/// channel it lacks.
pub fn accepts(s: &GSphere, it: &Item) -> bool {
    match s.machine {
        M_BIN | M_READER => true,
        M_STACKER => match it {
            Item::Bar(c, _) => s.outq.len() < 3 && !s.hold.iter().any(|h| matches!(h, Item::Bar(x, _) if x == c)),
            _ => s.outq.len() < 3,
        },
        0 => false,
        _ => s.outq.len() < 3,
    }
}

/// A machine's work on an item it took; a reader's value comes back as a line to emit.
pub fn work(s: &mut GSphere, it: Item) -> Option<Vec<Val>> {
    match (s.machine, it) {
        (M_BIN, _) => {}
        (M_READER, it) => return Some(vec![it.value()]),
        (M_ROTATOR, Item::Cube(v)) => s.outq.push_back(Item::Cube(rotate_cube(v))),
        (M_CUTTER, Item::Cube(v)) => {
            for c in 0..3u8 {
                s.outq.push_back(Item::Bar(c, ((v >> (16 - 8 * c as u32)) & 255) as u8));
            }
        }
        (M_STACKER, Item::Bar(c, b)) => {
            s.hold.push(Item::Bar(c, b));
            if s.hold.len() == 3 {
                let mut v = 0u32;
                for h in s.hold.drain(..) {
                    if let Item::Bar(c, b) = h {
                        v |= (b as u32) << (16 - 8 * c as u32);
                    }
                }
                s.outq.push_back(Item::Cube(v));
            }
        }
        (M_PAINTER, Item::Cube(v)) => {
            let and = s.param("and", 16777215.0).max(0.0) as u32;
            let or = s.param("or", 0.0).max(0.0) as u32;
            s.outq.push_back(Item::Cube((v & and | or) & 0xFF_FFFF));
        }
        (_, it) => s.outq.push_back(it),
    }
    None
}

/// One tick of every track and machine in a graph; returns the lines readers put out.
pub fn tick(g: &mut Graph) -> Vec<(u32, Vec<Val>)> {
    let mut emits = Vec::new();
    // items advance, front first, each stopping a cell behind the one ahead
    for t in g.tracks.iter_mut() {
        let end = t.cells.len() as u32 - 1;
        let mut limit = end;
        for it in t.items.iter_mut() {
            it.pos = (it.pos + t.speed).min(limit).max(it.pos);
            limit = it.pos.saturating_sub(1);
        }
    }
    // a front item at the end goes to the machine or the track past it
    for ti in 0..g.tracks.len() {
        let t = &g.tracks[ti];
        let Some(front) = t.items.first() else { continue };
        if front.pos as usize != t.cells.len() - 1 {
            continue;
        }
        let Some(next) = t.next_cell() else { continue };
        let it = front.item.clone();
        if let Some(si) = g.spheres.iter().position(|s| s.machine != 0 && s.cell == Some(next)) {
            if !accepts(&g.spheres[si], &it) {
                continue;
            }
            g.tracks[ti].items.remove(0);
            if let Some(line) = work(&mut g.spheres[si], it) {
                emits.push((g.spheres[si].id, line));
            }
        } else if let Some(nj) = g.tracks.iter().position(|o| o.id != g.tracks[ti].id && o.cells.first() == Some(&next)) {
            if g.tracks[nj].start_free() {
                let mut gi = g.tracks[ti].items.remove(0);
                gi.pos = 0;
                g.tracks[nj].items.push(gi);
            }
        }
    }
    // machines put out onto the tracks starting beside them, a splitter taking turns
    for si in 0..g.spheres.len() {
        let (machine, cell, empty) = (g.spheres[si].machine, g.spheres[si].cell, g.spheres[si].outq.is_empty());
        let Some(c) = cell else { continue };
        if machine == 0 || empty {
            continue;
        }
        let outs: Vec<usize> = (0..g.tracks.len()).filter(|&j| g.tracks[j].cells.first().map_or(false, |f| beside(*f, c)) && g.tracks[j].next_cell() != Some(c)).collect();
        if outs.is_empty() {
            continue;
        }
        let n = outs.len();
        let first = if machine == M_SPLITTER { g.spheres[si].turn as usize % n } else { 0 };
        for k in 0..n {
            let j = outs[(first + k) % n];
            if g.tracks[j].start_free() {
                let item = g.spheres[si].outq.pop_front().unwrap();
                let id = g.next_item;
                g.next_item += 1;
                g.tracks[j].items.push(GItem { id, item, pos: 0 });
                if machine == M_SPLITTER {
                    g.spheres[si].turn = g.spheres[si].turn.wrapping_add(1);
                }
                break;
            }
        }
    }
    emits
}

/// A writer's line becomes items in its queue.
pub fn write(s: &mut GSphere, line: &[Val]) {
    for v in line {
        s.outq.push_back(Item::of(v));
    }
}

pub fn tracks_json(g: &Graph) -> J {
    J::Arr(
        g.tracks
            .iter()
            .map(|t| {
                J::obj().with("id", t.id.into()).with("name", t.name.clone().into()).with(
                    "items",
                    J::Arr(t.items.iter().map(|i| i.item.to_json().with("id", i.id.into()).with("pos", J::Num(i.pos as f64))).collect()),
                )
            })
            .collect(),
    )
}

pub fn machines_json(g: &Graph) -> J {
    J::Arr(
        g.spheres
            .iter()
            .filter(|s| s.machine != 0 && (!s.hold.is_empty() || !s.outq.is_empty()))
            .map(|s| {
                J::obj()
                    .with("id", s.id.into())
                    .with("hold", J::Arr(s.hold.iter().map(|i| i.to_json()).collect()))
                    .with("queue", J::Arr(s.outq.iter().map(|i| i.to_json()).collect()))
            })
            .collect(),
    )
}

pub type ItemQueue = VecDeque<Item>;
