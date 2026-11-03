// The world document: spaces, plates, bricks, spheres, links and zones, with edits that
// carry their own inverse. Everything the studio shows is a projection of this.
use crate::json::J;
use crate::rt::{active_form, canon_num, cmd_of, is_ws, SUFFIX};
use std::collections::{BTreeMap, HashMap};

pub type Id = u32;

macro_rules! names {
    ($t:ident { $($v:ident = $s:literal),* $(,)? }) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
        pub enum $t { $($v),* }
        impl $t {
            pub fn name(self) -> &'static str { match self { $($t::$v => $s),* } }
            pub fn of(s: &str) -> Option<$t> { match s { $($s => Some($t::$v),)* _ => None } }
            pub const ALL: &'static [$t] = &[$($t::$v),*];
        }
    };
}

names!(BKind { Lit = "lit", Num = "num", Cmd = "cmd", Esc = "esc", Name = "name", Read = "read", Box = "box", Quote = "quote", Lib = "lib", Geo = "geo", Sock = "sock", Raw = "raw" });
names!(PKind { Main = "main", Script = "script", Shelf = "shelf", Inside = "inside" });
names!(Arrow { Px = "+x", Nx = "-x", Pz = "+z", Nz = "-z" });
names!(SKind { Src = "src", Fn = "fn", Ratchet = "ratchet", Out = "out", Gate = "gate", Mast = "mast", Port = "port", Asm = "asm" });
names!(Strike { Sticky = "sticky", Ray = "ray" });
names!(Port { Out = "out", Loop = "loop", Exit = "exit" });
names!(ZKind { Async = "async", Thread = "thread", Net = "net", Rpc = "rpc", Scope = "scope" });
names!(Mode { Copy = "copy", Move = "move", Ref = "ref", Mut = "mut" });

pub const CAPS: &[&str] = &["", "~", "`", "#", ";", ":", "$", ".", "..", "@"];

