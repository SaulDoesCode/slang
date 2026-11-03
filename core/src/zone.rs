// Zones: what crosses between them (messages), what a name carries (stored), and the
// capture rules a zone puts on its parent's names when it spawns.
use crate::json::J;
use crate::rt::*;
use crate::wire::{self, Reader};

/// Everything stored under one name.
#[derive(Default)]
pub struct Stored {
    pub mem: Option<String>,
    pub num: Option<f64>,
    pub list: Option<Vec<String>>,
    pub script: Option<String>,
    pub handle: Option<FileH>,
}

impl Stored {
    pub fn is_empty(&self) -> bool {
        self.mem.is_none() && self.num.is_none() && self.list.is_none() && self.script.is_none() && self.handle.is_none()
    }
    pub fn copy(&self) -> Stored {
        Stored { mem: self.mem.clone(), num: self.num, list: self.list.clone(), script: self.script.clone(), handle: None }
    }
}

pub fn take_stored(rt: &mut Rt, name: &str, take: bool) -> Stored {
    let k = rt.it.sym(name);
    let i = k.0 as usize;
    let mut s = Stored::default();
    rt.epoch += 1;
    s.mem = rt.mem.get(i).and_then(|v| v.as_ref()).map(|v| v.text().into_owned());
    s.num = rt.nums.get(i).copied().flatten();
    s.list = rt.lists.get(i).and_then(|v| v.as_ref()).map(|l| l.texts());
    s.script = rt.script_text(k);
    if take {
        if let Some(v) = rt.mem.get_mut(i) {
            *v = None;
        }
        if let Some(v) = rt.nums.get_mut(i) {
            *v = None;
        }
        if let Some(v) = rt.lists.get_mut(i) {
            *v = None;
        }
        if let Some(v) = rt.scripts.get_mut(i) {
            *v = None;
        }
        s.handle = rt.files.remove(&k.0);
    }
    s
}

pub fn put_stored(rt: &mut Rt, name: &str, s: Stored) {
    let k = rt.it.sym(name);
    let i = k.0 as usize;
    let grow_to = |n: usize, l: usize| n.max(l + 1);
    if rt.mem.len() <= i {
        rt.mem.resize(grow_to(rt.mem.len(), i), None);
    }
    if rt.nums.len() <= i {
        rt.nums.resize(grow_to(rt.nums.len(), i), None);
    }
    if rt.lists.len() <= i {
        rt.lists.resize_with(i + 1, || None);
    }
    rt.epoch += 1;
    rt.mem[i] = s.mem.map(Val::of);
    rt.nums[i] = s.num;
    rt.lists[i] = s.list.map(|l| List::from_vals(l.into_iter().map(Val::of).collect()));
    if let Some(t) = s.script {
        let _ = rt.def_script_text(name, &t);
    }
    if let Some(h) = s.handle {
        rt.files.insert(k.0, h);
    }
}

pub fn has_handle(rt: &Rt, name: &str) -> bool {
    rt.it.map.get(name).map_or(false, |k| rt.files.contains_key(k))
}

/// What crosses between zones.
pub enum Msg {
    Spawn { defs: Vec<(String, String)>, guards: Vec<String>, caps: Vec<(String, String, Stored)> },
    Line { gate: String, line: Vec<String>, given: Vec<(String, Stored)> },
    Call { id: u64, gate: String, line: Vec<String>, given: Vec<(String, Stored)>, chain: Vec<u32> },
    Ret { id: u64, line: Vec<String>, err: Option<String> },
    Join,
    Joined { caps: Vec<(String, Stored)> },
    Stop,
    Err(String),
    /// From a zone running elsewhere: what a viewer reads.
    Report(J),
}

impl Msg {
    pub fn kind(&self) -> &'static str {
        match self {
            Msg::Spawn { .. } => "spawn",
            Msg::Line { .. } => "line",
            Msg::Call { .. } => "call",
            Msg::Ret { .. } => "ret",
            Msg::Join => "join",
            Msg::Joined { .. } => "joined",
            Msg::Stop => "stop",
            Msg::Err(_) => "err",
            Msg::Report(_) => "report",
        }
    }
    pub fn has_handles(&self) -> bool {
        let any = |v: &Vec<(String, Stored)>| v.iter().any(|x| x.1.handle.is_some());
        match self {
            Msg::Spawn { caps, .. } => caps.iter().any(|c| c.2.handle.is_some()),
            Msg::Line { given, .. } | Msg::Call { given, .. } => any(given),
            Msg::Joined { caps } => any(caps),
            _ => false,
        }
    }
}

fn put_stored_w(out: &mut Vec<u8>, name: &str, mode: &str, s: &Stored) {
    let mut inner = Vec::new();
    wire::put_str(&mut inner, name);
    wire::put_str(&mut inner, mode);
    let mut fields = Vec::new();
    if let Some(m) = &s.mem {
        wire::put_str(&mut fields, "mem");
        wire::put_str(&mut fields, m);
    }
    if let Some(n) = s.num {
        wire::put_str(&mut fields, "num");
        wire::put_str(&mut fields, &format!("{:?}", n));
    }
    if let Some(l) = &s.list {
        wire::put_str(&mut fields, "list");
        wire::put_list(&mut fields, l);
    }
    if let Some(t) = &s.script {
        wire::put_str(&mut fields, "script");
        wire::put_str(&mut fields, t);
    }
    wire::put(&mut inner, &fields);
    wire::put(out, &inner);
}

