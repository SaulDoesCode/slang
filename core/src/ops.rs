// Edits as the studio sends them, expanded into the world's primitive puts and deletes.
use crate::json::J;
use crate::rt::{cmd_of, is_ws};
use crate::world::*;
use std::collections::BTreeSet;

pub struct Ctx<'a> {
    pub w: &'a World,
    next: Id,
    /// Records this expansion creates or changes, newest last, so later steps see them.
    pending: Vec<Rec>,
    gone: BTreeSet<Id>,
    pub edits: Vec<Edit>,
}

fn e_space(o: &J) -> Id {
    o.get("space").u(0)
}

fn arr3(j: &J, what: &str) -> Result<[i32; 3], String> {
    let a = j.arr();
    if a.len() != 3 {
        return Err(format!("{} needs [x, y, z]", what));
    }
    Ok([a[0].i(0) as i32, a[1].i(0) as i32, a[2].i(0) as i32])
}

/// Splits one line of s into brick words: braces and quotes stay whole.
pub fn split_words(line: &str) -> Result<Vec<String>, String> {
    let cs: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        if is_ws(cs[i]) {
            i += 1;
            continue;
        }
        let mut w = String::new();
        while i < cs.len() && !is_ws(cs[i]) {
            match cs[i] {
                '{' => {
                    let mut depth = 0;
                    while i < cs.len() {
                        let c = cs[i];
                        w.push(c);
                        i += 1;
                        if c == '{' {
                            depth += 1;
                        } else if c == '}' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                    }
                    if depth != 0 {
                        return Err("unclosed {".into());
                    }
                }
                '\'' => {
                    w.push('\'');
                    i += 1;
                    while i < cs.len() && cs[i] != '\'' {
                        w.push(cs[i]);
                        i += 1;
                    }
                    if i >= cs.len() {
                        return Err("unclosed '".into());
                    }
                    w.push('\'');
                    i += 1;
                }
                '⟨' => {
                    while i < cs.len() && cs[i] != '⟩' {
                        w.push(cs[i]);
                        i += 1;
                    }
                    if i >= cs.len() {
                        return Err("unclosed ⟨".into());
                    }
                    w.push('⟩');
                    i += 1;
                }
                c => {
                    w.push(c);
                    i += 1;
                }
            }
        }
        out.push(w);
    }
    Ok(out)
}

