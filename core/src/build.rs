// World → text units and graph tables per zone: what a session or a compiled program runs.
use crate::json::J;
use crate::rt::{parse, Interner};
use crate::session::{Spec, Unit, ZoneSpec};
use crate::world::*;

#[derive(Debug, Clone)]
pub struct Problem {
    pub brick: Option<Id>,
    pub msg: String,
}

impl Problem {
    pub fn to_json(&self) -> J {
        let mut o = J::obj().with("msg", self.msg.clone().into());
        if let Some(b) = self.brick {
            o.set("brick", b.into());
        }
        o
    }
}

struct B {
    units: Vec<Unit>,
}

impl B {
    fn unit(&mut self, s: TW, map: Vec<(u32, u32, Id)>) -> usize {
        self.units.push(Unit { text: s.s, map });
        self.units.len() - 1
    }
}

/// The zones of a world in order: main first, then by id. Each with its parent's index.
pub fn zone_order(w: &World) -> Vec<(Option<Id>, Option<usize>)> {
    let mut v: Vec<(Option<Id>, Option<usize>)> = vec![(None, None)];
    let zs: Vec<&Zone> = w.zones.values().collect();
    for z in &zs {
        v.push((Some(z.id), None));
    }
    for i in 1..v.len() {
        let z = w.zones.get(&v[i].0.unwrap()).unwrap();
        let p = match w.parent_zone(z) {
            Some(pz) => v.iter().position(|x| x.0 == Some(pz.id)),
            None => Some(0),
        };
        v[i].1 = p;
    }
    v
}

pub fn sphere_json(s: &Sphere) -> J {
    J::obj()
        .with("id", s.id.into())
        .with("name", s.name.clone().into())
        .with("kind", s.kind.name().into())
        .with("pos", s.pos.into())
        .with("r", s.r.into())
        .with("aim", s.aim.into())
        // a port connects by its pins: its ray strikes nothing
        .with("reach", if s.kind == SKind::Port { 1e-9 } else { s.reach }.into())
        .with("strike", s.strike.name().into())
        .with("params", J::Obj(s.params.iter().map(|(k, v)| (k.clone(), J::Num(*v))).collect()))
        .with("muted", J::Arr(s.muted.iter().map(|x| J::Num(*x as f64)).collect()))
}

/// A sphere as its zone's graph holds it: on the grid with its cell, and a device whose
/// body is one machine word works as that machine.
pub fn placed_json(w: &World, s: &Sphere) -> J {
    let mut j = sphere_json(s);
    if let Some(c) = s.cell {
        j.set("cell", c.into());
        if let Some(m) = body_machine(w, s) {
            j.set("machine", m.into());
        }
    }
    j
}

pub fn body_machine(w: &World, s: &Sphere) -> Option<String> {
    let mut t = TW::default();
    let mut map = Vec::new();
    w.space_text_into(s.body, &mut t, &mut map, 0);
    let word = t.s.trim();
    MACHINES.contains(&word).then(|| word.to_string())
}

pub fn link_json(l: &Link) -> J {
    J::obj()
        .with("id", l.id.into())
        .with("from", l.from.into())
        .with("to", l.to.into())
        .with("mode", l.mode.name().into())
        .with("port", l.port.name().into())
}

/// The zone (index in zone_order) a sphere runs in.
pub fn sphere_zone(w: &World, order: &[(Option<Id>, Option<usize>)], s: &Sphere) -> usize {
    let z = w.zone_of_point(s.pos).map(|z| z.id);
    order.iter().position(|x| x.0 == z).unwrap_or(0)
}

/// The zone a datum's names live in: the innermost zone holding its cell.
pub fn datum_zone(w: &World, order: &[(Option<Id>, Option<usize>)], d: &Datum) -> usize {
    let c = [d.cell[0] as f64 + 0.5, d.cell[1] as f64 + 0.5, d.cell[2] as f64 + 0.5];
    let z = w.zone_of_point(c).map(|z| z.id);
    order.iter().position(|x| x.0 == z).unwrap_or(0)
}

pub fn datum_json(d: &Datum) -> J {
    J::obj()
        .with("id", d.id.into())
        .with("name", d.name.clone().into())
        .with("value", J::Num(d.value as f64))
        .with("parts", J::Obj(d.parts().into_iter().map(|(k, v)| (k.to_string(), J::Num(v))).collect()))
}

