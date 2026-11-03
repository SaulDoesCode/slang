// Questions the studio asks while editing: what a word is, what can be typed, how a line reads.
use crate::docs::*;
use crate::json::J;
use crate::rt::{active_form, canon_num, classify, cmd_of, is_ws, Interner, Op};
use crate::world::*;

/// Tokens of one s line with a class for coloring and a short reading.
pub fn lex(text: &str) -> J {
    let cs: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut err = J::Null;
    let mut it = Interner::new();
    let mut i = 0;
    while i < cs.len() {
        if is_ws(cs[i]) {
            i += 1;
            continue;
        }
        let from = i;
        let kind;
        let mut j = i;
        match cs[i] {
            '{' => {
                let mut d = 0;
                while j < cs.len() {
                    if cs[j] == '{' {
                        d += 1;
                    } else if cs[j] == '}' {
                        d -= 1;
                        if d == 0 {
                            j += 1;
                            break;
                        }
                    }
                    j += 1;
                }
                if d != 0 {
                    err = J::obj().with("at", from.into()).with("msg", "unclosed {".into());
                }
                kind = "brace";
                while j < cs.len() && !is_ws(cs[j]) {
                    j += 1;
                }
            }
            '\'' => {
                j += 1;
                while j < cs.len() && cs[j] != '\'' {
                    j += 1;
                }
                if j >= cs.len() {
                    err = J::obj().with("at", from.into()).with("msg", "unclosed '".into());
                } else {
                    j += 1;
                }
                kind = "quote";
            }
            '⟨' => {
                while j < cs.len() && cs[j] != '⟩' {
                    j += 1;
                }
                if j >= cs.len() {
                    err = J::obj().with("at", from.into()).with("msg", "unclosed ⟨".into());
                }
                while j < cs.len() && !is_ws(cs[j]) {
                    j += 1;
                }
                kind = "angle";
            }
            _ => {
                while j < cs.len() && !is_ws(cs[j]) {
                    j += 1;
                }
                let w: String = cs[from..j].iter().collect();
                kind = word_class(&w, &mut it);
            }
        }
        let w: String = cs[from..j.min(cs.len())].iter().collect();
        out.push(J::obj().with("from", from.into()).with("to", j.min(cs.len()).into()).with("kind", kind.into()).with("w", w.into()));
        i = j.max(i + 1);
    }
    J::obj().with("tokens", J::Arr(out)).with("error", err)
}

pub fn word_class(w: &str, it: &mut Interner) -> &'static str {
    if canon_num(w).is_some() {
        return "num";
    }
    if cmd_of(w).is_some() {
        return "cmd";
    }
    if w.len() > 1 && w.starts_with('\\') {
        return "esc";
    }
    if crate::graph::HOST_WORDS.contains(&w) {
        return "geo";
    }
    if lib_doc(w).is_some() {
        return "lib";
    }
    if active_form(w) {
        let mut op = classify(w, it);
        while let Op::Cond(_, o) = op {
            op = *o;
        }
        return match op {
            Op::Lookup(_) => "look",
            Op::Pop(_) => "pop",
            Op::Def(_) => "def",
            Op::SetMem(_) => "set",
            Op::SetNum(_) | Op::Inc(_) | Op::Dec(_) => "numset",
            Op::Call(_) | Op::CallPass(_) => "call",
            Op::Cmd(_) | Op::Native(_) => "cmd",
            _ => "word",
        };
    }
    "word"
}

/// A definition column on a shelf: {…} 'word' ><.
pub struct Def {
    pub word: String,
    pub column: Vec<Id>,
    pub gen: usize,
    pub doc: String,
}

