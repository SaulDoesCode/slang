// Strikes in the edited world: aiming a sphere at another links them, as a run does.
// Rays stay in their zone; gates on a zone's walls are seen from both sides.
use crate::graph::ray_hit;
use crate::world::*;

/// The zone a sphere runs in; a gate answers with its inner zone and its outer zone.
fn sides(w: &World, s: &Sphere) -> Vec<(Option<Id>, [f64; 3])> {
    if s.kind == SKind::Gate {
        if let Some((z, n)) = w.gate_wall(s) {
            let inner = if dot(s.aim, n) > 0.0 { s.aim } else { neg(s.aim) };
            let outer = neg(inner);
            let parent = w.parent_zone(z).map(|p| p.id);
            return vec![(Some(z.id), inner), (parent, outer)];
        }
    }
    vec![(w.zone_of_point(s.pos).map(|z| z.id), s.aim)]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn neg(a: [f64; 3]) -> [f64; 3] {
    [-a[0], -a[1], -a[2]]
}

/// Whether t can be struck from zone `z`.
fn in_zone(w: &World, t: &Sphere, z: Option<Id>) -> bool {
    if t.kind == SKind::Gate {
        if let Some((gz, _)) = w.gate_wall(t) {
            let outer = w.parent_zone(gz).map(|p| p.id);
            return Some(gz.id) == z || outer == z;
        }
    }
    w.zone_of_point(t.pos).map(|x| x.id) == z
}

/// The nearest accepting sphere on each of s's rays (a gate has one per side).
pub fn world_hits(w: &World, s: &Sphere) -> Vec<(Id, f64)> {
    if !s.kind.emits() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (z, dir) in sides(w, s) {
        let mut best: Option<(Id, f64)> = None;
        for t in w.spheres.values() {
            if t.id == s.id || t.space != s.space || !t.kind.accepts() || (s.space == WORLD && !in_zone(w, t, z)) {
                continue;
            }
            if s.kind == SKind::Gate && t.kind == SKind::Gate {
                continue;
            }
            if let Some(th) = ray_hit(s.pos, dir, t.pos, t.r, s.reach) {
                if best.map_or(true, |b| th < b.1) {
                    best = Some((t.id, th));
                }
            }
        }
        if let Some(b) = best {
            out.push(b);
        }
    }
    out
}

pub fn world_hit(w: &World, s: &Sphere) -> Option<(Id, f64)> {
    world_hits(w, s).into_iter().next()
}

/// The links new strikes make and the mutes that no longer hold.
pub fn strike_edits(w: &World) -> Vec<Edit> {
    let mut out = Vec::new();
    let mut next = w.next;
    let mut made: Vec<Link> = Vec::new();
    for s in w.spheres.values() {
        // ports and assembly placements connect by their pins, never by aim
        if matches!(s.kind, SKind::Port | SKind::Asm) {
            continue;
        }
        let hits: Vec<Id> = world_hits(w, s).into_iter().map(|x| x.0).collect();
        let kept: Vec<Id> = s.muted.iter().copied().filter(|m| hits.contains(m)).collect();
        if kept != s.muted {
            let mut n = s.clone();
            n.muted = kept.clone();
            out.push(Edit::Put(Rec::Sphere(n)));
        }
        for t in hits {
            let exists = w.link_between(s.id, t).is_some() || made.iter().any(|l| l.from == s.id && l.to == t);
            if kept.contains(&t) || exists {
                continue;
            }
            let port = if s.kind != SKind::Ratchet {
                Port::Out
            } else if w.links.values().chain(made.iter()).any(|l| l.from == s.id && l.port == Port::Loop) {
                Port::Exit
            } else {
                Port::Loop
            };
            let l = Link { id: next, from: s.id, to: t, mode: s.strike, port, from_pin: String::new(), to_pin: String::new(), route: Vec::new() };
            next += 1;
            made.push(l.clone());
            out.push(Edit::Put(Rec::Link(l)));
        }
    }
    out
}