/// The world with every assembly placement replaced by a copy of its inside, named
/// `placement/inner`, laid out from the placement's origin, its ports wired to the
/// placement's outside links. Placements inside assemblies expand in turn.
pub fn flatten(w: &World) -> Result<World, String> {
    let mut f = w.clone();
    let mut next = f.next.max(1 << 20);
    for round in 0.. {
        let places: Vec<Sphere> = f.spheres.values().filter(|s| s.kind == SKind::Asm && s.space == WORLD).cloned().collect();
        if places.is_empty() {
            break;
        }
        if round > 16 {
            return Err(format!("assembly {} contains itself", places[0].name));
        }
        for p in places {
            let aname = assembly_name(&f, p.body).ok_or_else(|| format!("{}: no assembly", p.name))?.to_string();
            let o = origin_of(&p);
            let of = [o[0] as f64, o[1] as f64, o[2] as f64];
            let inner: Vec<Sphere> = f.spheres.values().filter(|s| s.space == p.body).cloned().collect();
            let mut map: Vec<(Id, Id)> = Vec::new();
            for s in &inner {
                let mut c = s.clone();
                c.id = next;
                next += 1;
                let local = s.name.strip_prefix(&format!("{}/", aname)).unwrap_or(&s.name);
                c.name = format!("{}/{}", p.name, local);
                c.space = WORLD;
                c.pos = [s.pos[0] + of[0], s.pos[1] + of[1], s.pos[2] + of[2]];
                c.cell = s.cell.map(|k| [k[0] + o[0], k[1] + o[1], k[2] + o[2]]);
                c.lift = None;
                c.muted.clear();
                map.push((s.id, c.id));
                f.spheres.insert(c.id, c);
            }
            let to_new = |id: Id| map.iter().find(|m| m.0 == id).map(|m| m.1);
            let inner_links: Vec<Link> = f.links.values().filter(|l| to_new(l.from).is_some() && to_new(l.to).is_some()).cloned().collect();
            for l in inner_links {
                let mut c = l.clone();
                c.id = next;
                next += 1;
                c.from = to_new(l.from).unwrap();
                c.to = to_new(l.to).unwrap();
                f.links.insert(c.id, c);
            }
            let outer: Vec<Link> = f.links.values().filter(|l| l.from == p.id || l.to == p.id).cloned().collect();
            for l in outer {
                f.links.remove(&l.id);
                let mut c = l.clone();
                if l.to == p.id {
                    let port = pin_port(w, p.body, &l.to_pin, true).or_else(|| pin_port(&f, p.body, &l.to_pin, true)).ok_or_else(|| format!("{}: no input port {}", p.name, l.to_pin))?;
                    c.to = to_new(port.id).unwrap();
                    c.to_pin.clear();
                }
                if l.from == p.id {
                    let port = pin_port(w, p.body, &l.from_pin, false).or_else(|| pin_port(&f, p.body, &l.from_pin, false)).ok_or_else(|| format!("{}: no output port {}", p.name, l.from_pin))?;
                    c.from = to_new(port.id).unwrap();
                    c.from_pin.clear();
                }
                f.links.insert(c.id, c);
            }
            f.spheres.remove(&p.id);
        }
    }
    // what stays inside assemblies does not run on its own
    let hidden: Vec<Id> = f.spheres.values().filter(|s| s.space != WORLD).map(|s| s.id).collect();
    for id in &hidden {
        f.spheres.remove(id);
    }
    f.links.retain(|_, l| !hidden.contains(&l.from) && !hidden.contains(&l.to));
    f.next = next;
    Ok(f)
}