impl SKind {
    pub fn emits(self) -> bool {
        !matches!(self, SKind::Out)
    }
    pub fn accepts(self) -> bool {
        !matches!(self, SKind::Src)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Owner {
    World,
    Box(Id),
    Sphere(Id),
    /// The inside of an assembly, by its name; placements are spheres of kind asm.
    Assembly(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Space {
    pub id: Id,
    pub owner: Owner,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plate {
    pub id: Id,
    pub space: Id,
    pub kind: PKind,
    pub name: String,
    /// Corner cell of the plate; columns stand from y + 1.
    pub at: [i32; 3],
    pub w: i32,
    pub d: i32,
    pub arrow: Arrow,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Brick {
    pub id: Id,
    pub space: Id,
    pub cell: [i32; 3],
    pub kind: BKind,
    pub word: String,
    pub cap: String,
    pub cond: String,
    pub note: String,
    /// For a box: the space holding its inside.
    pub inner: Id,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sphere {
    pub id: Id,
    pub name: String,
    pub kind: SKind,
    pub pos: [f64; 3],
    pub r: f64,
    pub aim: [f64; 3],
    pub reach: f64,
    pub strike: Strike,
    pub params: Vec<(String, f64)>,
    pub muted: Vec<Id>,
    /// The space holding its body plate; for an assembly placement, the assembly's inside.
    pub body: Id,
    /// The space it stands in: the world, or an assembly's inside.
    pub space: Id,
    /// On the grid layer: the cell it stands in. A mast's position is its top.
    pub cell: Option<[i32; 3]>,
    /// Where it floated before it was set down; lifting returns it there.
    pub lift: Option<[f64; 3]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub id: Id,
    pub from: Id,
    pub to: Id,
    pub mode: Strike,
    pub port: Port,
    /// Ports of assembly placements at either end, by name; empty for the first one.
    pub from_pin: String,
    pub to_pin: String,
    /// A wire's route over grid cells, for drawing.
    pub route: Vec<[i32; 3]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub id: Id,
    pub name: String,
    pub kind: ZKind,
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub params: Vec<(String, String)>,
    pub captures: Vec<(String, Mode)>,
}

/// A datum cube: 24 bits on its faces, with a transform that is part of its schema.
#[derive(Clone, Debug, PartialEq)]
pub struct Datum {
    pub id: Id,
    pub space: Id,
    pub name: String,
    pub cell: [i32; 3],
    pub value: u32,
    /// Scale along its own axes; 1 1 1 is standard and absent from the schema.
    pub scale: [f64; 3],
    /// Rotation in degrees about x, y, z, applied z then y then x; 0 0 0 is standard.
    pub rot: [f64; 3],
    /// The player's mutations as s text that redoes them, oldest first.
    pub log: Vec<String>,
}

/// A track: a path of cells on the grid that carries items, `speed` cells per tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub id: Id,
    pub space: Id,
    pub name: String,
    pub cells: Vec<[i32; 3]>,
    pub speed: u32,
}

/// The words a device body may be to work as a machine on tracks.
pub const MACHINES: &[&str] = &["conveyor", "rotator", "splitter", "merger", "cutter", "stacker", "painter", "reader", "writer", "bin"];

pub const PART_NAMES: [&str; 6] = ["sx", "sy", "sz", "rx", "ry", "rz"];
pub const PART_DEFAULT: [f64; 6] = [1.0, 1.0, 1.0, 0.0, 0.0, 0.0];

impl Datum {
    pub fn part(&self, i: usize) -> f64 {
        if i < 3 {
            self.scale[i]
        } else {
            self.rot[i - 3]
        }
    }
    pub fn set_part(&mut self, i: usize, x: f64) {
        if i < 3 {
            self.scale[i] = x
        } else {
            self.rot[i - 3] = x
        }
    }
    /// The transform parts that differ from standard, as (name, value).
    pub fn parts(&self) -> Vec<(&'static str, f64)> {
        (0..6).filter(|&i| self.part(i) != PART_DEFAULT[i]).map(|i| (PART_NAMES[i], self.part(i))).collect()
    }
}

/// The bit a tile carries: face 0…5 is +x −x +y −y +z −z, (u, v) its 2 × 2 tile.
/// +x and −x hold the high and low nibble of red, ±y of green, ±z of blue; tile
/// t = 2v + u is the nibble's bit 3 − t, so tile (0, 0) is its highest bit.
pub fn tile_bit(face: usize, u: usize, v: usize) -> u32 {
    let channel = face / 2;
    let high = face % 2 == 0;
    let base = (2 - channel) * 8 + if high { 4 } else { 0 };
    (base + 3 - (v * 2 + u)) as u32
}

pub const FACE_NAMES: [&str; 6] = ["+x", "-x", "+y", "-y", "+z", "-z"];

#[derive(Clone, Debug, PartialEq)]
pub enum Rec {
    Space(Space),
    Plate(Plate),
    Brick(Brick),
    Sphere(Sphere),
    Link(Link),
    Zone(Zone),
    Datum(Datum),
    Track(Track),
}

impl Rec {
    pub fn id(&self) -> Id {
        match self {
            Rec::Space(x) => x.id,
            Rec::Plate(x) => x.id,
            Rec::Brick(x) => x.id,
            Rec::Sphere(x) => x.id,
            Rec::Link(x) => x.id,
            Rec::Zone(x) => x.id,
            Rec::Datum(x) => x.id,
            Rec::Track(x) => x.id,
        }
    }
    pub fn tag(&self) -> &'static str {
        match self {
            Rec::Space(_) => "space",
            Rec::Plate(_) => "plate",
            Rec::Brick(_) => "brick",
            Rec::Sphere(_) => "sphere",
            Rec::Link(_) => "link",
            Rec::Zone(_) => "zone",
            Rec::Datum(_) => "datum",
            Rec::Track(_) => "track",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Edit {
    Put(Rec),
    Del(Id),
    Meta(J),
}

#[derive(Clone, Debug, Default)]
pub struct Applied {
    pub inverse: Vec<Edit>,
    pub put: Vec<Id>,
    pub del: Vec<Id>,
}

#[derive(Clone, Debug)]
pub struct World {
    pub name: String,
    pub next: Id,
    pub spaces: BTreeMap<Id, Space>,
    pub plates: BTreeMap<Id, Plate>,
    pub bricks: BTreeMap<Id, Brick>,
    pub spheres: BTreeMap<Id, Sphere>,
    pub links: BTreeMap<Id, Link>,
    pub zones: BTreeMap<Id, Zone>,
    pub datums: BTreeMap<Id, Datum>,
    pub tracks: BTreeMap<Id, Track>,
    pub params: J,
    pub goal: J,
    pub cells: HashMap<(Id, [i32; 3]), Id>,
    pub version: u64,
}

pub const WORLD: Id = 0;

impl World {
    pub fn new(name: &str) -> World {
        let mut w = World {
            name: name.to_string(),
            next: 1,
            spaces: BTreeMap::new(),
            plates: BTreeMap::new(),
            bricks: BTreeMap::new(),
            spheres: BTreeMap::new(),
            links: BTreeMap::new(),
            zones: BTreeMap::new(),
            datums: BTreeMap::new(),
            tracks: BTreeMap::new(),
            params: J::obj(),
            goal: J::Null,
            cells: HashMap::new(),
            version: 0,
        };
        w.spaces.insert(WORLD, Space { id: WORLD, owner: Owner::World });
        w
    }

    /// A world with one main plate, ready to type on.
    pub fn starter(name: &str) -> World {
        let mut w = World::new(name);
        let id = w.alloc();
        w.plates.insert(id, Plate { id, space: WORLD, kind: PKind::Main, name: "main".into(), at: [0, 0, 0], w: 12, d: 8, arrow: Arrow::Px });
        w
    }

    pub fn alloc(&mut self) -> Id {
        let id = self.next;
        self.next += 1;
        id
    }

    pub fn rec(&self, id: Id) -> Option<Rec> {
        if let Some(x) = self.bricks.get(&id) {
            return Some(Rec::Brick(x.clone()));
        }
        if let Some(x) = self.plates.get(&id) {
            return Some(Rec::Plate(x.clone()));
        }
        if let Some(x) = self.spheres.get(&id) {
            return Some(Rec::Sphere(x.clone()));
        }
        if let Some(x) = self.links.get(&id) {
            return Some(Rec::Link(x.clone()));
        }
        if let Some(x) = self.zones.get(&id) {
            return Some(Rec::Zone(x.clone()));
        }
        if let Some(x) = self.datums.get(&id) {
            return Some(Rec::Datum(x.clone()));
        }
        if let Some(x) = self.tracks.get(&id) {
            return Some(Rec::Track(x.clone()));
        }
        if id != WORLD {
            if let Some(x) = self.spaces.get(&id) {
                return Some(Rec::Space(x.clone()));
            }
        }
        None
    }

    pub fn brick_at(&self, space: Id, cell: [i32; 3]) -> Option<&Brick> {
        self.cells.get(&(space, cell)).and_then(|id| self.bricks.get(id))
    }

    /// The plate a cell stands on: the highest plate in the space whose footprint holds it.
    pub fn plate_under(&self, space: Id, cell: [i32; 3]) -> Option<&Plate> {
        self.plates
            .values()
            .filter(|p| p.space == space && cell[0] >= p.at[0] && cell[0] < p.at[0] + p.w && cell[2] >= p.at[2] && cell[2] < p.at[2] + p.d && cell[1] > p.at[1])
            .max_by_key(|p| (p.at[1], p.id))
    }

    pub fn sphere_named(&self, name: &str) -> Option<&Sphere> {
        self.spheres.values().find(|s| s.name == name)
    }

    pub fn datum_named(&self, name: &str) -> Option<&Datum> {
        self.datums.values().find(|d| d.name == name)
    }

    pub fn datum_at(&self, space: Id, cell: [i32; 3]) -> Option<&Datum> {
        self.datums.values().find(|d| d.space == space && d.cell == cell)
    }

    pub fn track_named(&self, name: &str) -> Option<&Track> {
        self.tracks.values().find(|t| t.name == name)
    }

    pub fn track_at(&self, space: Id, cell: [i32; 3]) -> Option<&Track> {
        self.tracks.values().find(|t| t.space == space && t.cells.contains(&cell))
    }

    pub fn link_between(&self, from: Id, to: Id) -> Option<&Link> {
        self.links.values().find(|l| l.from == from && l.to == to)
    }

    // checks

    pub fn check_rec(&self, r: &Rec) -> Result<(), String> {
        match r {
            Rec::Space(s) => {
                if s.id == WORLD {
                    return Err("space 0 is the world".into());
                }
                Ok(())
            }
            Rec::Plate(p) => {
                if !self.spaces.contains_key(&p.space) {
                    return Err(format!("plate {}: no space {}", p.id, p.space));
                }
                if p.w < 1 || p.d < 1 || p.w > 512 || p.d > 512 {
                    return Err(format!("plate {}: size {}×{} is outside 1…512", p.id, p.w, p.d));
                }
                if p.kind == PKind::Script && !valid_name(&p.name) {
                    return Err(format!("script plate needs a name without spaces, braces or quotes, got '{}'", p.name));
                }
                Ok(())
            }
            Rec::Brick(b) => {
                if !self.spaces.contains_key(&b.space) {
                    return Err(format!("brick {}: no space {}", b.id, b.space));
                }
                if let Some(&o) = self.cells.get(&(b.space, b.cell)) {
                    if o != b.id {
                        return Err(format!("cell {:?} already holds brick {}", b.cell, o));
                    }
                }
                if self.plate_under(b.space, b.cell).is_none() {
                    return Err(format!("cell {:?} is not above a plate", b.cell));
                }
                if let Some(o) = self.spheres.values().find(|o| o.space == b.space && o.cell == Some(b.cell)) {
                    return Err(format!("cell {:?} holds {}", b.cell, o.name));
                }
                if let Some(o) = self.datum_at(b.space, b.cell) {
                    return Err(format!("cell {:?} holds {}", b.cell, o.name));
                }
                check_word(b)?;
                if b.kind == BKind::Box && b.word.is_empty() && !self.spaces.contains_key(&b.inner) {
                    return Err(format!("box {}: its inside space {} is missing", b.id, b.inner));
                }
                Ok(())
            }
            Rec::Sphere(s) => {
                if !valid_name(&s.name) {
                    return Err(format!("sphere name '{}' must be one word without braces or quotes", s.name));
                }
                if let Some(o) = self.sphere_named(&s.name) {
                    if o.id != s.id {
                        return Err(format!("a sphere is already named '{}'", s.name));
                    }
                }
                if !(s.r > 0.0) || !(s.reach > 0.0) || !s.pos.iter().all(|x| x.is_finite()) {
                    return Err(format!("sphere {}: radius and reach must be positive, position finite", s.name));
                }
                if !self.spaces.contains_key(&s.space) {
                    return Err(format!("sphere {}: no space {}", s.name, s.space));
                }
                if let Some(c) = s.cell {
                    if let Some(o) = self.spheres.values().find(|o| o.id != s.id && o.space == s.space && o.cell == Some(c)) {
                        return Err(format!("cell {:?} already holds {}", c, o.name));
                    }
                    if self.cells.contains_key(&(s.space, c)) {
                        return Err(format!("cell {:?} holds a brick", c));
                    }
                    if let Some(o) = self.datum_at(s.space, c) {
                        return Err(format!("cell {:?} holds {}", c, o.name));
                    }
                }
                if s.kind == SKind::Asm && !matches!(self.spaces.get(&s.body).map(|x| &x.owner), Some(Owner::Assembly(_))) {
                    return Err(format!("{}: an assembly placement needs an assembly", s.name));
                }
                if s.kind == SKind::Mast && s.cell.is_none() {
                    return Err(format!("{}: a mast stands on a cell", s.name));
                }
                Ok(())
            }
            Rec::Link(l) => {
                let a = self.spheres.get(&l.from).ok_or_else(|| format!("link {}: no sphere {}", l.id, l.from))?;
                let b = self.spheres.get(&l.to).ok_or_else(|| format!("link {}: no sphere {}", l.id, l.to))?;
                if l.from == l.to {
                    return Err("a sphere does not link to itself".into());
                }
                if a.space != b.space {
                    return Err(format!("{} and {} are in different spaces", a.name, b.name));
                }
                if !a.kind.emits() || !b.kind.accepts() {
                    return Err(format!("{} cannot link to {}: {} spheres do not emit or {} spheres do not accept", a.name, b.name, a.kind.name(), b.kind.name()));
                }
                if let Some(o) = self.link_between(l.from, l.to) {
                    if o.id != l.id {
                        return Err(format!("{} already links to {}", a.name, b.name));
                    }
                }
                Ok(())
            }
            Rec::Zone(z) => {
                if !valid_name(&z.name) {
                    return Err(format!("zone name '{}' must be one word", z.name));
                }
                for i in 0..3 {
                    if z.min[i] > z.max[i] {
                        return Err(format!("zone {}: min is above max", z.name));
                    }
                }
                for o in self.zones.values() {
                    if o.id == z.id {
                        continue;
                    }
                    if o.name == z.name {
                        return Err(format!("a zone is already named '{}'", z.name));
                    }
                    let overlap = (0..3).all(|i| z.min[i] <= o.max[i] && o.min[i] <= z.max[i]);
                    let inside = |a: &Zone, b: &Zone| (0..3).all(|i| a.min[i] >= b.min[i] && a.max[i] <= b.max[i]);
                    if overlap && !inside(z, o) && !inside(o, z) {
                        return Err(format!("zone {} overlaps zone {} without nesting", z.name, o.name));
                    }
                }
                Ok(())
            }
            Rec::Datum(d) => {
                if !datum_name_ok(&d.name) {
                    return Err(format!("datum name '{}' must be letters, digits, _ or -, starting with a letter", d.name));
                }
                if !self.spaces.contains_key(&d.space) {
                    return Err(format!("datum {}: no space {}", d.name, d.space));
                }
                if let Some(o) = self.datum_named(&d.name) {
                    if o.id != d.id {
                        return Err(format!("a datum is already named '{}'", d.name));
                    }
                }
                if let Some(o) = self.datum_at(d.space, d.cell) {
                    if o.id != d.id {
                        return Err(format!("cell {:?} already holds {}", d.cell, o.name));
                    }
                }
                if self.cells.contains_key(&(d.space, d.cell)) {
                    return Err(format!("cell {:?} holds a brick", d.cell));
                }
                if let Some(o) = self.spheres.values().find(|o| o.space == d.space && o.cell == Some(d.cell)) {
                    return Err(format!("cell {:?} holds {}", d.cell, o.name));
                }
                if d.value > 0xFF_FFFF {
                    return Err(format!("{}: a datum holds 24 bits, got {}", d.name, d.value));
                }
                if !d.scale.iter().all(|x| x.is_finite() && *x > 0.0 && *x <= 64.0) {
                    return Err(format!("{}: scale must be above 0 and at most 64", d.name));
                }
                if !d.rot.iter().all(|x| x.is_finite()) {
                    return Err(format!("{}: rotation must be finite", d.name));
                }
                Ok(())
            }
            Rec::Track(t) => {
                if !datum_name_ok(&t.name) {
                    return Err(format!("track name '{}' must be letters, digits, _ or -, starting with a letter", t.name));
                }
                if !self.spaces.contains_key(&t.space) {
                    return Err(format!("track {}: no space {}", t.name, t.space));
                }
                if let Some(o) = self.track_named(&t.name) {
                    if o.id != t.id {
                        return Err(format!("a track is already named '{}'", t.name));
                    }
                }
                if t.cells.is_empty() {
                    return Err(format!("track {} has no cells", t.name));
                }
                if t.speed == 0 || t.speed > 8 {
                    return Err(format!("track {}: speed is 1 to 8 cells a tick, got {}", t.name, t.speed));
                }
                for (i, c) in t.cells.iter().enumerate() {
                    if i > 0 {
                        let p = t.cells[i - 1];
                        if (c[0] - p[0]).abs() + (c[1] - p[1]).abs() + (c[2] - p[2]).abs() != 1 {
                            return Err(format!("track {}: {:?} does not follow {:?}", t.name, c, p));
                        }
                    }
                    if t.cells[..i].contains(c) {
                        return Err(format!("track {} crosses itself at {:?}", t.name, c));
                    }
                    if let Some(o) = self.tracks.values().find(|o| o.id != t.id && o.space == t.space && o.cells.contains(c)) {
                        return Err(format!("cell {:?} already carries {}", c, o.name));
                    }
                    if self.cells.contains_key(&(t.space, *c)) {
                        return Err(format!("cell {:?} holds a brick", c));
                    }
                    if let Some(o) = self.spheres.values().find(|o| o.space == t.space && o.cell == Some(*c)) {
                        return Err(format!("cell {:?} holds {}", c, o.name));
                    }
                }
                Ok(())
            }
        }
    }

    // apply

    fn put_raw(&mut self, r: Rec) -> Option<Rec> {
        let old = self.rec(r.id());
        if let Some(Rec::Brick(b)) = &old {
            self.cells.remove(&(b.space, b.cell));
        }
        match r {
            Rec::Space(x) => {
                self.spaces.insert(x.id, x);
            }
            Rec::Plate(x) => {
                self.plates.insert(x.id, x);
            }
            Rec::Brick(x) => {
                self.cells.insert((x.space, x.cell), x.id);
                self.bricks.insert(x.id, x);
            }
            Rec::Sphere(x) => {
                self.spheres.insert(x.id, x);
            }
            Rec::Link(x) => {
                self.links.insert(x.id, x);
            }
            Rec::Zone(x) => {
                self.zones.insert(x.id, x);
            }
            Rec::Datum(x) => {
                self.datums.insert(x.id, x);
            }
            Rec::Track(x) => {
                self.tracks.insert(x.id, x);
            }
        }
        old
    }

    fn del_raw(&mut self, id: Id) -> Option<Rec> {
        let old = self.rec(id)?;
        match &old {
            Rec::Space(_) => {
                self.spaces.remove(&id);
            }
            Rec::Plate(_) => {
                self.plates.remove(&id);
            }
            Rec::Brick(b) => {
                self.cells.remove(&(b.space, b.cell));
                self.bricks.remove(&id);
            }
            Rec::Sphere(_) => {
                self.spheres.remove(&id);
            }
            Rec::Link(_) => {
                self.links.remove(&id);
            }
            Rec::Zone(_) => {
                self.zones.remove(&id);
            }
            Rec::Datum(_) => {
                self.datums.remove(&id);
            }
            Rec::Track(_) => {
                self.tracks.remove(&id);
            }
        }
        Some(old)
    }

    /// Applies edits in order; on the first failure everything already applied is undone.
    pub fn apply(&mut self, edits: Vec<Edit>) -> Result<Applied, String> {
        self.apply_with(edits, true)
    }

    /// Applies edits that restore a state known to be whole (undo, redo): no checks.
    pub fn apply_raw(&mut self, edits: Vec<Edit>) -> Applied {
        self.apply_with(edits, false).unwrap_or_default()
    }

    fn apply_with(&mut self, edits: Vec<Edit>, check: bool) -> Result<Applied, String> {
        let mut done = Applied::default();
        for e in edits {
            if let Err(err) = self.apply_one(e, &mut done, check) {
                let inv = std::mem::take(&mut done.inverse);
                for e in inv.into_iter().rev() {
                    let _ = self.apply_one(e, &mut Applied::default(), false);
                }
                return Err(err);
            }
        }
        done.inverse.reverse();
        self.version += 1;
        Ok(done)
    }

    fn apply_one(&mut self, e: Edit, out: &mut Applied, check: bool) -> Result<(), String> {
        match e {
            Edit::Put(r) => {
                if check {
                    self.check_rec(&r)?;
                }
                let id = r.id();
                if id >= self.next {
                    self.next = id + 1;
                }
                match self.put_raw(r) {
                    Some(old) => out.inverse.push(Edit::Put(old)),
                    None => out.inverse.push(Edit::Del(id)),
                }
                out.put.push(id);
                out.del.retain(|&x| x != id);
            }
            Edit::Del(id) => {
                if let Some(old) = self.del_raw(id) {
                    out.inverse.push(Edit::Put(old));
                    out.del.push(id);
                    out.put.retain(|&x| x != id);
                }
            }
            Edit::Meta(m) => {
                let old = J::obj().with("name", self.name.clone().into()).with("params", self.params.clone()).with("goal", self.goal.clone());
                if let Some(n) = m.get("name").str() {
                    self.name = n.to_string();
                }
                if m.has("params") {
                    self.params = m.get("params").clone();
                }
                if m.has("goal") {
                    self.goal = m.get("goal").clone();
                }
                out.inverse.push(Edit::Meta(old));
            }
        }
        Ok(())
    }

    // structure

    /// Bricks of a space grouped by the plate they stand on.
    pub fn bricks_by_plate(&self, space: Id) -> BTreeMap<Id, Vec<&Brick>> {
        let plates: Vec<&Plate> = self.plates.values().filter(|p| p.space == space).collect();
        let mut out: BTreeMap<Id, Vec<&Brick>> = BTreeMap::new();
        for p in &plates {
            out.insert(p.id, Vec::new());
        }
        for b in self.bricks.values().filter(|b| b.space == space) {
            let under = plates
                .iter()
                .filter(|p| b.cell[0] >= p.at[0] && b.cell[0] < p.at[0] + p.w && b.cell[2] >= p.at[2] && b.cell[2] < p.at[2] + p.d && b.cell[1] > p.at[1])
                .max_by_key(|p| (p.at[1], p.id));
            if let Some(p) = under {
                out.entry(p.id).or_default().push(b);
            }
        }
        out
    }

    /// The columns of a plate in reading order, each bottom to top.
    pub fn columns(&self, plate: Id) -> Vec<Vec<Id>> {
        let p = match self.plates.get(&plate) {
            Some(p) => p,
            None => return Vec::new(),
        };
        let by = self.bricks_by_plate(p.space);
        let bs = by.get(&plate).cloned().unwrap_or_default();
        columns_of(p, &bs)
    }

    pub fn space_plates(&self, space: Id) -> Vec<&Plate> {
        let mut v: Vec<&Plate> = self.plates.values().filter(|p| p.space == space).collect();
        v.sort_by_key(|p| (p.at[1], p.at[0], p.at[2], p.id));
        v
    }

    /// Everything inside a space, recursively: plates, bricks, inner spaces.
    pub fn space_contents(&self, space: Id, out: &mut Vec<Id>) {
        for p in self.plates.values().filter(|p| p.space == space) {
            out.push(p.id);
        }
        for b in self.bricks.values().filter(|b| b.space == space) {
            out.push(b.id);
            if b.kind == BKind::Box && b.inner != 0 {
                out.push(b.inner);
                self.space_contents(b.inner, out);
            }
        }
    }

    /// The zone a point belongs to: the innermost zone holding it, or None for main.
    pub fn zone_of_point(&self, p: [f64; 3]) -> Option<&Zone> {
        self.zones
            .values()
            .filter(|z| (0..3).all(|i| p[i] >= z.min[i] as f64 && p[i] < (z.max[i] + 1) as f64))
            .min_by_key(|z| (0..3).map(|i| (z.max[i] - z.min[i] + 1) as i64).product::<i64>())
    }
    pub fn zone_of_cell(&self, c: [i32; 3]) -> Option<&Zone> {
        self.zone_of_point([c[0] as f64 + 0.5, c[1] as f64 + 0.5, c[2] as f64 + 0.5])
    }
    /// The zone a plate of the world space belongs to (by its corner cell).
    pub fn zone_of_plate(&self, p: &Plate) -> Option<&Zone> {
        if p.space != WORLD {
            return None;
        }
        self.zone_of_cell([p.at[0], p.at[1] + 1, p.at[2]])
    }
    /// The zone that owns a space: the world space's plates by position, a box by its brick,
    /// a sphere body by its sphere.
    pub fn zone_of_space(&self, space: Id) -> Option<&Zone> {
        match self.spaces.get(&space).map(|s| &s.owner) {
            Some(Owner::Box(b)) => {
                let b = self.bricks.get(b)?;
                if b.space == WORLD {
                    self.zone_of_cell(b.cell)
                } else {
                    self.zone_of_space(b.space)
                }
            }
            Some(Owner::Sphere(s)) => {
                let s = self.spheres.get(s)?;
                if s.kind == SKind::Gate || s.space != WORLD {
                    None
                } else {
                    self.zone_of_point(s.pos)
                }
            }
            _ => None,
        }
    }
    /// The wall a gate sits on: (zone, unit normal pointing into the zone).
    pub fn gate_wall(&self, s: &Sphere) -> Option<(&Zone, [f64; 3])> {
        let mut best: Option<(&Zone, [f64; 3], i64)> = None;
        for z in self.zones.values() {
            for i in 0..3 {
                for (face, sign) in [(z.min[i] as f64, 1.0), ((z.max[i] + 1) as f64, -1.0)] {
                    if (s.pos[i] - face).abs() > 0.25 {
                        continue;
                    }
                    let within = (0..3).all(|j| j == i || (s.pos[j] >= z.min[j] as f64 && s.pos[j] <= (z.max[j] + 1) as f64));
                    if within {
                        let vol: i64 = (0..3).map(|j| (z.max[j] - z.min[j] + 1) as i64).product();
                        if best.map_or(true, |b| vol < b.2) {
                            let mut n = [0.0; 3];
                            n[i] = sign;
                            best = Some((z, n, vol));
                        }
                    }
                }
            }
        }
        best.map(|b| (b.0, b.1))
    }

    pub fn parent_zone(&self, z: &Zone) -> Option<&Zone> {
        self.zones
            .values()
            .filter(|o| o.id != z.id && (0..3).all(|i| z.min[i] >= o.min[i] && z.max[i] <= o.max[i]))
            .min_by_key(|o| (0..3).map(|i| (o.max[i] - o.min[i] + 1) as i64).product::<i64>())
    }

    // text

    pub fn brick_text(&self, b: &Brick) -> String {
        let mut s = TW::default();
        let mut map = Vec::new();
        self.brick_text_into(b, &mut s, &mut map, 0);
        s.s
    }

    /// Appends a brick's text, recording (start, end, brick) in characters for every brick written.
    pub fn brick_text_into(&self, b: &Brick, s: &mut TW, map: &mut Vec<(u32, u32, Id)>, depth: u32) {
        let start = s.n;
        match b.kind {
            BKind::Box => {
                s.push("{");
                if !b.word.is_empty() {
                    s.push(&b.word);
                } else if depth < 64 {
                    self.space_text_into(b.inner, s, map, depth + 1);
                }
                s.push("}");
                s.push(&b.cond);
            }
            _ => s.push(&word_text(b)),
        }
        let end = s.n;
        map.push((start, end, b.id));
    }

    pub fn column_text_into(&self, col: &[Id], s: &mut TW, map: &mut Vec<(u32, u32, Id)>, depth: u32) {
        for (i, id) in col.iter().enumerate() {
            if i > 0 {
                s.push(" ");
            }
            if let Some(b) = self.bricks.get(id) {
                self.brick_text_into(b, s, map, depth);
            }
        }
    }

    pub fn plate_text_into(&self, plate: Id, s: &mut TW, map: &mut Vec<(u32, u32, Id)>, depth: u32) {
        for (i, col) in self.columns(plate).iter().enumerate() {
            if i > 0 {
                s.push("\n");
            }
            self.column_text_into(col, s, map, depth);
        }
    }

    pub fn space_text_into(&self, space: Id, s: &mut TW, map: &mut Vec<(u32, u32, Id)>, depth: u32) {
        let ps = self.space_plates(space);
        for (i, p) in ps.iter().enumerate() {
            if i > 0 {
                s.push("\n");
            }
            self.plate_text_into(p.id, s, map, depth);
        }
    }

    pub fn plate_text(&self, plate: Id) -> String {
        let mut s = TW::default();
        self.plate_text_into(plate, &mut s, &mut Vec::new(), 0);
        s.s
    }

    pub fn space_text(&self, space: Id) -> String {
        let mut s = TW::default();
        self.space_text_into(space, &mut s, &mut Vec::new(), 0);
        s.s
    }
}

/// Text being written, with its length in characters.
#[derive(Default)]
pub struct TW {
    pub s: String,
    pub n: u32,
}

impl TW {
    pub fn push(&mut self, t: &str) {
        self.s.push_str(t);
        self.n += t.chars().count() as u32;
    }
}

pub fn columns_of(p: &Plate, bs: &[&Brick]) -> Vec<Vec<Id>> {
    let mut by: BTreeMap<(i64, i64), Vec<(i32, Id)>> = BTreeMap::new();
    for b in bs {
        let (x, z) = (b.cell[0] as i64, b.cell[2] as i64);
        let key = match p.arrow {
            Arrow::Px => (z, x),
            Arrow::Nx => (-z, -x),
            Arrow::Pz => (-x, z),
            Arrow::Nz => (x, -z),
        };
        by.entry(key).or_default().push((b.cell[1], b.id));
    }
    let mut cols = Vec::new();
    for (_, mut v) in by {
        v.sort();
        let mut cur: Vec<Id> = Vec::new();
        let mut last: Option<i32> = None;
        for (y, id) in v {
            if let Some(l) = last {
                if y != l + 1 {
                    cols.push(std::mem::take(&mut cur));
                }
            }
            cur.push(id);
            last = Some(y);
        }
        if !cur.is_empty() {
            cols.push(cur);
        }
    }
    cols
}

pub fn valid_name(s: &str) -> bool {
    !s.is_empty() && !s.chars().any(|c| is_ws(c) || "{}'⟨⟩".contains(c))
}

pub fn datum_name_ok(s: &str) -> bool {
    s.chars().next().map_or(false, |c| c.is_alphabetic() || c == '_') && s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// Whether a word must be written ⟨word⟩ to be read as itself.
pub fn needs_angle(w: &str) -> bool {
    active_form(w) || (w.len() > 1 && w.starts_with('\\'))
}

/// The s text of a brick that is not a box.
pub fn word_text(b: &Brick) -> String {
    let w = &b.word;
    match b.kind {
        BKind::Num | BKind::Cmd => format!("{}{}", w, b.cond),
        BKind::Esc => format!("\\{}{}", w, b.cond),
        BKind::Quote => format!("'{}'", w),
        BKind::Read => format!("{{~{}~}}", w),
        BKind::Sock => format!("~{}~", w),
        BKind::Raw => w.clone(),
        BKind::Name => {
            let plain = format!("{}{}", w, b.cap);
            let angle = w.is_empty()
                || w.starts_with('\\')
                || cmd_of(&plain).is_some()
                || (b.cap == "." && w.ends_with('.'))
                || (b.cap == ".." && w.ends_with('.'))
                || w.starts_with('⟨');
            let stem = if angle { format!("⟨{}⟩", w) } else { w.clone() };
            format!("{}{}{}", stem, b.cap, b.cond)
        }
        BKind::Box => String::new(),
        BKind::Lit | BKind::Lib | BKind::Geo => {
            let lit = if needs_angle(w) || (b.kind == BKind::Lit && cmd_of(w).is_some()) { format!("⟨{}⟩", w) } else { w.clone() };
            format!("{}{}", lit, b.cond)
        }
    }
}

pub fn check_word(b: &Brick) -> Result<(), String> {
    let w = &b.word;
    if !CAPS.contains(&b.cap.as_str()) {
        return Err(format!("'{}' is not a cap", b.cap));
    }
    if !matches!(b.cond.as_str(), "" | "?" | "!") {
        return Err(format!("'{}' is not a collar; use ? or !", b.cond));
    }
    match b.kind {
        BKind::Box => Ok(()),
        BKind::Raw => match crate::ops::split_words(w) {
            Ok(v) if v.len() == 1 && v[0] == *w => Ok(()),
            _ => Err(format!("'{}' is not one token", w)),
        },
        BKind::Quote => {
            if w.contains('\'') {
                Err("a quote brick cannot hold '".into())
            } else {
                Ok(())
            }
        }
        BKind::Num => match canon_num(w) {
            Some(_) => Ok(()),
            None => Err(format!("'{}' is not a number in canonical form", w)),
        },
        BKind::Cmd => match cmd_of(w) {
            Some(_) => Ok(()),
            None => Err(format!("'{}' is not a command", w)),
        },
        BKind::Esc => {
            if valid_name(w) {
                Ok(())
            } else {
                Err(format!("'{}' cannot be escaped", w))
            }
        }
        BKind::Name => {
            if b.cap.is_empty() {
                return Err("a name brick needs a cap".into());
            }
            if valid_name(w) {
                Ok(())
            } else {
                Err(format!("'{}' is not a name", w))
            }
        }
        _ => {
            let tilde_ok = !matches!(b.kind, BKind::Read | BKind::Sock) || !w.contains('~');
            if valid_name(w) && tilde_ok {
                Ok(())
            } else {
                Err(format!("'{}' cannot stand on a brick; quote it", w))
            }
        }
    }
}

/// What typing a word onto a brick makes of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Typed {
    pub kind: BKind,
    pub word: String,
    pub cap: String,
    pub cond: String,
    /// For a box typed as {…}: its inside text.
    pub inside: Option<String>,
}

pub fn classify_word(w: &str, lib: &dyn Fn(&str) -> bool) -> Result<Typed, String> {
    let w = w.trim();
    let t = |kind, word: &str, cap: &str, cond: &str| Typed { kind, word: word.to_string(), cap: cap.to_string(), cond: cond.to_string(), inside: None };
    if w.is_empty() {
        return Err("empty word".into());
    }
    if w.starts_with('{') {
        let (body, cond) = if w.ends_with("}?") || w.ends_with("}!") { (&w[..w.len() - 1], &w[w.len() - 1..]) } else { (w, "") };
        if !body.ends_with('}') {
            return Err("a box closes with }".into());
        }
        let inner = &body[1..body.len() - 1];
        if let Some(name) = inner.strip_prefix('~').and_then(|s| s.strip_suffix('~')) {
            if valid_name(name) && !name.contains('~') {
                return Ok(t(BKind::Read, name, "", cond));
            }
        }
        let mut r = t(BKind::Box, "", "", cond);
        r.inside = Some(inner.to_string());
        return Ok(r);
    }
    if !w.starts_with('{') && !w.starts_with('\'') && !w.starts_with('⟨') && (w.contains('{') || w.contains('\'')) {
        return Ok(t(BKind::Raw, w, "", ""));
    }
    if w.starts_with('\'') {
        if w.len() >= 2 && w.ends_with('\'') && w[1..w.len() - 1].find('\'').is_none() {
            return Ok(t(BKind::Quote, &w[1..w.len() - 1], "", ""));
        }
        return Err("a quote closes with '".into());
    }
    if w.chars().any(is_ws) {
        return Err("one word per brick; quote text with spaces".into());
    }
    if w.len() > 2 && w.starts_with('~') && w.ends_with('~') {
        return Ok(t(BKind::Sock, &w[1..w.len() - 1], "", ""));
    }
    if w.starts_with('⟨') {
        let close = w.find('⟩').ok_or("⟨ needs ⟩")?;
        let stem = &w['⟨'.len_utf8()..close];
        let suf = &w[close + '⟩'.len_utf8()..];
        let (rest, cond) = split_cond(suf);
        if !CAPS.contains(&rest) {
            return Err(format!("⟨{}⟩{}: unknown suffix", stem, suf));
        }
        if rest.is_empty() {
            return Ok(t(BKind::Lit, stem, "", cond));
        }
        return Ok(t(BKind::Name, stem, rest, cond));
    }
    if canon_num(w).is_some() {
        return Ok(t(BKind::Num, w, "", ""));
    }
    if cmd_of(w).is_some() {
        return Ok(t(BKind::Cmd, w, "", ""));
    }
    if w.len() > 1 && w.starts_with('\\') {
        let (rest, cond) = split_cond(&w[1..]);
        return Ok(t(BKind::Esc, rest, "", cond));
    }
    if active_form(w) {
        let (rest, cond) = split_cond(w);
        if rest.is_empty() {
            return Ok(t(BKind::Lit, w, "", ""));
        }
        if cmd_of(rest).is_some() {
            return Ok(t(BKind::Cmd, rest, "", cond));
        }
        if canon_num(rest).is_some() && !cond.is_empty() {
            return Ok(t(BKind::Num, rest, "", cond));
        }
        let (stem, cap) = if rest.len() > 2 && rest.ends_with("..") {
            (&rest[..rest.len() - 2], "..")
        } else {
            let last = rest.chars().last().unwrap();
            if SUFFIX.contains(last) && last != '?' && last != '!' && rest.len() > last.len_utf8() {
                (&rest[..rest.len() - last.len_utf8()], &rest[rest.len() - last.len_utf8()..])
            } else {
                (rest, "")
            }
        };
        if cap.is_empty() {
            let k = if crate::graph::HOST_WORDS.contains(&stem) { BKind::Geo } else if lib(stem) { BKind::Lib } else { BKind::Lit };
            return Ok(t(k, stem, "", cond));
        }
        return Ok(t(BKind::Name, stem, cap, cond));
    }
    if crate::graph::HOST_WORDS.contains(&w) {
        return Ok(t(BKind::Geo, w, "", ""));
    }
    if lib(w) {
        return Ok(t(BKind::Lib, w, "", ""));
    }
    Ok(t(BKind::Lit, w, "", ""))
}

fn split_cond(s: &str) -> (&str, &str) {
    if s.len() > 1 && (s.ends_with('?') || s.ends_with('!')) {
        (&s[..s.len() - 1], &s[s.len() - 1..])
    } else {
        (s, "")
    }
}

// JSON

fn v3(v: [f64; 3]) -> J {
    J::Arr(v.iter().map(|x| J::Num(*x)).collect())
}

impl Rec {
    pub fn to_json(&self) -> J {
        match self {
            Rec::Space(s) => {
                let (k, of) = match &s.owner {
                    Owner::World => ("world", 0),
                    Owner::Box(b) => ("box", *b),
                    Owner::Sphere(x) => ("sphere", *x),
                    Owner::Assembly(_) => ("assembly", 0),
                };
                let mut o = J::obj().with("id", s.id.into()).with("owner", k.into()).with("of", of.into());
                if let Owner::Assembly(n) = &s.owner {
                    o.set("name", n.clone().into());
                }
                o
            }
            Rec::Plate(p) => J::obj()
                .with("id", p.id.into())
                .with("space", p.space.into())
                .with("kind", p.kind.name().into())
                .with("name", p.name.clone().into())
                .with("at", p.at.into())
                .with("w", p.w.into())
                .with("d", p.d.into())
                .with("arrow", p.arrow.name().into()),
            Rec::Brick(b) => {
                let mut o = J::obj()
                    .with("id", b.id.into())
                    .with("space", b.space.into())
                    .with("cell", b.cell.into())
                    .with("kind", b.kind.name().into())
                    .with("word", b.word.clone().into());
                if !b.cap.is_empty() {
                    o.set("cap", b.cap.clone().into());
                }
                if !b.cond.is_empty() {
                    o.set("cond", b.cond.clone().into());
                }
                if !b.note.is_empty() {
                    o.set("note", b.note.clone().into());
                }
                if b.inner != 0 {
                    o.set("inner", b.inner.into());
                }
                o
            }
            Rec::Sphere(s) => {
                let mut o = J::obj()
                    .with("id", s.id.into())
                    .with("name", s.name.clone().into())
                    .with("kind", s.kind.name().into())
                    .with("pos", v3(s.pos))
                    .with("r", s.r.into())
                    .with("aim", v3(s.aim))
                    .with("reach", s.reach.into())
                    .with("strike", s.strike.name().into())
                    .with("params", J::Obj(s.params.iter().map(|(k, v)| (k.clone(), J::Num(*v))).collect()))
                    .with("muted", J::Arr(s.muted.iter().map(|x| J::Num(*x as f64)).collect()))
                    .with("body", s.body.into());
                if s.space != WORLD {
                    o.set("space", s.space.into());
                }
                if let Some(c) = s.cell {
                    o.set("cell", c.into());
                }
                if let Some(l) = s.lift {
                    o.set("lift", v3(l));
                }
                o
            }
            Rec::Link(l) => {
                let mut o = J::obj()
                    .with("id", l.id.into())
                    .with("from", l.from.into())
                    .with("to", l.to.into())
                    .with("mode", l.mode.name().into())
                    .with("port", l.port.name().into());
                if !l.from_pin.is_empty() {
                    o.set("from_pin", l.from_pin.clone().into());
                }
                if !l.to_pin.is_empty() {
                    o.set("to_pin", l.to_pin.clone().into());
                }
                if !l.route.is_empty() {
                    o.set("route", J::Arr(l.route.iter().map(|c| (*c).into()).collect()));
                }
                o
            }
            Rec::Zone(z) => J::obj()
                .with("id", z.id.into())
                .with("name", z.name.clone().into())
                .with("kind", z.kind.name().into())
                .with("min", z.min.into())
                .with("max", z.max.into())
                .with("params", J::Obj(z.params.iter().map(|(k, v)| (k.clone(), J::Str(v.clone()))).collect()))
                .with("captures", J::Arr(z.captures.iter().map(|(n, m)| J::Arr(vec![J::Str(n.clone()), J::Str(m.name().into())])).collect())),
            Rec::Datum(d) => {
                let mut o = J::obj()
                    .with("id", d.id.into())
                    .with("name", d.name.clone().into())
                    .with("cell", d.cell.into())
                    .with("value", J::Num(d.value as f64));
                if d.space != WORLD {
                    o.set("space", d.space.into());
                }
                if d.scale != [1.0; 3] {
                    o.set("scale", v3(d.scale));
                }
                if d.rot != [0.0; 3] {
                    o.set("rot", v3(d.rot));
                }
                if !d.log.is_empty() {
                    o.set("log", J::Arr(d.log.iter().map(|l| J::Str(l.clone())).collect()));
                }
                o
            }
            Rec::Track(t) => {
                let mut o = J::obj()
                    .with("id", t.id.into())
                    .with("name", t.name.clone().into())
                    .with("cells", J::Arr(t.cells.iter().map(|c| (*c).into()).collect()))
                    .with("speed", J::Num(t.speed as f64));
                if t.space != WORLD {
                    o.set("space", t.space.into());
                }
                o
            }
        }
    }

    pub fn from_json(tag: &str, o: &J) -> Result<Rec, String> {
        let id = o.get("id").u(0);
        let bad = |what: &str, v: &J| format!("{} {}: bad {} {}", tag, id, what, v.dump());
        Ok(match tag {
            "space" => Rec::Space(Space {
                id,
                owner: match o.get("owner").str() {
                    Some("box") => Owner::Box(o.get("of").u(0)),
                    Some("sphere") => Owner::Sphere(o.get("of").u(0)),
                    Some("assembly") => Owner::Assembly(o.get("name").s("assembly")),
                    _ => Owner::World,
                },
            }),
            "plate" => Rec::Plate(Plate {
                id,
                space: o.get("space").u(0),
                kind: PKind::of(&o.get("kind").s("main")).ok_or_else(|| bad("kind", o.get("kind")))?,
                name: o.get("name").s(""),
                at: o.get("at").i3([0, 0, 0]),
                w: o.get("w").i(8) as i32,
                d: o.get("d").i(8) as i32,
                arrow: Arrow::of(&o.get("arrow").s("+x")).ok_or_else(|| bad("arrow", o.get("arrow")))?,
            }),
            "brick" => Rec::Brick(Brick {
                id,
                space: o.get("space").u(0),
                cell: o.get("cell").i3([0, 0, 0]),
                kind: BKind::of(&o.get("kind").s("lit")).ok_or_else(|| bad("kind", o.get("kind")))?,
                word: o.get("word").s(""),
                cap: o.get("cap").s(""),
                cond: o.get("cond").s(""),
                note: o.get("note").s(""),
                inner: o.get("inner").u(0),
            }),
            "sphere" => Rec::Sphere(Sphere {
                id,
                name: o.get("name").s(""),
                kind: SKind::of(&o.get("kind").s("fn")).ok_or_else(|| bad("kind", o.get("kind")))?,
                pos: o.get("pos").v3([0.0; 3]),
                r: o.get("r").f(0.8),
                aim: norm(o.get("aim").v3([1.0, 0.0, 0.0])),
                reach: o.get("reach").f(60.0),
                strike: Strike::of(&o.get("strike").s("sticky")).unwrap_or(Strike::Sticky),
                params: o.get("params").entries().iter().filter_map(|(k, v)| v.num().map(|x| (k.clone(), x))).collect(),
                muted: o.get("muted").arr().iter().map(|x| x.u(0)).collect(),
                body: o.get("body").u(0),
                space: o.get("space").u(0),
                cell: if o.get("cell").is_null() { None } else { Some(o.get("cell").i3([0, 0, 0])) },
                lift: if o.get("lift").is_null() { None } else { Some(o.get("lift").v3([0.0; 3])) },
            }),
            "link" => Rec::Link(Link {
                id,
                from: o.get("from").u(0),
                to: o.get("to").u(0),
                mode: Strike::of(&o.get("mode").s("sticky")).unwrap_or(Strike::Sticky),
                port: Port::of(&o.get("port").s("out")).unwrap_or(Port::Out),
                from_pin: o.get("from_pin").s(""),
                to_pin: o.get("to_pin").s(""),
                route: o.get("route").arr().iter().map(|c| c.i3([0, 0, 0])).collect(),
            }),
            "zone" => Rec::Zone(Zone {
                id,
                name: o.get("name").s(""),
                kind: ZKind::of(&o.get("kind").s("async")).ok_or_else(|| bad("kind", o.get("kind")))?,
                min: o.get("min").i3([0, 0, 0]),
                max: o.get("max").i3([0, 0, 0]),
                params: o.get("params").entries().iter().map(|(k, v)| (k.clone(), match v {
                    J::Str(s) => s.clone(),
                    v => v.dump(),
                })).collect(),
                captures: o
                    .get("captures")
                    .arr()
                    .iter()
                    .filter_map(|c| Some((c.arr().get(0)?.str()?.to_string(), Mode::of(c.arr().get(1)?.str()?)?)))
                    .collect(),
            }),
            "datum" => Rec::Datum(Datum {
                id,
                space: o.get("space").u(0),
                name: o.get("name").s(""),
                cell: o.get("cell").i3([0, 0, 0]),
                value: o.get("value").f(0.0).max(0.0) as u32,
                scale: o.get("scale").v3([1.0; 3]),
                rot: o.get("rot").v3([0.0; 3]),
                log: o.get("log").arr().iter().filter_map(|l| l.str().map(|x| x.to_string())).collect(),
            }),
            "track" => Rec::Track(Track {
                id,
                space: o.get("space").u(0),
                name: o.get("name").s(""),
                cells: o.get("cells").arr().iter().map(|c| c.i3([0, 0, 0])).collect(),
                speed: o.get("speed").u(1),
            }),
            _ => return Err(format!("unknown record '{}'", tag)),
        })
    }
}

/// Where a device's ray starts and where it is struck: its cell's center, or a mast's top.
pub fn device_pos(s: &Sphere, c: [i32; 3]) -> [f64; 3] {
    let h = if s.kind == SKind::Mast { s.params.iter().find(|p| p.0 == "h").map(|p| p.1).unwrap_or(4.0) } else { 0.0 };
    [c[0] as f64 + 0.5, c[1] as f64 + 0.5 + h, c[2] as f64 + 0.5]
}

/// The cell an assembly placement's inside is laid out from.
pub fn origin_of(s: &Sphere) -> [i32; 3] {
    match s.cell {
        Some(c) => c,
        None => [s.pos[0].floor() as i32, s.pos[1].floor() as i32, s.pos[2].floor() as i32],
    }
}

pub fn assembly_name(w: &World, space: Id) -> Option<&str> {
    match w.spaces.get(&space).map(|x| &x.owner) {
        Some(Owner::Assembly(n)) => Some(n.as_str()),
        _ => None,
    }
}

pub fn assembly_named(w: &World, name: &str) -> Option<Id> {
    w.spaces.values().find(|x| matches!(&x.owner, Owner::Assembly(n) if n == name)).map(|x| x.id)
}

/// The port of an assembly a pin names: its own name, or the part after the last `/`.
pub fn pin_port<'a>(w: &'a World, space: Id, pin: &str, input: bool) -> Option<&'a Sphere> {
    let mut ports: Vec<&Sphere> = w.spheres.values().filter(|s| s.space == space && s.kind == SKind::Port).collect();
    ports.sort_by_key(|s| s.id);
    if !pin.is_empty() {
        return ports.into_iter().find(|s| s.name == pin || s.name.rsplit('/').next() == Some(pin));
    }
    // the first port that feeds the inside (input) or that the inside feeds (output)
    ports.into_iter().find(|p| {
        if input {
            w.links.values().any(|l| l.from == p.id)
        } else {
            w.links.values().any(|l| l.to == p.id)
        }
    })
}

pub fn norm(a: [f64; 3]) -> [f64; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if l > 1e-12 && l.is_finite() {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        [1.0, 0.0, 0.0]
    }
}

impl World {
    pub fn to_json(&self) -> J {
        let all = |v: Vec<Rec>| J::Arr(v.iter().map(|r| r.to_json()).collect());
        J::obj()
            .with("s3", J::Num(1.0))
            .with("name", self.name.clone().into())
            .with("next", self.next.into())
            .with("params", self.params.clone())
            .with("goal", self.goal.clone())
            .with("spaces", all(self.spaces.values().filter(|s| s.id != WORLD).map(|x| Rec::Space(x.clone())).collect()))
            .with("plates", all(self.plates.values().map(|x| Rec::Plate(x.clone())).collect()))
            .with("bricks", all(self.bricks.values().map(|x| Rec::Brick(x.clone())).collect()))
            .with("spheres", all(self.spheres.values().map(|x| Rec::Sphere(x.clone())).collect()))
            .with("links", all(self.links.values().map(|x| Rec::Link(x.clone())).collect()))
            .with("zones", all(self.zones.values().map(|x| Rec::Zone(x.clone())).collect()))
            .with("datums", all(self.datums.values().map(|x| Rec::Datum(x.clone())).collect()))
            .with("tracks", all(self.tracks.values().map(|x| Rec::Track(x.clone())).collect()))
    }

    pub fn from_json(j: &J) -> Result<World, String> {
        let mut w = World::new(&j.get("name").s("world"));
        w.params = if j.get("params").is_null() { J::obj() } else { j.get("params").clone() };
        w.goal = j.get("goal").clone();
        let mut max = 0;
        // structure first, so checks on later records find what they refer to
        for (key, tag) in [("spaces", "space"), ("zones", "zone"), ("plates", "plate"), ("spheres", "sphere"), ("bricks", "brick"), ("links", "link"), ("datums", "datum"), ("tracks", "track")] {
            for o in j.get(key).arr() {
                let r = Rec::from_json(tag, o)?;
                if r.id() == 0 {
                    return Err(format!("{} without an id", tag));
                }
                max = max.max(r.id());
                w.check_rec(&r).map_err(|e| format!("loading {}: {}", tag, e))?;
                w.put_raw(r);
            }
        }
        w.next = j.get("next").u(0).max(max + 1);
        Ok(w)
    }

    /// Changes as the studio reads them: full records put, ids deleted.
    pub fn changes_json(&self, a: &Applied) -> J {
        let mut put = Vec::new();
        for id in &a.put {
            if let Some(r) = self.rec(*id) {
                put.push(r.to_json().with("t", r.tag().into()));
            }
        }
        J::obj()
            .with("put", J::Arr(put))
            .with("del", J::Arr(a.del.iter().map(|x| J::Num(*x as f64)).collect()))
            .with("version", J::Num(self.version as f64))
    }
}