fn get_stored_w(r: &mut Reader) -> Result<(String, String, Stored), String> {
    let b = r.get()?;
    let mut x = Reader::new(b);
    let name = x.str()?;
    let mode = x.str()?;
    let f = x.get()?;
    let mut y = Reader::new(f);
    let mut s = Stored::default();
    while !y.done() {
        match y.str()?.as_str() {
            "mem" => s.mem = Some(y.str()?),
            "num" => s.num = y.str()?.parse().ok(),
            "list" => s.list = Some(y.list()?),
            "script" => s.script = Some(y.str()?),
            other => return Err(format!("wire: unknown field {}", other)),
        }
    }
    Ok((name, mode, s))
}

/// Netstrings in this order: kind, zone id, gate, sequence number, value count and values,
/// given count and given names; a call ends with its chain.
pub fn encode(zone: u32, seq: u64, m: &Msg) -> Vec<u8> {
    let mut o = Vec::new();
    wire::put_str(&mut o, m.kind());
    wire::put_str(&mut o, &zone.to_string());
    let (gate, line): (&str, &[String]) = match m {
        Msg::Line { gate, line, .. } | Msg::Call { gate, line, .. } => (gate, line),
        Msg::Ret { line, .. } => ("", line),
        _ => ("", &[]),
    };
    wire::put_str(&mut o, gate);
    wire::put_str(&mut o, &seq.to_string());
    wire::put_str(&mut o, &line.len().to_string());
    for v in line {
        wire::put_str(&mut o, v);
    }
    let given: Vec<(&str, &str, &Stored)> = match m {
        Msg::Line { given, .. } | Msg::Call { given, .. } => given.iter().map(|(n, s)| (n.as_str(), "give", s)).collect(),
        Msg::Joined { caps } => caps.iter().map(|(n, s)| (n.as_str(), "mut", s)).collect(),
        Msg::Spawn { caps, .. } => caps.iter().map(|(n, md, s)| (n.as_str(), md.as_str(), s)).collect(),
        _ => Vec::new(),
    };
    wire::put_str(&mut o, &given.len().to_string());
    for (n, md, s) in given {
        put_stored_w(&mut o, n, md, s);
    }
    match m {
        Msg::Call { id, chain, .. } => {
            wire::put_str(&mut o, &id.to_string());
            wire::put_list(&mut o, &chain.iter().map(|x| x.to_string()).collect::<Vec<_>>());
        }
        Msg::Ret { id, err, .. } => {
            wire::put_str(&mut o, &id.to_string());
            wire::put_str(&mut o, err.as_deref().unwrap_or(""));
        }
        Msg::Spawn { defs, guards, .. } => {
            wire::put_str(&mut o, &defs.len().to_string());
            for (k, v) in defs {
                wire::put_str(&mut o, k);
                wire::put_str(&mut o, v);
            }
            wire::put_list(&mut o, guards);
        }
        Msg::Err(e) => wire::put_str(&mut o, e),
        Msg::Report(j) => wire::put_str(&mut o, &j.dump()),
        _ => {}
    }
    o
}

pub fn decode(b: &[u8]) -> Result<(u32, u64, Msg), String> {
    let mut r = Reader::new(b);
    let kind = r.str()?;
    let zone: u32 = r.str()?.parse().map_err(|_| "wire: bad zone")?;
    let gate = r.str()?;
    let seq = r.num()?;
    let n = r.num()? as usize;
    let mut line = Vec::with_capacity(n.min(1 << 16));
    for _ in 0..n {
        line.push(r.str()?);
    }
    let g = r.num()? as usize;
    let mut given = Vec::new();
    for _ in 0..g {
        given.push(get_stored_w(&mut r)?);
    }
    let m = match kind.as_str() {
        "line" => Msg::Line { gate, line, given: given.into_iter().map(|(n, _, s)| (n, s)).collect() },
        "call" => {
            let id = r.num()?;
            let chain = r.list()?.iter().filter_map(|x| x.parse().ok()).collect();
            Msg::Call { id, gate, line, given: given.into_iter().map(|(n, _, s)| (n, s)).collect(), chain }
        }
        "ret" => {
            let id = r.num()?;
            let e = r.str()?;
            Msg::Ret { id, line, err: if e.is_empty() { None } else { Some(e) } }
        }
        "spawn" => {
            let nd = r.num()? as usize;
            let mut defs = Vec::new();
            for _ in 0..nd {
                let k = r.str()?;
                let v = r.str()?;
                defs.push((k, v));
            }
            let guards = r.list()?;
            Msg::Spawn { defs, guards, caps: given }
        }
        "join" => Msg::Join,
        "joined" => Msg::Joined { caps: given.into_iter().map(|(n, _, s)| (n, s)).collect() },
        "stop" => Msg::Stop,
        "err" => Msg::Err(r.str()?),
        "report" => Msg::Report(J::parse(&r.str()?)?),
        k => return Err(format!("wire: unknown kind {}", k)),
    };
    Ok((zone, seq, m))
}

/// Whether a child may take a captured name in `mode` from a parent that holds it as `held`.
pub fn reborrow_ok(held: Option<&str>, mode: &str) -> bool {
    match held {
        Some("ref") => matches!(mode, "copy" | "ref"),
        Some("mut") => matches!(mode, "copy" | "ref" | "mut"),
        _ => true,
    }
}