/// Every shelf definition in a zone (None: main), in generation order.
pub fn defs(w: &World, zone: Option<Id>) -> Vec<Def> {
    let mut shelves: Vec<&Plate> = w.plates.values().filter(|p| p.space == WORLD && p.kind == PKind::Shelf && w.zone_of_plate(p).map(|z| z.id) == zone).collect();
    shelves.sort_by_key(|p| (p.at[1], p.at[0], p.at[2], p.id));
    let mut out = Vec::new();
    for p in shelves {
        for col in w.columns(p.id) {
            let bs: Vec<&Brick> = col.iter().filter_map(|id| w.bricks.get(id)).collect();
            if let Some(i) = bs.iter().position(|b| b.kind == BKind::Cmd && b.word == "><") {
                if i >= 2 {
                    let key = bs[i - 1];
                    let word = match key.kind {
                        BKind::Quote | BKind::Lit | BKind::Lib | BKind::Geo | BKind::Num | BKind::Cmd => key.word.clone(),
                        _ => continue,
                    };
                    let gen = out.len();
                    out.push(Def { word, column: col.clone(), gen, doc: key.note.clone() });
                }
            }
        }
    }
    out
}

fn zone_id_of_brick(w: &World, b: &Brick) -> Option<Id> {
    if b.space == WORLD {
        w.zone_of_cell(b.cell).map(|z| z.id)
    } else {
        w.zone_of_space(b.space).map(|z| z.id)
    }
}

/// The definition column a brick belongs to, if it is inside one.
fn own_def(w: &World, b: &Brick, ds: &[Def]) -> Option<usize> {
    let mut cur = b.clone();
    for _ in 0..64 {
        if let Some(d) = ds.iter().find(|d| d.column.contains(&cur.id)) {
            return Some(d.gen);
        }
        match w.spaces.get(&cur.space).map(|s| s.owner.clone()) {
            Some(Owner::Box(o)) => match w.bricks.get(&o) {
                Some(x) => cur = x.clone(),
                None => return None,
            },
            _ => return None,
        }
    }
    None
}

/// What a brick's word stands for where it is.
pub fn resolve(w: &World, id: Id) -> J {
    let b = match w.bricks.get(&id) {
        Some(b) => b,
        None => return J::obj().with("kind", "none".into()),
    };
    let zone = zone_id_of_brick(w, b);
    let ds = defs(w, zone);
    let limit = own_def(w, b, &ds).unwrap_or(usize::MAX);
    let visible_def = |word: &str| ds.iter().filter(|d| d.word == word && d.gen < limit).last();
    let mut o = J::obj().with("brick", id.into()).with("word", b.word.clone().into()).with("text", word_text(b).into());
    let word = b.word.as_str();
    match b.kind {
        BKind::Box => {
            let inner = w.space_text(b.inner);
            return o.with("kind", "box".into()).with("doc", "quotation: data until something runs it".into()).with("inside", inner.into());
        }
        BKind::Quote => return o.with("kind", "quote".into()).with("doc", "raw text, pushed as one value".into()),
        BKind::Num => return o.with("kind", "num".into()).with("doc", "number literal".into()),
        BKind::Read => return o.with("kind", "read".into()).with("doc", format!("the value of {} as data", word).into()),
        BKind::Sock => return o.with("kind", "sock".into()).with("doc", format!("socket: the value of {} is written into the box's text", word).into()),
        _ => {}
    }
    if b.kind == BKind::Name {
        let what = match b.cap.as_str() {
            "~" => "look up and run its value as a word",
            "`" => "mem = the line",
            "#" => "numbers = first value",
            ";" => "numbers += first value or 1",
            ":" => "numbers −= first value or 1",
            "$" => "define the script from the line",
            "." => "call the script (one value on the line = repeat count)",
            ".." => "call the script once on the current line",
            "@" => "move the top value into mem",
            _ => "",
        };
        o.set("doc", format!("{} {}", word, what).into());
        if b.cap == "." || b.cap == ".." {
            if let Some(p) = w.plates.values().find(|p| p.kind == PKind::Script && p.name == word) {
                return o.with("kind", "script".into()).with("plate", p.id.into());
            }
            if let Some(s) = w.sphere_named(word) {
                return o.with("kind", "sphere".into()).with("sphere", s.id.into());
            }
            if crate::LIB.iter().any(|f| f.0 == word) {
                return o.with("kind", "file".into()).with("file", word.into());
            }
            return o.with("kind", "call".into());
        }
        if word.starts_with(':') {
            return o.with("kind", "local".into());
        }
        return o.with("kind", "name".into());
    }
    if let Some(d) = visible_def(word) {
        let doc = if d.doc.is_empty() { format!("defined on a shelf, generation {}", d.gen) } else { d.doc.clone() };
        return o.with("kind", "def".into()).with("column", J::Arr(d.column.iter().map(|x| J::Num(*x as f64)).collect())).with("gen", d.gen.into()).with("doc", doc.into());
    }
    if b.kind == BKind::Esc {
        let c = cmd_doc(word);
        return o.with("kind", "esc".into()).with("doc", c.map(|c| format!("{} — {} (never replaced)", c.2, c.3)).unwrap_or_else(|| format!("the word {} as plain text", word)).into());
    }
    if let Some(c) = cmd_doc(word) {
        return o.with("kind", "cmd".into()).with("sig", c.2.into()).with("doc", c.3.into()).with("group", c.0.into());
    }
    if let Some(h) = HOSTDOC.iter().find(|h| h.0 == word) {
        return o.with("kind", "geo".into()).with("sig", h.1.into()).with("doc", h.2.into());
    }
    if let Some(l) = lib_doc(word) {
        return o.with("kind", "lib".into()).with("file", l.file.into()).with("doc", l.doc.clone().into());
    }
    o.with("kind", "word".into()).with("doc", "literal word, pushed as text".into())
}