/// Text as bricks would give it back: one column per line, single spaces between words.
pub fn normalize(t: &str) -> String {
    split_lines(t)
        .iter()
        .map(|l| split_words(l).map(|w| w.join(" ")).unwrap_or_else(|_| l.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Splits the text of a plate into lines, keeping newlines inside braces in their line.
pub fn split_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut quote = false;
    for c in text.chars() {
        match c {
            '\'' if depth == 0 => quote = !quote,
            '{' if !quote => depth += 1,
            '}' if !quote => depth -= 1,
            _ => {}
        }
        if c == '\n' && depth <= 0 && !quote {
            lines.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    lines.push(cur);
    lines.into_iter().filter(|l| !l.trim().is_empty()).collect()
}

pub fn arrow_step(a: Arrow) -> ([i32; 2], [i32; 2]) {
    // (along the arrow, to the next row), as (dx, dz)
    match a {
        Arrow::Px => ([1, 0], [0, 1]),
        Arrow::Nx => ([-1, 0], [0, -1]),
        Arrow::Pz => ([0, 1], [-1, 0]),
        Arrow::Nz => ([0, -1], [1, 0]),
    }
}

pub fn row_start(p: &Plate) -> [i32; 2] {
    match p.arrow {
        Arrow::Px => [p.at[0], p.at[2]],
        Arrow::Nx => [p.at[0] + p.w - 1, p.at[2] + p.d - 1],
        Arrow::Pz => [p.at[0] + p.w - 1, p.at[2]],
        Arrow::Nz => [p.at[0], p.at[2] + p.d - 1],
    }
}

impl<'a> Ctx<'a> {
    pub fn new(w: &'a World) -> Ctx<'a> {
        Ctx { w, next: w.next, pending: Vec::new(), gone: BTreeSet::new(), edits: Vec::new() }
    }
    pub fn alloc(&mut self) -> Id {
        let id = self.next;
        self.next += 1;
        id
    }
    fn put(&mut self, r: Rec) {
        self.gone.remove(&r.id());
        self.pending.retain(|x| x.id() != r.id());
        self.pending.push(r.clone());
        self.edits.push(Edit::Put(r));
    }
    fn del(&mut self, id: Id) {
        if self.gone.insert(id) {
            self.pending.retain(|x| x.id() != id);
            self.edits.push(Edit::Del(id));
        }
    }
    /// The current state of a record, counting this expansion's own changes.
    pub fn get(&self, id: Id) -> Option<Rec> {
        if self.gone.contains(&id) {
            return None;
        }
        if let Some(r) = self.pending.iter().rev().find(|r| r.id() == id) {
            return Some(r.clone());
        }
        self.w.rec(id)
    }
    fn brick(&self, id: Id) -> Option<Brick> {
        match self.get(id) {
            Some(Rec::Brick(b)) => Some(b),
            _ => None,
        }
    }
    fn brick_at(&self, space: Id, cell: [i32; 3]) -> Option<Brick> {
        if let Some(Rec::Brick(b)) = self.pending.iter().rev().find(|r| matches!(r, Rec::Brick(b) if b.space == space && b.cell == cell)) {
            return Some(b.clone());
        }
        let b = self.w.brick_at(space, cell)?;
        self.brick(b.id).filter(|x| x.cell == cell && x.space == space)
    }
    fn plates_in(&self, space: Id) -> Vec<Plate> {
        let mut v: Vec<Plate> = self.w.plates.values().filter(|p| p.space == space && !self.gone.contains(&p.id)).cloned().collect();
        for r in &self.pending {
            if let Rec::Plate(p) = r {
                if p.space == space {
                    v.retain(|x| x.id != p.id);
                    v.push(p.clone());
                }
            }
        }
        v
    }
    fn lib(&self) -> impl Fn(&str) -> bool {
        |w: &str| crate::docs::lib_doc(w).is_some()
    }

    /// A new space owned by `owner`, with one plate for typing into.
    pub fn new_space(&mut self, owner: Owner, kind: PKind) -> (Id, Id) {
        let s = self.alloc();
        self.put(Rec::Space(Space { id: s, owner }));
        let p = self.alloc();
        self.put(Rec::Plate(Plate { id: p, space: s, kind, name: String::new(), at: [0, 0, 0], w: 8, d: 6, arrow: Arrow::Px }));
        (s, p)
    }

    /// Places or replaces a brick from a typed word.
    pub fn place(&mut self, space: Id, cell: [i32; 3], word: &str) -> Result<Id, String> {
        let typed = classify_word(word, &self.lib())?;
        let old = self.brick_at(space, cell);
        let id = match &old {
            Some(b) => b.id,
            None => self.alloc(),
        };
        let mut inner = 0;
        if let Some(b) = &old {
            if b.kind == BKind::Box && typed.kind == BKind::Box && typed.inside.as_deref().map_or(true, |t| t.trim().is_empty()) {
                inner = b.inner;
            } else if b.kind == BKind::Box {
                self.remove_space(b.inner);
            }
        }
        let mut word = typed.word.clone();
        if typed.kind == BKind::Box && inner == 0 {
            let t = typed.inside.clone().unwrap_or_default();
            if normalize(&t) == t {
                let (s, p) = self.new_space(Owner::Box(id), PKind::Inside);
                inner = s;
                if !t.trim().is_empty() {
                    self.layout(p, &t)?;
                }
            } else {
                // text whose spacing matters stays exactly as typed
                word = t;
            }
        }
        let note = old.as_ref().map(|b| b.note.clone()).unwrap_or_default();
        let b = Brick { id, space, cell, kind: typed.kind, word, cap: typed.cap, cond: typed.cond, note, inner };
        check_word(&b)?;
        self.put(Rec::Brick(b));
        Ok(id)
    }

    /// Lays out text on a plate: one column per line, along the arrow, wrapping to the next row.
    pub fn layout(&mut self, plate: Id, text: &str) -> Result<(), String> {
        let p = match self.get(plate) {
            Some(Rec::Plate(p)) => p,
            _ => return Err(format!("no plate {}", plate)),
        };
        let lines = split_lines(text);
        let (along, next) = arrow_step(p.arrow);
        let along_len = if along[0] != 0 { p.w } else { p.d };
        let start = row_start(&p);
        for (i, line) in lines.iter().enumerate() {
            let i = i as i32;
            let (row, col) = (i / along_len, i % along_len);
            let x = start[0] + along[0] * col + next[0] * row;
            let z = start[1] + along[1] * col + next[1] * row;
            let inside = x >= p.at[0] && x < p.at[0] + p.w && z >= p.at[2] && z < p.at[2] + p.d;
            if !inside {
                // grow the plate across rows to fit
                let mut q = p.clone();
                let rows = (lines.len() as i32 + along_len - 1) / along_len;
                if along[0] != 0 {
                    q.d = q.d.max(rows);
                    if next[1] < 0 {
                        q.at[2] = p.at[2] + p.d - q.d;
                    }
                } else {
                    q.w = q.w.max(rows);
                    if next[0] < 0 {
                        q.at[0] = p.at[0] + p.w - q.w;
                    }
                }
                self.put(Rec::Plate(q));
            }
            self.column_at(p.space, [x, p.at[1] + 1, z], line)?;
        }
        Ok(())
    }

    /// The bricks of the column holding `cell`, bottom to top.
    fn column_ids(&self, space: Id, cell: [i32; 3]) -> Vec<Brick> {
        let mut out = Vec::new();
        if self.brick_at(space, cell).is_none() {
            return out;
        }
        let mut y = cell[1];
        while y - 1 > i32::MIN && self.brick_at(space, [cell[0], y - 1, cell[2]]).is_some() {
            y -= 1;
        }
        while let Some(b) = self.brick_at(space, [cell[0], y, cell[2]]) {
            out.push(b);
            y += 1;
        }
        out
    }

    /// Replaces the column at a cell (or starts one there) with the words of a line.
    pub fn column_at(&mut self, space: Id, cell: [i32; 3], line: &str) -> Result<(), String> {
        let words = split_words(line)?;
        let old = self.column_ids(space, cell);
        let base = old.first().map(|b| b.cell[1]).unwrap_or(cell[1]);
        for b in &old {
            self.remove_brick(b.id);
        }
        for (i, w) in words.iter().enumerate() {
            self.place(space, [cell[0], base + i as i32, cell[2]], w)?;
        }
        Ok(())
    }

    pub fn remove_space(&mut self, space: Id) {
        let inside: Vec<Id> = self.w.spheres.values().filter(|x| x.space == space && space != WORLD).map(|x| x.id).collect();
        for sid in inside {
            let _ = self.remove(sid);
        }
        let mut ids = Vec::new();
        self.w.space_contents(space, &mut ids);
        for r in self.pending.clone() {
            match &r {
                Rec::Plate(p) if p.space == space => ids.push(p.id),
                Rec::Brick(b) if b.space == space => {
                    ids.push(b.id);
                    if b.inner != 0 {
                        ids.push(b.inner);
                    }
                }
                _ => {}
            }
        }
        for id in ids {
            self.del(id);
        }
        self.del(space);
    }

    pub fn remove_brick(&mut self, id: Id) {
        if let Some(b) = self.brick(id) {
            if b.kind == BKind::Box && b.inner != 0 {
                self.remove_space(b.inner);
            }
            self.del(id);
        }
    }

    pub fn remove(&mut self, id: Id) -> Result<(), String> {
        match self.get(id) {
            Some(Rec::Brick(_)) => self.remove_brick(id),
            Some(Rec::Plate(p)) => {
                let by = self.w.bricks_by_plate(p.space);
                for b in by.get(&id).cloned().unwrap_or_default() {
                    self.remove_brick(b.id);
                }
                self.del(id);
            }
            Some(Rec::Sphere(s)) => {
                for l in self.w.links.values() {
                    if l.from == id || l.to == id {
                        self.del(l.id);
                    }
                }
                for o in self.w.spheres.values() {
                    if o.muted.contains(&id) {
                        let mut o = o.clone();
                        o.muted.retain(|&m| m != id);
                        self.put(Rec::Sphere(o));
                    }
                }
                if s.body != 0 && s.kind != SKind::Asm {
                    self.remove_space(s.body);
                }
                self.del(id);
            }
            Some(Rec::Link(l)) => {
                // a link removed while its ray still strikes stays muted, so it is not struck again
                if let (Some(Rec::Sphere(a)), Some(Rec::Sphere(_))) = (self.get(l.from), self.get(l.to)) {
                    if crate::strike::world_hit(self.w, &a).map(|h| h.0) == Some(l.to) && !a.muted.contains(&l.to) {
                        let mut a = a.clone();
                        a.muted.push(l.to);
                        self.put(Rec::Sphere(a));
                    }
                }
                self.del(id);
            }
            Some(Rec::Zone(_)) | Some(Rec::Datum(_)) | Some(Rec::Track(_)) => self.del(id),
            Some(Rec::Space(_)) => self.remove_space(id),
            None => return Err(format!("nothing with id {}", id)),
        }
        Ok(())
    }

    /// Moves bricks together: a brick may move into a cell another moving brick leaves.
    pub fn move_bricks(&mut self, ids: &[Id], d: [i32; 3]) -> Result<(), String> {
        let bs: Vec<Brick> = ids.iter().filter_map(|&i| self.brick(i)).collect();
        for b in &bs {
            self.del(b.id);
        }
        for b in &bs {
            let mut n = b.clone();
            n.cell = [b.cell[0] + d[0], b.cell[1] + d[1], b.cell[2] + d[2]];
            self.put(Rec::Brick(n));
        }
        Ok(())
    }

    /// Copies bricks (with their insides) by an offset.
    pub fn duplicate(&mut self, ids: &[Id], d: [i32; 3]) -> Result<Vec<Id>, String> {
        let mut out = Vec::new();
        for &id in ids {
            if let Some(b) = self.brick(id) {
                let cell = [b.cell[0] + d[0], b.cell[1] + d[1], b.cell[2] + d[2]];
                out.push(self.copy_brick(&b, b.space, cell)?);
            }
        }
        Ok(out)
    }

    fn copy_brick(&mut self, b: &Brick, space: Id, cell: [i32; 3]) -> Result<Id, String> {
        let id = self.alloc();
        let mut n = b.clone();
        n.id = id;
        n.space = space;
        n.cell = cell;
        if b.kind == BKind::Box {
            let s = self.alloc();
            self.put(Rec::Space(Space { id: s, owner: Owner::Box(id) }));
            for p in self.plates_in(b.inner) {
                let mut q = p.clone();
                q.id = self.alloc();
                q.space = s;
                self.put(Rec::Plate(q));
            }
            let inner: Vec<Brick> = self.w.bricks.values().filter(|x| x.space == b.inner).cloned().collect();
            for x in inner {
                self.copy_brick(&x, s, x.cell)?;
            }
            n.inner = s;
        }
        self.put(Rec::Brick(n));
        Ok(id)
    }

    fn spheres_now(&self) -> Vec<Sphere> {
        let mut v: Vec<Sphere> = self.w.spheres.values().filter(|s| !self.gone.contains(&s.id)).cloned().collect();
        for r in &self.pending {
            if let Rec::Sphere(s) = r {
                v.retain(|x| x.id != s.id);
                v.push(s.clone());
            }
        }
        v
    }

    fn cell_taken(&self, space: Id, c: [i32; 3], me: Id) -> bool {
        self.w.cells.contains_key(&(space, c)) || self.spheres_now().iter().any(|s| s.id != me && s.space == space && s.cell == Some(c)) || self.datums_now().iter().any(|d| d.id != me && d.space == space && d.cell == c)
            || self.w.tracks.values().any(|t| t.id != me && !self.gone.contains(&t.id) && t.space == space && t.cells.contains(&c))
    }

    fn datums_now(&self) -> Vec<Datum> {
        let mut v: Vec<Datum> = self.w.datums.values().filter(|d| !self.gone.contains(&d.id)).cloned().collect();
        for r in &self.pending {
            if let Rec::Datum(d) = r {
                v.retain(|x| x.id != d.id);
                v.push(d.clone());
            }
        }
        v
    }

    fn datum(&self, id: Id) -> Result<Datum, String> {
        match self.get(id) {
            Some(Rec::Datum(d)) => Ok(d),
            _ => Err(format!("no datum {}", id)),
        }
    }

    pub fn new_datum(&mut self, e: &J) -> Result<Id, String> {
        let id = self.alloc();
        let space = e_space(e);
        let cell = arr3(e.get("cell"), "cell")?;
        let name = match e.get("name").str() {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => {
                let all = self.datums_now();
                let mut i = 1;
                while all.iter().any(|d| d.name == format!("c{}", i)) || self.w.sphere_named(&format!("c{}", i)).is_some() {
                    i += 1;
                }
                format!("c{}", i)
            }
        };
        let value = e.get("value").f(0.0);
        if !(value >= 0.0 && value <= 16777215.0 && value.fract() == 0.0) {
            return Err(format!("a datum holds a whole number from 0 to 16777215, got {}", value));
        }
        let mut d = Datum { id, space, name, cell, value: value as u32, scale: e.get("scale").v3([1.0; 3]), rot: e.get("rot").v3([0.0; 3]), log: Vec::new() };
        d.log.push(format!("{} {}#", d.value, d.name));
        for (n, x) in d.parts() {
            d.log.push(format!("{} {}/{}#", num_text(x), d.name, n));
        }
        self.put(Rec::Datum(d));
        Ok(id)
    }

    /// Records a mutation on a datum's log, folding the log into a rebuild when it grows long.
    fn logged(d: &mut Datum, line: String) {
        d.log.push(line);
        if d.log.len() > LOG_MAX {
            d.log = rebuild_log(d);
        }
    }

    fn free_name(&self, base: &str) -> String {
        let all = self.spheres_now();
        if !all.iter().any(|s| s.name == base) {
            return base.to_string();
        }
        let mut i = 2;
        while all.iter().any(|s| s.name == format!("{}{}", base, i)) {
            i += 1;
        }
        format!("{}{}", base, i)
    }

    /// Collapses spheres into an assembly placed where they were. Links that cross the
    /// selection's edge are cut: each becomes a port inside and a pinned link outside.
    pub fn assemble(&mut self, ids: &[Id], name: &str) -> Result<Id, String> {
        if !valid_name(name) {
            return Err("an assembly needs a one-word name".into());
        }
        if assembly_named(self.w, name).is_some() {
            return Err(format!("an assembly is already named {}", name));
        }
        let sel: Vec<Sphere> = ids.iter().filter_map(|i| match self.get(*i) {
            Some(Rec::Sphere(s)) => Some(s),
            _ => None,
        }).collect();
        if sel.is_empty() {
            return Err("select spheres or devices to assemble".into());
        }
        let space = sel[0].space;
        if sel.iter().any(|s| s.space != space) {
            return Err("an assembly gathers spheres of one space".into());
        }
        if let Some(g) = sel.iter().find(|s| s.kind == SKind::Gate) {
            return Err(format!("{} is a gate: gates stay on their zone's wall", g.name));
        }
        let n = sel.len() as f64;
        let c = [0, 1, 2].map(|i| sel.iter().map(|s| s.pos[i]).sum::<f64>() / n);
        let all_devices = sel.iter().all(|s| s.cell.is_some());
        let o = [c[0].floor() as i32, if all_devices { sel.iter().map(|s| s.cell.unwrap()[1]).min().unwrap() } else { c[1].floor() as i32 }, c[2].floor() as i32];
        let of = [o[0] as f64, o[1] as f64, o[2] as f64];
        let a = self.alloc();
        self.put(Rec::Space(Space { id: a, owner: Owner::Assembly(name.to_string()) }));
        let chosen: Vec<Id> = sel.iter().map(|s| s.id).collect();
        for s in &sel {
            let mut m = s.clone();
            m.space = a;
            m.pos = [s.pos[0] - of[0], s.pos[1] - of[1], s.pos[2] - of[2]];
            m.cell = s.cell.map(|k| [k[0] - o[0], k[1] - o[1], k[2] - o[2]]);
            m.lift = s.lift.map(|l| [l[0] - of[0], l[1] - of[1], l[2] - of[2]]);
            self.put(Rec::Sphere(m));
        }
        // the placement
        let pid = self.alloc();
        let pname = self.free_name(name);
        let mut place = Sphere {
            id: pid,
            name: pname,
            kind: SKind::Asm,
            pos: [of[0] + 0.5, of[1] + 0.5, of[2] + 0.5],
            r: 0.9,
            aim: [1.0, 0.0, 0.0],
            reach: 60.0,
            strike: Strike::Sticky,
            params: Vec::new(),
            muted: Vec::new(),
            body: a,
            space,
            cell: None,
            lift: None,
        };
        if all_devices && !self.cell_taken(space, o, pid) {
            place.cell = Some(o);
            place.pos = device_pos(&place, o);
        }
        self.put(Rec::Sphere(place));
        // cut the crossing links
        let crossing: Vec<Link> = self.w.links.values().filter(|l| chosen.contains(&l.from) != chosen.contains(&l.to) && (chosen.contains(&l.from) || chosen.contains(&l.to))).cloned().collect();
        for l in crossing {
            self.del(l.id);
            let inward = chosen.contains(&l.to);
            let (inside, outside) = if inward { (l.to, l.from) } else { (l.from, l.to) };
            let (Some(Rec::Sphere(ins)), Some(Rec::Sphere(out))) = (self.get(inside), self.get(outside)) else { continue };
            let pin = format!("{}_{}", if inward { "from" } else { "to" }, out.name.replace('/', "_"));
            let port_name = self.free_name(&format!("{}/{}", name, pin));
            let side = if inward { -1.5 } else { 1.5 };
            let ppos = [ins.pos[0] + side, ins.pos[1], ins.pos[2]];
            let port = self.alloc();
            let (body, _) = self.new_space(Owner::Sphere(port), PKind::Inside);
            self.put(Rec::Sphere(Sphere {
                id: port,
                name: port_name.clone(),
                kind: SKind::Port,
                pos: ppos,
                r: 0.5,
                aim: if inward { [1.0, 0.0, 0.0] } else { [-1.0, 0.0, 0.0] },
                reach: 60.0,
                strike: Strike::Sticky,
                params: Vec::new(),
                muted: Vec::new(),
                body,
                space: a,
                cell: None,
                lift: None,
            }));
            let pin = port_name.rsplit('/').next().unwrap_or(&pin).to_string();
            let in_link = self.alloc();
            let out_link = self.alloc();
            if inward {
                self.put(Rec::Link(Link { id: in_link, from: port, to: inside, mode: Strike::Sticky, port: Port::Out, from_pin: String::new(), to_pin: String::new(), route: Vec::new() }));
                self.put(Rec::Link(Link { id: out_link, from: outside, to: pid, mode: l.mode, port: l.port, from_pin: l.from_pin.clone(), to_pin: pin, route: l.route.clone() }));
            } else {
                self.put(Rec::Link(Link { id: in_link, from: inside, to: port, mode: l.mode, port: l.port, from_pin: String::new(), to_pin: String::new(), route: Vec::new() }));
                self.put(Rec::Link(Link { id: out_link, from: pid, to: outside, mode: Strike::Sticky, port: Port::Out, from_pin: pin, to_pin: l.to_pin.clone(), route: l.route.clone() }));
            }
        }
        Ok(pid)
    }

    pub fn new_sphere(&mut self, o: &J) -> Result<Id, String> {
        let id = self.alloc();
        let kind = SKind::of(&o.get("kind").s("fn")).ok_or("unknown sphere kind")?;
        let base = kind.name();
        let name = match o.get("name").str() {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => {
                let mut i = 1;
                while self.w.sphere_named(&format!("{}{}", base, i)).is_some() {
                    i += 1;
                }
                format!("{}{}", base, i)
            }
        };
        let (body, plate) = self.new_space(Owner::Sphere(id), PKind::Inside);
        let mut params: Vec<(String, f64)> = Vec::new();
        match kind {
            SKind::Ratchet => params.push(("n".into(), 5.0)),
            SKind::Src => {
                params.push(("every".into(), 0.0));
                params.push(("run".into(), 1.0));
            }
            _ => {}
        }
        for (k, v) in o.get("params").entries() {
            if let Some(x) = v.num() {
                params.retain(|p| &p.0 != k);
                params.push((k.clone(), x));
            }
        }
        let s = Sphere {
            id,
            name,
            kind,
            pos: o.get("pos").v3([0.0, 2.0, 0.0]),
            r: o.get("r").f(0.8),
            aim: norm(o.get("aim").v3([1.0, 0.0, 0.0])),
            reach: o.get("reach").f(60.0),
            strike: Strike::of(&o.get("strike").s("sticky")).unwrap_or(Strike::Sticky),
            params,
            muted: Vec::new(),
            body,
            space: e_space(o),
            cell: None,
            lift: None,
        };
        let mut s = s;
        if !o.get("cell").is_null() {
            let c = o.get("cell").i3([0, 1, 0]);
            if self.cell_taken(s.space, c, id) {
                return Err(format!("cell {:?} is taken", c));
            }
            s.cell = Some(c);
            s.pos = device_pos(&s, c);
        }
        self.put(Rec::Sphere(s));
        let code = match o.get("code").str() {
            Some(c) => c.to_string(),
            None => match kind {
                SKind::Src => "1".to_string(),
                _ => String::new(),
            },
        };
        if !code.trim().is_empty() {
            self.layout(plate, &code)?;
        }
        Ok(id)
    }

    /// Expands one studio edit.
    pub fn op(&mut self, e: &J) -> Result<J, String> {
        let op = e.get("op").s("");
        match op.as_str() {
            "place" => {
                let space = e.get("space").u(0);
                let cell = arr3(e.get("cell"), "cell")?;
                let id = self.place(space, cell, &e.get("word").s(""))?;
                if let Some(n) = e.get("note").str() {
                    if let Some(mut b) = self.brick(id) {
                        b.note = n.to_string();
                        self.put(Rec::Brick(b));
                    }
                }
                Ok(J::obj().with("id", id.into()))
            }
            "set" => {
                let id = e.get("id").u(0);
                let b = self.brick(id).ok_or_else(|| format!("no brick {}", id))?;
                if let Some(w) = e.get("word").str() {
                    self.place(b.space, b.cell, w)?;
                }
                let mut b = self.brick(id).ok_or("brick vanished")?;
                if let Some(n) = e.get("note").str() {
                    b.note = n.to_string();
                }
                if let Some(c) = e.get("cap").str() {
                    b.cap = c.to_string();
                    if !c.is_empty() && matches!(b.kind, BKind::Lit | BKind::Lib | BKind::Geo) {
                        b.kind = BKind::Name;
                    }
                    if c.is_empty() && b.kind == BKind::Name {
                        b.kind = BKind::Lit;
                    }
                }
                if let Some(c) = e.get("cond").str() {
                    b.cond = c.to_string();
                }
                if let Some(k) = e.get("kind").str() {
                    b.kind = BKind::of(k).ok_or("unknown kind")?;
                }
                check_word(&b)?;
                self.put(Rec::Brick(b));
                Ok(J::obj().with("id", id.into()))
            }
            "settext" => {
                // replace a plate's bricks (or a space's first plate) with text laid out as columns
                let plate = match e.get("plate").num() {
                    Some(p) => p as Id,
                    None => {
                        let space = e.get("space").u(0);
                        let mut ps = self.plates_in(space);
                        ps.sort_by_key(|p| (p.at[1], p.at[0], p.at[2], p.id));
                        ps.first().map(|p| p.id).ok_or_else(|| format!("space {} has no plate", space))?
                    }
                };
                let p = match self.get(plate) {
                    Some(Rec::Plate(p)) => p,
                    _ => return Err(format!("no plate {}", plate)),
                };
                let by = self.w.bricks_by_plate(p.space);
                for b in by.get(&plate).cloned().unwrap_or_default() {
                    self.remove_brick(b.id);
                }
                self.layout(plate, &e.get("text").s(""))?;
                Ok(J::obj())
            }
            "column" => {
                let space = e.get("space").u(0);
                let cell = arr3(e.get("cell"), "cell")?;
                self.column_at(space, cell, &e.get("line").s(""))?;
                Ok(J::obj())
            }
            "paste" => {
                let space = e.get("space").u(0);
                let cell = arr3(e.get("cell"), "cell")?;
                let text = e.get("text").s("");
                let p = self.w.plate_under(space, cell).cloned().ok_or("paste onto a plate")?;
                let (along, next) = arrow_step(p.arrow);
                let mut c = [cell[0], cell[2]];
                let mut row0 = c;
                for line in split_lines(&text) {
                    let inside = |c: [i32; 2]| c[0] >= p.at[0] && c[0] < p.at[0] + p.w && c[1] >= p.at[2] && c[1] < p.at[2] + p.d;
                    if !inside(c) {
                        row0 = [row0[0] + next[0], row0[1] + next[1]];
                        let rs = row_start(&p);
                        c = if along[0] != 0 { [rs[0], row0[1]] } else { [row0[0], rs[1]] };
                        if !inside(c) {
                            return Err("the plate is full; make it bigger first".into());
                        }
                    }
                    self.column_at(space, [c[0], p.at[1] + 1, c[1]], &line)?;
                    c = [c[0] + along[0], c[1] + along[1]];
                }
                Ok(J::obj())
            }
            "remove" => {
                for id in e.get("ids").arr() {
                    self.remove(id.u(0))?;
                }
                Ok(J::obj())
            }
            "move" => {
                let d = arr3(e.get("delta"), "delta")?;
                let ids: Vec<Id> = e.get("ids").arr().iter().map(|x| x.u(0)).collect();
                let mut bricks = Vec::new();
                for &id in &ids {
                    match self.get(id) {
                        Some(Rec::Brick(_)) => bricks.push(id),
                        Some(Rec::Plate(mut p)) => {
                            let by = self.w.bricks_by_plate(p.space);
                            for b in by.get(&id).cloned().unwrap_or_default() {
                                if !ids.contains(&b.id) {
                                    bricks.push(b.id);
                                }
                            }
                            p.at = [p.at[0] + d[0], p.at[1] + d[1], p.at[2] + d[2]];
                            self.put(Rec::Plate(p));
                        }
                        Some(Rec::Sphere(mut s)) => {
                            s.pos = [s.pos[0] + d[0] as f64, s.pos[1] + d[1] as f64, s.pos[2] + d[2] as f64];
                            if let Some(c) = s.cell {
                                let c = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
                                s.cell = Some(c);
                                s.pos = device_pos(&s, c);
                            }
                            if let Some(l) = s.lift {
                                s.lift = Some([l[0] + d[0] as f64, l[1] + d[1] as f64, l[2] + d[2] as f64]);
                            }
                            self.put(Rec::Sphere(s));
                        }
                        Some(Rec::Zone(mut z)) => {
                            for i in 0..3 {
                                z.min[i] += d[i];
                                z.max[i] += d[i];
                            }
                            self.put(Rec::Zone(z));
                        }
                        Some(Rec::Datum(mut x)) => {
                            x.cell = [x.cell[0] + d[0], x.cell[1] + d[1], x.cell[2] + d[2]];
                            self.put(Rec::Datum(x));
                        }
                        _ => {}
                    }
                }
                self.move_bricks(&bricks, d)?;
                Ok(J::obj())
            }
            "rotate" => {
                // a quarter turn about +y through the pivot cell, counterclockwise seen from above
                let ids: Vec<Id> = e.get("ids").arr().iter().map(|x| x.u(0)).collect();
                let pv = e.get("pivot").i3([0, 0, 0]);
                let (px, pz) = (pv[0], pv[2]);
                let turns = (e.get("turns").i(1).rem_euclid(4)) as usize;
                let cell = |c: [i32; 3]| -> [i32; 3] {
                    let (mut x, mut z) = (c[0], c[2]);
                    for _ in 0..turns {
                        let (dx, dz) = (x - px, z - pz);
                        x = px + dz;
                        z = pz - dx;
                    }
                    [x, c[1], z]
                };
                let vec = |v: [f64; 3], about: (f64, f64)| -> [f64; 3] {
                    let (mut x, mut z) = (v[0], v[2]);
                    for _ in 0..turns {
                        let (dx, dz) = (x - about.0, z - about.1);
                        x = about.0 + dz;
                        z = about.1 - dx;
                    }
                    [x, v[1], z]
                };
                let mut bricks: Vec<Brick> = Vec::new();
                for &id in &ids {
                    match self.get(id) {
                        Some(Rec::Brick(b)) => bricks.push(b),
                        Some(Rec::Plate(mut p)) => {
                            let by = self.w.bricks_by_plate(p.space);
                            for b in by.get(&id).cloned().unwrap_or_default() {
                                if !ids.contains(&b.id) {
                                    bricks.push(b.clone());
                                }
                            }
                            let a = cell([p.at[0], p.at[1], p.at[2]]);
                            let c = cell([p.at[0] + p.w - 1, p.at[1], p.at[2] + p.d - 1]);
                            p.at = [a[0].min(c[0]), p.at[1], a[2].min(c[2])];
                            if turns % 2 == 1 {
                                std::mem::swap(&mut p.w, &mut p.d);
                            }
                            for _ in 0..turns {
                                p.arrow = match p.arrow {
                                    Arrow::Px => Arrow::Nz,
                                    Arrow::Nz => Arrow::Nx,
                                    Arrow::Nx => Arrow::Pz,
                                    Arrow::Pz => Arrow::Px,
                                };
                            }
                            self.put(Rec::Plate(p));
                        }
                        Some(Rec::Sphere(mut sp)) => {
                            let c = (px as f64 + 0.5, pz as f64 + 0.5);
                            sp.pos = vec(sp.pos, c);
                            sp.aim = vec(sp.aim, (0.0, 0.0));
                            if let Some(cc) = sp.cell {
                                let n = cell(cc);
                                sp.cell = Some(n);
                                sp.pos = device_pos(&sp, n);
                            }
                            if let Some(l) = sp.lift {
                                sp.lift = Some(vec(l, c));
                            }
                            self.put(Rec::Sphere(sp));
                        }
                        Some(Rec::Datum(mut d)) => {
                            d.cell = cell(d.cell);
                            let ry = norm_deg(d.rot[1] + 90.0 * turns as f64);
                            if ry != d.rot[1] {
                                d.rot[1] = ry;
                                let line = format!("{} {}/ry#", num_text(ry), d.name);
                                Self::logged(&mut d, line);
                            }
                            self.put(Rec::Datum(d));
                        }
                        _ => {}
                    }
                }
                for b in &bricks {
                    self.del(b.id);
                }
                for b in &bricks {
                    let mut n = b.clone();
                    n.cell = cell(b.cell);
                    self.put(Rec::Brick(n));
                }
                Ok(J::obj())
            }
            "setdown" => {
                let ids: Vec<Id> = e.get("ids").arr().iter().map(|x| x.u(0)).collect();
                for id in ids {
                    let Some(Rec::Sphere(mut sp)) = self.get(id) else { return Err(format!("no sphere {}", id)) };
                    if sp.cell.is_some() {
                        continue;
                    }
                    if sp.kind == SKind::Gate {
                        return Err(format!("{} is a gate: gates stay on their zone's wall", sp.name));
                    }
                    let mut c = if !e.get("cell").is_null() {
                        e.get("cell").i3([0, 1, 0])
                    } else {
                        let (x, z) = (sp.pos[0].floor() as i32, sp.pos[2].floor() as i32);
                        let y = self.w.plate_under(sp.space, [x, i32::MAX - 1, z]).map(|p| p.at[1] + 1).unwrap_or(1);
                        [x, y, z]
                    };
                    while self.cell_taken(sp.space, c, sp.id) {
                        c[1] += 1;
                    }
                    sp.lift = Some(sp.pos);
                    sp.cell = Some(c);
                    sp.pos = device_pos(&sp, c);
                    self.put(Rec::Sphere(sp));
                }
                Ok(J::obj())
            }
            "lift" => {
                let ids: Vec<Id> = e.get("ids").arr().iter().map(|x| x.u(0)).collect();
                for id in ids {
                    let Some(Rec::Sphere(mut sp)) = self.get(id) else { return Err(format!("no sphere {}", id)) };
                    let Some(c) = sp.cell else { continue };
                    if sp.kind == SKind::Mast {
                        return Err(format!("{}: a mast stands on a cell", sp.name));
                    }
                    sp.pos = sp.lift.unwrap_or([c[0] as f64 + 0.5, c[1] as f64 + 3.5, c[2] as f64 + 0.5]);
                    sp.cell = None;
                    sp.lift = None;
                    self.put(Rec::Sphere(sp));
                }
                Ok(J::obj())
            }
            "assemble" => {
                let ids: Vec<Id> = e.get("ids").arr().iter().map(|x| x.u(0)).collect();
                let name = e.get("name").s("");
                let id = self.assemble(&ids, &name)?;
                Ok(J::obj().with("id", id.into()))
            }
            "instance" => {
                let asm = match e.get("assembly") {
                    J::Str(n) => assembly_named(self.w, n).ok_or_else(|| format!("no assembly {}", n))?,
                    j => j.u(0),
                };
                let aname = assembly_name(self.w, asm).ok_or_else(|| format!("no assembly {}", asm))?.to_string();
                let id = self.alloc();
                let name = match e.get("name").str() {
                    Some(n) if !n.is_empty() => n.to_string(),
                    _ => self.free_name(&aname),
                };
                let mut sp = Sphere {
                    id,
                    name,
                    kind: SKind::Asm,
                    pos: e.get("pos").v3([0.5, 1.5, 0.5]),
                    r: 0.9,
                    aim: norm(e.get("aim").v3([1.0, 0.0, 0.0])),
                    reach: 60.0,
                    strike: Strike::Sticky,
                    params: Vec::new(),
                    muted: Vec::new(),
                    body: asm,
                    space: e_space(e),
                    cell: None,
                    lift: None,
                };
                if !e.get("cell").is_null() {
                    let c = e.get("cell").i3([0, 1, 0]);
                    sp.cell = Some(c);
                    sp.pos = device_pos(&sp, c);
                }
                self.put(Rec::Sphere(sp));
                Ok(J::obj().with("id", id.into()))
            }
            "datum" => {
                let id = self.new_datum(e)?;
                Ok(J::obj().with("id", id.into()))
            }
            "tile" => {
                let mut d = self.datum(e.get("id").u(0))?;
                let face = match e.get("face") {
                    J::Str(f) => FACE_NAMES.iter().position(|x| x == f).ok_or_else(|| format!("unknown face {}", f))?,
                    j => j.u(9) as usize,
                };
                let (u, v) = (e.get("u").u(9) as usize, e.get("v").u(9) as usize);
                if face > 5 || u > 1 || v > 1 {
                    return Err("a tile is face 0…5 (+x −x +y −y +z −z) and u, v 0 or 1".into());
                }
                let bit = tile_bit(face, u, v);
                let m = 1u32 << bit;
                let on = d.value & m == 0;
                d.value ^= m;
                let line = format!("{} {}{}", m, d.name, if on { ";" } else { ":" });
                Self::logged(&mut d, line);
                let value = d.value;
                self.put(Rec::Datum(d));
                Ok(J::obj().with("bit", J::Num(bit as f64)).with("value", J::Num(value as f64)))
            }
            "value" => {
                let mut d = self.datum(e.get("id").u(0))?;
                let x = e.get("value").f(-1.0);
                if !(x >= 0.0 && x <= 16777215.0 && x.fract() == 0.0) {
                    return Err(format!("a datum holds a whole number from 0 to 16777215, got {}", x));
                }
                if d.value != x as u32 {
                    d.value = x as u32;
                    let line = format!("{} {}#", d.value, d.name);
                    Self::logged(&mut d, line);
                    self.put(Rec::Datum(d));
                }
                Ok(J::obj())
            }
            "transform" => {
                let mut d = self.datum(e.get("id").u(0))?;
                let mut set: Vec<(usize, f64)> = Vec::new();
                if !e.get("scale").is_null() {
                    let v = e.get("scale").v3(d.scale);
                    set.extend((0..3).map(|i| (i, v[i])));
                }
                if !e.get("rot").is_null() {
                    let v = e.get("rot").v3(d.rot);
                    set.extend((0..3).map(|i| (i + 3, v[i])));
                }
                for (k, v) in e.get("parts").entries() {
                    let i = PART_NAMES.iter().position(|p| p == k).ok_or_else(|| format!("unknown part {}: the parts are sx sy sz rx ry rz", k))?;
                    set.push((i, v.num().ok_or_else(|| format!("part {} needs a number", k))?));
                }
                for (i, x) in set {
                    let x = if i >= 3 { norm_deg(x) } else { x };
                    if d.part(i) != x {
                        d.set_part(i, x);
                        let line = format!("{} {}/{}#", num_text(x), d.name, PART_NAMES[i]);
                        Self::logged(&mut d, line);
                    }
                }
                self.put(Rec::Datum(d));
                Ok(J::obj())
            }
            "clearlog" => {
                let mut d = self.datum(e.get("id").u(0))?;
                d.log = rebuild_log(&d);
                self.put(Rec::Datum(d));
                Ok(J::obj())
            }
            "duplicate" => {
                let d = arr3(e.get("delta"), "delta")?;
                let ids: Vec<Id> = e.get("ids").arr().iter().map(|x| x.u(0)).collect();
                let out = self.duplicate(&ids, d)?;
                Ok(J::obj().with("ids", J::Arr(out.into_iter().map(|x| J::Num(x as f64)).collect())))
            }
            "track" => {
                // a track from its cells, or from `from` to `to` along x first, then z, then y
                let mut cells: Vec<[i32; 3]> = Vec::new();
                if !e.get("cells").is_null() {
                    for c in e.get("cells").arr() {
                        cells.push(arr3(c, "cell")?);
                    }
                } else {
                    let a = arr3(e.get("from"), "from")?;
                    let b = arr3(e.get("to"), "to")?;
                    let mut c = a;
                    cells.push(c);
                    for ax in [0, 2, 1] {
                        while c[ax] != b[ax] {
                            c[ax] += (b[ax] - c[ax]).signum();
                            cells.push(c);
                        }
                    }
                }
                let space = e_space(e);
                let name = match e.get("name").str() {
                    Some(n) if !n.is_empty() => n.to_string(),
                    _ => {
                        let mut i = 1;
                        while self.w.track_named(&format!("track{}", i)).is_some() {
                            i += 1;
                        }
                        format!("track{}", i)
                    }
                };
                for c in &cells {
                    if self.cell_taken(space, *c, 0) {
                        return Err(format!("cell {:?} is taken", c));
                    }
                }
                let id = self.alloc();
                self.put(Rec::Track(Track { id, space, name, cells, speed: e.get("speed").u(1) }));
                Ok(J::obj().with("id", id.into()))
            }
            "sphere" => {
                let id = self.new_sphere(e)?;
                Ok(J::obj().with("id", id.into()))
            }
            "plate" => {
                let id = self.alloc();
                let p = Plate {
                    id,
                    space: e.get("space").u(0),
                    kind: PKind::of(&e.get("kind").s("main")).ok_or("unknown plate kind")?,
                    name: e.get("name").s(""),
                    at: e.get("at").i3([0, 0, 0]),
                    w: e.get("w").i(8) as i32,
                    d: e.get("d").i(6) as i32,
                    arrow: Arrow::of(&e.get("arrow").s("+x")).ok_or("unknown arrow")?,
                };
                self.put(Rec::Plate(p));
                Ok(J::obj().with("id", id.into()))
            }
            "zone" => {
                let id = self.alloc();
                let r = Rec::from_json("zone", &e.clone().with("id", id.into()))?;
                self.put(r);
                Ok(J::obj().with("id", id.into()))
            }
            "link" => {
                let (from, to) = (e.get("from").u(0), e.get("to").u(0));
                let a = match self.get(from) {
                    Some(Rec::Sphere(s)) => s,
                    _ => return Err(format!("no sphere {}", from)),
                };
                let id = self.alloc();
                let port = match e.get("port").str().and_then(Port::of) {
                    Some(p) => p,
                    None => self.port_for(&a),
                };
                let mode = Strike::of(&e.get("mode").s(a.strike.name())).unwrap_or(a.strike);
                let route: Vec<[i32; 3]> = e.get("route").arr().iter().map(|c| c.i3([0, 0, 0])).collect();
                self.put(Rec::Link(Link { id, from, to, mode, port, from_pin: e.get("from_pin").s(""), to_pin: e.get("to_pin").s(""), route }));
                Ok(J::obj().with("id", id.into()))
            }
            "put" => {
                let tag = e.get("t").s("");
                let mut o = e.clone();
                if o.get("id").u(0) == 0 {
                    let id = self.alloc();
                    o.set("id", id.into());
                }
                let r = Rec::from_json(&tag, &o)?;
                let id = r.id();
                self.put(r);
                Ok(J::obj().with("id", id.into()))
            }
            "meta" => {
                self.edits.push(Edit::Meta(e.clone()));
                Ok(J::obj())
            }
            "batch" => {
                let mut res = Vec::new();
                for x in e.get("edits").arr() {
                    res.push(self.op(x)?);
                }
                Ok(J::obj().with("results", J::Arr(res)))
            }
            other => Err(format!("unknown edit '{}'", other)),
        }
    }

    pub fn port_for(&self, a: &Sphere) -> Port {
        if a.kind != SKind::Ratchet {
            return Port::Out;
        }
        let has_loop = self.w.links.values().any(|l| l.from == a.id && l.port == Port::Loop && !self.gone.contains(&l.id))
            || self.pending.iter().any(|r| matches!(r, Rec::Link(l) if l.from == a.id && l.port == Port::Loop));
        if has_loop { Port::Exit } else { Port::Loop }
    }

    pub fn finish(self) -> (Vec<Edit>, Id) {
        (self.edits, self.next)
    }
}

pub const LOG_MAX: usize = 256;

/// A number as s writes it: whole numbers without a point.
pub fn num_text(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{}", x)
    }
}

/// Degrees in (−180, 180].
pub fn norm_deg(x: f64) -> f64 {
    let mut r = x % 360.0;
    if r > 180.0 {
        r -= 360.0;
    }
    if r <= -180.0 {
        r += 360.0;
    }
    if r == 0.0 { 0.0 } else { r }
}

/// The shortest log that rebuilds a datum: its value, then each present part.
pub fn rebuild_log(d: &Datum) -> Vec<String> {
    let mut v = vec![format!("{} {}#", d.value, d.name)];
    for (n, x) in d.parts() {
        v.push(format!("{} {}/{}#", num_text(x), d.name, n));
    }
    v
}

/// Expands a studio edit and applies it with the strikes it causes, as one undoable step.
pub fn apply_op(w: &mut World, e: &J) -> Result<(Applied, J), String> {
    let (edits, res) = {
        let mut c = Ctx::new(w);
        let res = c.op(e)?;
        (c.edits, res)
    };
    let mut a = w.apply(edits)?;
    let strikes = crate::strike::strike_edits(w);
    if !strikes.is_empty() {
        let b = w.apply(strikes)?;
        let mut inv = b.inverse;
        inv.extend(a.inverse);
        a.inverse = inv;
        a.put.extend(b.put);
        a.del.extend(b.del);
    }
    Ok((a, res))
}

/// True when a name is a builtin, so typing it makes a command brick.
pub fn is_cmd(w: &str) -> bool {
    cmd_of(w).is_some()
}

/// An s file as a world: leading definitions go to a shelf, leading `{…} name$` lines become
/// script plates, every other line becomes a column of the main plate in order.
pub fn import_text(name: &str, text: &str) -> Result<World, String> {
    let mut w = World::new(name);
    let lines = split_lines(text);
    let mut shelf: Vec<String> = Vec::new();
    let mut scripts: Vec<(String, String)> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i].trim();
        let words = split_words(l)?;
        let def = words.len() == 3 && words[0].starts_with('{') && words[2] == "><" && words[1].starts_with('\'');
        let script = words.len() == 2 && words[0].starts_with('{') && words[0].ends_with('}') && words[1].ends_with('$') && words[1].len() > 1 && !words[1].ends_with("\\$");
        if def {
            shelf.push(l.to_string());
        } else if script {
            let body = &words[0][1..words[0].len() - 1];
            let nm = &words[1][..words[1].len() - 1];
            if normalize(body) != body || nm.contains('~') || nm.contains(':') {
                break;
            }
            scripts.push((nm.to_string(), body.to_string()));
        } else {
            break;
        }
        i += 1;
    }
    let rest: Vec<String> = lines[i..].to_vec();
    let mut x = 0;
    let edits;
    let next;
    {
        let mut c = Ctx::new(&w);
        if !shelf.is_empty() {
            let id = c.alloc();
            let wd = 12.max(shelf.len() as i32).min(64);
            c.put(Rec::Plate(Plate { id, space: WORLD, kind: PKind::Shelf, name: "shelf".into(), at: [0, -8, -12], w: wd, d: 1, arrow: Arrow::Px }));
            c.layout(id, &shelf.join("\n"))?;
        }
        for (nm, body) in &scripts {
            let id = c.alloc();
            c.put(Rec::Plate(Plate { id, space: WORLD, kind: PKind::Script, name: nm.clone(), at: [x, 0, -6], w: 6, d: 2, arrow: Arrow::Px }));
            x += 7;
            if !body.trim().is_empty() {
                c.layout(id, body)?;
            }
        }
        let id = c.alloc();
        let wd = 12;
        c.put(Rec::Plate(Plate { id, space: WORLD, kind: PKind::Main, name: "main".into(), at: [0, 0, 0], w: wd, d: ((rest.len() as i32 + wd - 1) / wd).max(1), arrow: Arrow::Px }));
        if !rest.is_empty() {
            c.layout(id, &rest.join("\n"))?;
        }
        let (e, n) = c.finish();
        edits = e;
        next = n;
    }
    w.apply(edits)?;
    w.next = w.next.max(next);
    Ok(w)
}