pub fn build(w: &World) -> Result<Spec, Vec<Problem>> {
    let flat;
    let w = if w.spheres.values().any(|s| s.space != WORLD || s.kind == SKind::Asm) {
        flat = flatten(w).map_err(|e| vec![Problem { brick: None, msg: e }])?;
        &flat
    } else {
        w
    };
    let mut b = B { units: Vec::new() };
    let mut problems = Vec::new();
    let order = zone_order(w);
    let zone_idx = |p: &Plate| -> usize {
        let z = w.zone_of_plate(p).map(|z| z.id);
        order.iter().position(|x| x.0 == z).unwrap_or(0)
    };

    // units for every box inside, script plate and sphere body: text that runs on its own
    for br in w.bricks.values() {
        if br.kind == BKind::Box && br.word.is_empty() {
            let mut s = TW::default();
            let mut map = Vec::new();
            w.space_text_into(br.inner, &mut s, &mut map, 1);
            b.unit(s, map);
        }
    }
    for p in w.plates.values().filter(|p| p.kind == PKind::Script) {
        let mut s = TW::default();
        let mut map = Vec::new();
        w.plate_text_into(p.id, &mut s, &mut map, 0);
        b.unit(s, map);
    }
    for sp in w.spheres.values() {
        let mut s = TW::default();
        let mut map = Vec::new();
        w.space_text_into(sp.body, &mut s, &mut map, 0);
        b.unit(s, map);
    }

    // where each sphere runs: a gate on a wall runs on both sides, aimed into each
    let idx_of = |z: Option<Id>| order.iter().position(|x| x.0 == z).unwrap_or(0);
    let mut placements: Vec<(usize, Id, J)> = Vec::new();
    for sp in w.spheres.values() {
        if sp.kind == SKind::Gate {
            if let Some((z, n)) = w.gate_wall(sp) {
                let inner_i = idx_of(Some(z.id));
                let outer_i = idx_of(w.parent_zone(z).map(|p| p.id));
                let d = sp.aim[0] * n[0] + sp.aim[1] * n[1] + sp.aim[2] * n[2];
                let inner_aim = if d > 0.0 { sp.aim } else { [-sp.aim[0], -sp.aim[1], -sp.aim[2]] };
                let outer_aim = [-inner_aim[0], -inner_aim[1], -inner_aim[2]];
                let mut a = placed_json(w, sp);
                a.set("aim", inner_aim.into());
                a.set("side", J::Num(2.0));
                a.set("zone_to", outer_i.into());
                placements.push((inner_i, sp.id, a));
                let mut b = placed_json(w, sp);
                b.set("aim", outer_aim.into());
                b.set("side", J::Num(1.0));
                b.set("zone_to", inner_i.into());
                b.set("call", (z.kind == ZKind::Rpc).into());
                placements.push((outer_i, sp.id, b));
                continue;
            }
        }
        placements.push((sphere_zone(w, &order, sp), sp.id, placed_json(w, sp)));
    }
    let mut zones = Vec::new();
    for (zi, (zid, parent)) in order.iter().enumerate() {
        let plates: Vec<&Plate> = {
            let mut v: Vec<&Plate> = w.plates.values().filter(|p| p.space == WORLD && zone_idx(p) == zi).collect();
            v.sort_by_key(|p| (p.at[1], p.at[0], p.at[2], p.id));
            v
        };
        let mut s = TW::default();
        let mut map: Vec<(u32, u32, Id)> = Vec::new();
        let mut first = true;
        let mut sep = |s: &mut TW| {
            if !first {
                s.push("\n");
            }
            first = false;
        };
        for p in plates.iter().filter(|p| p.kind == PKind::Shelf) {
            if w.columns(p.id).is_empty() {
                continue;
            }
            sep(&mut s);
            w.plate_text_into(p.id, &mut s, &mut map, 0);
        }
        for p in plates.iter().filter(|p| p.kind == PKind::Script) {
            sep(&mut s);
            s.push("{");
            w.plate_text_into(p.id, &mut s, &mut map, 1);
            s.push(&format!("}} {}$", p.name));
        }
        let placed: Vec<(&Sphere, J)> = placements.iter().filter(|p| p.0 == zi).map(|p| (w.spheres.get(&p.1).unwrap(), p.2.clone())).collect();
        for (sp, _) in &placed {
            sep(&mut s);
            s.push("{");
            w.space_text_into(sp.body, &mut s, &mut map, 1);
            s.push(&format!("}} sphere:{}$", sp.name));
        }
        for p in plates.iter().filter(|p| p.kind == PKind::Main) {
            if w.columns(p.id).is_empty() {
                continue;
            }
            sep(&mut s);
            w.plate_text_into(p.id, &mut s, &mut map, 0);
        }
        let text = s.s.clone();
        let mut it = Interner::new();
        if let Err(e) = parse(&text, &mut it) {
            problems.push(Problem { brick: None, msg: format!("zone {}: {}", zid.and_then(|z| w.zones.get(&z)).map(|z| z.name.as_str()).unwrap_or("main"), e) });
        }
        let unit = b.unit(s, map);
        let ids: Vec<Id> = placed.iter().map(|s| s.0.id).collect();
        let datums: Vec<J> = w.datums.values().filter(|d| d.space == WORLD && datum_zone(w, &order, d) == zi).map(datum_json).collect();
        let tracks: Vec<J> = w
            .tracks
            .values()
            .filter(|t| t.space == WORLD && {
                let c = t.cells[0];
                let z = w.zone_of_point([c[0] as f64 + 0.5, c[1] as f64 + 0.5, c[2] as f64 + 0.5]).map(|z| z.id);
                order.iter().position(|x| x.0 == z).unwrap_or(0) == zi
            })
            .map(|t| Rec::Track(t.clone()).to_json())
            .collect();
        let graph = J::obj()
            .with("datums", J::Arr(datums))
            .with("tracks", J::Arr(tracks))
            .with("spheres", J::Arr(placed.iter().map(|s| s.1.clone()).collect()))
            .with("links", J::Arr(w.links.values().filter(|l| ids.contains(&l.from) && ids.contains(&l.to)).map(link_json).collect()))
            .with("next_link", w.next.into());
        let z = zid.and_then(|z| w.zones.get(&z));
        zones.push(ZoneSpec {
            id: zid.unwrap_or(0),
            name: z.map(|z| z.name.clone()).unwrap_or_else(|| "main".into()),
            kind: z.map(|z| z.kind.name().to_string()).unwrap_or_else(|| "main".into()),
            unit,
            graph,
            params: z.map(|z| J::Obj(z.params.iter().map(|(k, v)| (k.clone(), J::Str(v.clone()))).collect())).unwrap_or(J::obj()),
            captures: z.map(|z| z.captures.iter().map(|(n, m)| (n.clone(), m.name().to_string())).collect()).unwrap_or_default(),
            parent: *parent,
        });
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(Spec { units: b.units, zones, params: w.params.clone() })
}

/// The s text of a world: one unit per zone, as `.s` files would hold them.
pub fn export_text(w: &World) -> Result<Vec<(String, String)>, Vec<Problem>> {
    let spec = build(w)?;
    Ok(spec.zones.iter().map(|z| (z.name.clone(), spec.units[z.unit].text.clone())).collect())
}

/// The definitions each zone's unit makes, as (zone index, name, text): script plates and
/// sphere bodies. A live edit redefines the ones whose text changed.
pub fn defs(w: &World) -> Vec<(usize, String, String)> {
    let flat;
    let w = if w.spheres.values().any(|s| s.space != WORLD || s.kind == SKind::Asm) {
        match flatten(w) {
            Ok(f) => {
                flat = f;
                &flat
            }
            Err(_) => w,
        }
    } else {
        w
    };
    let order = zone_order(w);
    let idx_of = |z: Option<Id>| order.iter().position(|x| x.0 == z).unwrap_or(0);
    let mut out = Vec::new();
    for p in w.plates.values().filter(|p| p.kind == PKind::Script && p.space == WORLD) {
        let mut s = TW::default();
        let mut map = Vec::new();
        w.plate_text_into(p.id, &mut s, &mut map, 1);
        out.push((idx_of(w.zone_of_plate(p).map(|z| z.id)), p.name.clone(), s.s));
    }
    for sp in w.spheres.values() {
        let mut s = TW::default();
        let mut map = Vec::new();
        w.space_text_into(sp.body, &mut s, &mut map, 1);
        let name = format!("sphere:{}", sp.name);
        if sp.kind == SKind::Gate {
            if let Some((z, _)) = w.gate_wall(sp) {
                out.push((idx_of(Some(z.id)), name.clone(), s.s.clone()));
                out.push((idx_of(w.parent_zone(z).map(|p| p.id)), name, s.s));
                continue;
            }
        }
        out.push((sphere_zone(w, &order, sp), name, s.s));
    }
    out
}