/// Words that can be typed at a brick or cell, best first.
pub fn complete(w: &World, prefix: &str, zone: Option<Id>) -> J {
    let mut out: Vec<(u8, String, &'static str, String)> = Vec::new();
    let mut add = |rank: u8, word: String, kind: &'static str, doc: String| {
        if word.starts_with(prefix) && !out.iter().any(|x| x.1 == word) {
            out.push((rank, word, kind, doc));
        }
    };
    for d in defs(w, zone) {
        add(0, d.word.clone(), "def", if d.doc.is_empty() { "shelf definition".into() } else { d.doc.clone() });
    }
    for p in w.plates.values().filter(|p| p.kind == PKind::Script) {
        add(1, format!("{}.", p.name), "call", "call the script".into());
    }
    for s in w.spheres.values() {
        add(2, format!("{}.", s.name), "call", format!("run sphere {}'s body", s.name));
    }
    let mut names: Vec<String> = w.bricks.values().filter(|b| b.kind == BKind::Name && matches!(b.cap.as_str(), "`" | "#" | "@" | ";" | ":")).map(|b| b.word.clone()).collect();
    names.sort();
    names.dedup();
    for n in names {
        add(3, format!("{}~", n), "look", "look up the name".into());
    }
    for c in CMDDOC {
        add(4, c.1.to_string(), "cmd", format!("{} — {}", c.2, c.3));
    }
    for h in HOSTDOC {
        add(5, h.0.to_string(), "geo", format!("{} — {}", h.1, h.2));
    }
    for l in lib_words() {
        add(6, l.word.clone(), "lib", format!("{} · {}", l.doc, l.file));
    }
    out.sort_by(|a, b| (a.1.len() != prefix.len(), a.0, a.1.len()).cmp(&(b.1.len() != prefix.len(), b.0, b.1.len())));
    out.truncate(40);
    J::Arr(out.into_iter().map(|(_, w, k, d)| J::obj().with("w", w.into()).with("kind", k.into()).with("doc", d.into())).collect())
}

/// What typing a word would make, without placing it.
pub fn typed(word: &str) -> J {
    match classify_word(word, &|w| lib_doc(w).is_some()) {
        Ok(t) => J::obj()
            .with("kind", t.kind.name().into())
            .with("word", t.word.into())
            .with("cap", t.cap.into())
            .with("cond", t.cond.into()),
        Err(e) => J::obj().with("error", e.into()),
    }
}
