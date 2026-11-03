// The whole world as s text: one line per plate, sphere, device, link, zone and assembly,
// read back by a dialect in which these words build the world instead of running.
use crate::json::J;
use crate::ops::Ctx;
use crate::world::*;

fn num(x: f64) -> String {
    if x == x.trunc() && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{}", x)
    }
}

fn balanced(t: &str) -> bool {
    let mut d = 0i64;
    for c in t.chars() {
        match c {
            '{' => d += 1,
            '}' => {
                d -= 1;
                if d < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    d == 0
}

/// A text value: a quote, a box when it holds quote marks, or a percent-coded quote.
fn txt(t: &str) -> String {
    if !t.contains('\'') && !t.starts_with("%:") {
        format!("'{}'", t)
    } else if balanced(t) {
        format!("{{{}}}", t)
    } else {
        let mut o = String::from("'%:");
        for c in t.chars() {
            match c {
                '\'' => o.push_str("%27"),
                '%' => o.push_str("%25"),
                '{' => o.push_str("%7B"),
                '}' => o.push_str("%7D"),
                c => o.push(c),
            }
        }
        o.push('\'');
        o
    }
}

fn body(t: &str) -> String {
    if balanced(t) {
        format!("{{{}}}", t)
    } else {
        txt(t)
    }
}

fn plate_text(w: &World, p: Id) -> String {
    let mut s = TW::default();
    w.plate_text_into(p, &mut s, &mut Vec::new(), 0);
    s.s
}

fn space_text(w: &World, sp: Id) -> String {
    let mut s = TW::default();
    w.space_text_into(sp, &mut s, &mut Vec::new(), 0);
    s.s
}

fn endpoint(w: &World, id: Id, pin: &str) -> String {
    let n = w.spheres.get(&id).map(|s| s.name.clone()).unwrap_or_default();
    if pin.is_empty() {
        txt(&n)
    } else {
        txt(&format!("{}@{}", n, pin))
    }
}

fn graph_lines(w: &World, space: Id, out: &mut String) {
    let mut spheres: Vec<&Sphere> = w.spheres.values().filter(|s| s.space == space).collect();
    spheres.sort_by_key(|s| s.id);
    for s in &spheres {
        let b = if s.kind == SKind::Asm { String::from("{}") } else { body(&space_text(w, s.body)) };
        match s.cell {
            Some(c) => out.push_str(&format!("{} {} '{}' {} {} {} {} {} {} device.\n", b, txt(&s.name), s.kind.name(), c[0], c[1], c[2], num(s.aim[0]), num(s.aim[1]), num(s.aim[2]))),
            None => out.push_str(&format!("{} {} '{}' {} {} {} {} {} {} sphere.\n", b, txt(&s.name), s.kind.name(), num(s.pos[0]), num(s.pos[1]), num(s.pos[2]), num(s.aim[0]), num(s.aim[1]), num(s.aim[2]))),
        }
        if s.kind == SKind::Asm {
            if let Some(a) = assembly_name(w, s.body) {
                out.push_str(&format!("{} {} uses.\n", txt(&s.name), txt(a)));
            }
        }
        if (s.r - if s.kind == SKind::Asm { 0.9 } else if s.kind == SKind::Port { 0.5 } else { 0.8 }).abs() > 1e-12 {
            out.push_str(&format!("{} 'r' {} with.\n", txt(&s.name), num(s.r)));
        }
        if s.reach != 60.0 {
            out.push_str(&format!("{} 'reach' {} with.\n", txt(&s.name), num(s.reach)));
        }
        if s.strike == Strike::Ray {
            out.push_str(&format!("{} 'ray' with.\n", txt(&s.name)));
        }
        let defaults: Vec<(&str, f64)> = match s.kind {
            SKind::Ratchet => vec![("n", 5.0)],
            SKind::Src => vec![("every", 0.0), ("run", 1.0)],
            _ => vec![],
        };
        for (k, v) in &s.params {
            if defaults.iter().any(|d| d.0 == k && d.1 == *v) {
                continue;
            }
            out.push_str(&format!("{} {} {} param.\n", txt(&s.name), txt(k), num(*v)));
        }
        for (k, _) in &defaults {
            if !s.params.iter().any(|p| &p.0 == k) {
                out.push_str(&format!("{} {} unparam.\n", txt(&s.name), txt(k)));
            }
        }
        if let Some(l) = s.lift {
            out.push_str(&format!("{} {} {} {} lift.\n", txt(&s.name), num(l[0]), num(l[1]), num(l[2])));
        }
    }
    let mut links: Vec<&Link> = w.links.values().filter(|l| spheres.iter().any(|s| s.id == l.from)).collect();
    links.sort_by_key(|l| l.id);
    for l in links {
        let mut a = format!("{} {}", endpoint(w, l.from, &l.from_pin), endpoint(w, l.to, &l.to_pin));
        if l.port != Port::Out {
            a.push_str(&format!(" '{}'", l.port.name()));
        }
        for c in &l.route {
            a.push_str(&format!(" {} {} {}", c[0], c[1], c[2]));
        }
        out.push_str(&format!("{} {}.\n", a, if l.mode == Strike::Ray { "beam" } else { "wire" }));
    }
    for s in &spheres {
        for m in &s.muted {
            if let Some(t) = w.spheres.get(m) {
                out.push_str(&format!("{} {} mute.\n", txt(&s.name), txt(&t.name)));
            }
        }
    }
    for d in w.datums.values().filter(|d| d.space == space) {
        out.push_str(&format!("{} {} {} {} {} datum.\n", txt(&d.name), d.cell[0], d.cell[1], d.cell[2], d.value));
        for (k, v) in d.parts() {
            out.push_str(&format!("{} '{}' {} part.\n", txt(&d.name), k, num(v)));
        }
        out.push_str(&format!("{} {} log.\n", txt(&d.name), body(&d.log.join("\n"))));
    }
    for t in w.tracks.values().filter(|t| t.space == space) {
        let cells: Vec<String> = t.cells.iter().map(|c| format!("{} {} {}", c[0], c[1], c[2])).collect();
        out.push_str(&format!("{} {} {} track.\n", txt(&t.name), t.speed, cells.join(" ")));
    }
}

/// The world as one s text in the graph dialect.
pub fn world_text(w: &World) -> String {
    let mut out = String::new();
    out.push_str(&format!("{} world.\n", txt(&w.name)));
    for (k, v) in w.params.entries() {
        let v = match v {
            J::Num(x) => num(*x),
            J::Str(s) => txt(s),
            J::Bool(b) => if *b { "1".into() } else { "0".into() },
            v => txt(&v.dump()),
        };
        out.push_str(&format!("{} {} wparam.\n", txt(k), v));
    }
    if !w.goal.is_null() {
        out.push_str(&format!("{} goal.\n", txt(&w.goal.dump())));
    }
    for z in w.zones.values() {
        if z.kind == ZKind::Scope {
            let d = z.params.iter().find(|p| p.0 == "dialect").map(|p| p.1.clone()).unwrap_or_else(|| "s".into());
            out.push_str(&format!("{} {} {} {} {} {} {} {} scope.\n", txt(&z.name), txt(&d), z.min[0], z.min[1], z.min[2], z.max[0], z.max[1], z.max[2]));
            for (n, m) in &z.captures {
                out.push_str(&format!("{} {} '{}' capture.\n", txt(&z.name), txt(n), m.name()));
            }
            for (k, v) in z.params.iter().filter(|p| p.0 != "dialect") {
                out.push_str(&format!("{} {} {} zparam.\n", txt(&z.name), txt(k), txt(v)));
            }
            continue;
        }
        out.push_str(&format!("{} '{}' {} {} {} {} {} {} zone.\n", txt(&z.name), z.kind.name(), z.min[0], z.min[1], z.min[2], z.max[0], z.max[1], z.max[2]));
        for (n, m) in &z.captures {
            out.push_str(&format!("{} {} '{}' capture.\n", txt(&z.name), txt(n), m.name()));
        }
        for (k, v) in &z.params {
            out.push_str(&format!("{} {} {} zparam.\n", txt(&z.name), txt(k), txt(v)));
        }
    }
    for p in w.space_plates(WORLD) {
        out.push_str(&format!("{} {} '{}' {} {} {} {} {} '{}' plate.\n", body(&plate_text(w, p.id)), txt(&p.name), p.kind.name(), p.at[0], p.at[1], p.at[2], p.w, p.d, p.arrow.name()));
    }
    let mut asms: Vec<(&Id, &Space)> = w.spaces.iter().filter(|(_, s)| matches!(s.owner, Owner::Assembly(_))).collect();
    asms.sort_by_key(|x| *x.0);
    for (id, sp) in asms {
        let mut inner = String::new();
        graph_lines(w, *id, &mut inner);
        if let Owner::Assembly(n) = &sp.owner {
            out.push_str(&format!("{{\n{}}} {} assembly.\n", inner, txt(n)));
        }
    }
    graph_lines(w, WORLD, &mut out);
    for b in w.bricks.values().filter(|b| b.space == WORLD && !b.note.is_empty()) {
        out.push_str(&format!("{} {} {} {} note.\n", b.cell[0], b.cell[1], b.cell[2], txt(&b.note)));
    }
    out
}

// reading

#[derive(Clone, Debug)]
enum Tok {
    Q(String),
    N(f64),
}

fn lex(src: &str) -> Result<Vec<(usize, Result<Tok, String>)>, String> {
    // each item: (line, Ok(value) or Err(word))
    let cs: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line = 1;
    while i < cs.len() {
        let c = cs[i];
        if c == '\n' {
            line += 1;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let at = line;
        if c == '{' {
            let mut d = 1;
            let mut j = i + 1;
            while j < cs.len() && d > 0 {
                match cs[j] {
                    '{' => d += 1,
                    '}' => d -= 1,
                    '\n' => line += 1,
                    _ => {}
                }
                j += 1;
            }
            if d != 0 {
                return Err(format!("line {}: unclosed {{", at));
            }
            out.push((at, Ok(Tok::Q(cs[i + 1..j - 1].iter().collect()))));
            i = j;
            continue;
        }
        if c == '\'' {
            let mut j = i + 1;
            while j < cs.len() && cs[j] != '\'' {
                if cs[j] == '\n' {
                    line += 1;
                }
                j += 1;
            }
            if j >= cs.len() {
                return Err(format!("line {}: unclosed '", at));
            }
            let t: String = cs[i + 1..j].iter().collect();
            out.push((at, Ok(Tok::Q(unpct(&t)))));
            i = j + 1;
            continue;
        }
        let mut j = i;
        while j < cs.len() && !cs[j].is_whitespace() {
            j += 1;
        }
        let w: String = cs[i..j].iter().collect();
        match w.parse::<f64>() {
            Ok(x) if !w.ends_with('.') => out.push((at, Ok(Tok::N(x)))),
            _ => out.push((at, Err(w))),
        }
        i = j;
    }
    Ok(out)
}

fn unpct(t: &str) -> String {
    let Some(rest) = t.strip_prefix("%:") else { return t.to_string() };
    let mut o = String::new();
    let cs: Vec<char> = rest.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '%' && i + 3 <= cs.len() {
            let h: String = cs[i + 1..i + 3].iter().collect();
            if let Ok(b) = u8::from_str_radix(&h, 16) {
                o.push(b as char);
                i += 3;
                continue;
            }
        }
        o.push(cs[i]);
        i += 1;
    }
    o
}

struct Reader {
    w: World,
    /// Assembly placements waiting for their `uses.` line.
    pending_place: Vec<J>,
}

fn q(a: &[Tok], i: usize, word: &str) -> Result<String, String> {
    match a.get(i) {
        Some(Tok::Q(s)) => Ok(s.clone()),
        Some(Tok::N(x)) => Ok(num(*x)),
        None => Err(format!("{}: argument {} is missing", word, i + 1)),
    }
}

fn n(a: &[Tok], i: usize, word: &str) -> Result<f64, String> {
    match a.get(i) {
        Some(Tok::N(x)) => Ok(*x),
        Some(Tok::Q(s)) => s.parse().map_err(|_| format!("{}: argument {} is not a number", word, i + 1)),
        None => Err(format!("{}: argument {} is missing", word, i + 1)),
    }
}

impl Reader {
    fn op(&mut self, e: J) -> Result<J, String> {
        let (edits, res) = {
            let mut c = Ctx::new(&self.w);
            let r = c.op(&e)?;
            (c.edits, r)
        };
        self.w.apply(edits)?;
        Ok(res)
    }

    fn sphere_id(&self, name: &str) -> Result<Id, String> {
        self.w.sphere_named(name).map(|s| s.id).ok_or_else(|| format!("no sphere {}", name))
    }

    fn put_sphere(&mut self, f: impl FnOnce(&mut Sphere), name: &str) -> Result<(), String> {
        let mut s = self.w.sphere_named(name).cloned().ok_or_else(|| format!("no sphere {}", name))?;
        f(&mut s);
        self.w.apply(vec![Edit::Put(Rec::Sphere(s))])?;
        Ok(())
    }

    fn run(&mut self, src: &str, space: Id) -> Result<(), String> {
        let toks = lex(src)?;
        let mut args: Vec<Tok> = Vec::new();
        for (line, t) in toks {
            match t {
                Ok(v) => args.push(v),
                Err(word) => {
                    let a = std::mem::take(&mut args);
                    self.word(&word, &a, space).map_err(|e| format!("line {}: {}", line, e))?;
                }
            }
        }
        if !args.is_empty() {
            return Err("text ends with values no word takes".into());
        }
        Ok(())
    }

    fn word(&mut self, word: &str, a: &[Tok], space: Id) -> Result<(), String> {
        match word {
            "world." => self.w.name = q(a, 0, word)?,
            "wparam." => {
                let k = q(a, 0, word)?;
                let v = match a.get(1) {
                    Some(Tok::N(x)) => J::Num(*x),
                    Some(Tok::Q(s)) => J::Str(s.clone()),
                    None => J::Null,
                };
                let v = if k == "lockstep" { J::Bool(v.num().map_or(false, |x| x != 0.0)) } else { v };
                let mut p = self.w.params.clone();
                if !matches!(p, J::Obj(_)) {
                    p = J::obj();
                }
                p.set(&k, v);
                self.w.params = p;
            }
            "goal." => self.w.goal = J::parse(&q(a, 0, word)?)?,
            "zone." => {
                let name = q(a, 0, word)?;
                let kind = q(a, 1, word)?;
                let v: Vec<f64> = (2..8).map(|i| n(a, i, word)).collect::<Result<_, _>>()?;
                self.op(J::obj()
                    .with("op", "zone".into())
                    .with("name", name.into())
                    .with("kind", kind.into())
                    .with("min", J::Arr(v[0..3].iter().map(|x| J::Num(*x)).collect()))
                    .with("max", J::Arr(v[3..6].iter().map(|x| J::Num(*x)).collect())))?;
            }
            "scope." => {
                let name = q(a, 0, word)?;
                let dialect = q(a, 1, word)?;
                let v: Vec<f64> = (2..8).map(|i| n(a, i, word)).collect::<Result<_, _>>()?;
                self.op(J::obj()
                    .with("op", "zone".into())
                    .with("name", name.into())
                    .with("kind", "scope".into())
                    .with("params", J::obj().with("dialect", dialect.into()))
                    .with("min", J::Arr(v[0..3].iter().map(|x| J::Num(*x)).collect()))
                    .with("max", J::Arr(v[3..6].iter().map(|x| J::Num(*x)).collect())))?;
            }
            "capture." | "zparam." => {
                let name = q(a, 0, word)?;
                let mut z = self.w.zones.values().find(|z| z.name == name).cloned().ok_or_else(|| format!("no zone {}", name))?;
                if word == "capture." {
                    z.captures.push((q(a, 1, word)?, Mode::of(&q(a, 2, word)?).ok_or("unknown capture mode")?));
                } else {
                    z.params.push((q(a, 1, word)?, q(a, 2, word)?));
                }
                self.w.apply(vec![Edit::Put(Rec::Zone(z))])?;
            }
            "plate." => {
                let text = q(a, 0, word)?;
                let name = q(a, 1, word)?;
                let kind = q(a, 2, word)?;
                let at: Vec<f64> = (3..6).map(|i| n(a, i, word)).collect::<Result<_, _>>()?;
                let r = self.op(J::obj()
                    .with("op", "plate".into())
                    .with("space", space.into())
                    .with("kind", kind.into())
                    .with("name", name.into())
                    .with("at", J::Arr(at.iter().map(|x| J::Num(*x)).collect()))
                    .with("w", J::Num(n(a, 6, word)?))
                    .with("d", J::Num(n(a, 7, word)?))
                    .with("arrow", q(a, 8, word)?.into()))?;
                if !text.trim().is_empty() {
                    self.op(J::obj().with("op", "settext".into()).with("plate", r.get("id").clone()).with("text", text.into()))?;
                }
            }
            "assembly." => {
                let inner = q(a, 0, word)?;
                let name = q(a, 1, word)?;
                let id = self.w.alloc();
                self.w.apply(vec![Edit::Put(Rec::Space(Space { id, owner: Owner::Assembly(name) }))])?;
                self.run(&inner, id)?;
            }
            "sphere." | "device." => {
                let text = q(a, 0, word)?;
                let name = q(a, 1, word)?;
                let kind = q(a, 2, word)?;
                let p: Vec<f64> = (3..9).map(|i| n(a, i, word)).collect::<Result<_, _>>()?;
                if kind == "asm" {
                    // placed when its assembly is known (`uses.`)
                    let mut e = J::obj().with("name", name.into()).with("aim", J::Arr(p[3..6].iter().map(|x| J::Num(*x)).collect())).with("space", space.into());
                    if word == "device." {
                        e.set("cell", J::Arr(p[0..3].iter().map(|x| J::Num(*x)).collect()));
                    } else {
                        e.set("pos", J::Arr(p[0..3].iter().map(|x| J::Num(*x)).collect()));
                    }
                    self.pending_place.push(e);
                    return Ok(());
                }
                let mut e = J::obj()
                    .with("op", "sphere".into())
                    .with("kind", kind.into())
                    .with("name", name.into())
                    .with("aim", J::Arr(p[3..6].iter().map(|x| J::Num(*x)).collect()))
                    .with("code", text.into())
                    .with("space", space.into())
                    .with("params", J::obj());
                if word == "device." {
                    e.set("cell", J::Arr(p[0..3].iter().map(|x| J::Num(*x)).collect()));
                } else {
                    e.set("pos", J::Arr(p[0..3].iter().map(|x| J::Num(*x)).collect()));
                }
                self.op(e)?;
            }
            "uses." => {
                let name = q(a, 0, word)?;
                let asm = q(a, 1, word)?;
                let i = self.pending_place.iter().position(|e| e.get("name").str() == Some(name.as_str())).ok_or_else(|| format!("uses: no placement {}", name))?;
                let mut e = self.pending_place.remove(i);
                e.set("op", "instance".into());
                e.set("assembly", asm.into());
                self.op(e)?;
            }
            "with." => {
                let name = q(a, 0, word)?;
                let k = q(a, 1, word)?;
                let v = a.get(2).map(|_| n(a, 2, word)).transpose()?;
                self.put_sphere(
                    |s| match (k.as_str(), v) {
                        ("r", Some(x)) => s.r = x,
                        ("reach", Some(x)) => s.reach = x,
                        ("ray", _) => s.strike = Strike::Ray,
                        _ => {}
                    },
                    &name,
                )?;
            }
            "param." | "unparam." => {
                let name = q(a, 0, word)?;
                let k = q(a, 1, word)?;
                let v = if word == "param." { Some(n(a, 2, word)?) } else { None };
                self.put_sphere(
                    |s| {
                        s.params.retain(|p| p.0 != k);
                        if let Some(v) = v {
                            s.params.push((k.clone(), v));
                        }
                        if let Some(c) = s.cell {
                            s.pos = device_pos(s, c);
                        }
                    },
                    &name,
                )?;
            }
            "lift." => {
                let name = q(a, 0, word)?;
                let l = [n(a, 1, word)?, n(a, 2, word)?, n(a, 3, word)?];
                self.put_sphere(|s| s.lift = Some(l), &name)?;
            }
            "wire." | "beam." => {
                let split = |t: String| -> (String, String) {
                    match t.split_once('@') {
                        Some((a, b)) => (a.to_string(), b.to_string()),
                        None => (t, String::new()),
                    }
                };
                let (from, from_pin) = split(q(a, 0, word)?);
                let (to, to_pin) = split(q(a, 1, word)?);
                let mut port = "out".to_string();
                let mut nums = Vec::new();
                for t in &a[2..] {
                    match t {
                        Tok::Q(s) => port = s.clone(),
                        Tok::N(x) => nums.push(*x),
                    }
                }
                if nums.len() % 3 != 0 {
                    return Err(format!("{}: a route is cells of three numbers", word));
                }
                let route = J::Arr(nums.chunks(3).map(|c| J::Arr(c.iter().map(|x| J::Num(*x)).collect())).collect());
                let f = self.sphere_id(&from)?;
                let t = self.sphere_id(&to)?;
                self.op(J::obj()
                    .with("op", "link".into())
                    .with("from", f.into())
                    .with("to", t.into())
                    .with("mode", if word == "beam." { "ray" } else { "sticky" }.into())
                    .with("port", port.into())
                    .with("from_pin", from_pin.into())
                    .with("to_pin", to_pin.into())
                    .with("route", route))?;
            }
            "datum." => {
                let name = q(a, 0, word)?;
                let c: Vec<f64> = (1..4).map(|i| n(a, i, word)).collect::<Result<_, _>>()?;
                self.op(J::obj()
                    .with("op", "datum".into())
                    .with("name", name.into())
                    .with("space", space.into())
                    .with("cell", J::Arr(c.iter().map(|x| J::Num(*x)).collect()))
                    .with("value", J::Num(n(a, 4, word)?)))?;
            }
            "track." => {
                let name = q(a, 0, word)?;
                let speed = n(a, 1, word)?;
                if a.len() < 5 || (a.len() - 2) % 3 != 0 {
                    return Err("track: a name, a speed, then x y z for each cell".into());
                }
                let mut cells = Vec::new();
                for i in (2..a.len()).step_by(3) {
                    cells.push(J::Arr(vec![J::Num(n(a, i, word)?), J::Num(n(a, i + 1, word)?), J::Num(n(a, i + 2, word)?)]));
                }
                self.op(J::obj()
                    .with("op", "track".into())
                    .with("name", name.into())
                    .with("space", space.into())
                    .with("speed", J::Num(speed))
                    .with("cells", J::Arr(cells)))?;
            }
            "part." | "log." => {
                let name = q(a, 0, word)?;
                let mut d = self.w.datum_named(&name).cloned().ok_or_else(|| format!("no datum {}", name))?;
                if word == "part." {
                    let k = q(a, 1, word)?;
                    let i = PART_NAMES.iter().position(|p| *p == k).ok_or_else(|| format!("part: {} is not one of sx sy sz rx ry rz", k))?;
                    d.set_part(i, n(a, 2, word)?);
                } else {
                    let t = q(a, 1, word)?;
                    d.log = t.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).map(|l| l.to_string()).collect();
                }
                self.w.apply(vec![Edit::Put(Rec::Datum(d))])?;
            }
            "mute." => {
                let t = self.sphere_id(&q(a, 1, word)?)?;
                self.put_sphere(|s| s.muted.push(t), &q(a, 0, word)?)?;
            }
            "note." => {
                let c = [n(a, 0, word)? as i32, n(a, 1, word)? as i32, n(a, 2, word)? as i32];
                let t = q(a, 3, word)?;
                let mut b = self.w.brick_at(WORLD, c).cloned().ok_or_else(|| format!("note: no brick at {:?}", c))?;
                b.note = t;
                self.w.apply(vec![Edit::Put(Rec::Brick(b))])?;
            }
            w => return Err(format!("{} is not a word of the world text", w)),
        }
        Ok(())
    }
}

/// Builds a world from its text. Links are exactly those the text names: nothing is struck.
pub fn world_from_text(src: &str) -> Result<World, String> {
    let mut r = Reader { w: World::new("world"), pending_place: Vec::new() };
    r.run(src, WORLD)?;
    if let Some(e) = r.pending_place.first() {
        return Err(format!("{} names no assembly (uses.)", e.get("name").s("")));
    }
    Ok(r.w)
}
