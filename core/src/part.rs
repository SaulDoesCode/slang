// Partitions: zones that run on other threads or other machines. A partition is a session
// of its own that owns one zone and everything nested in it. The coordinator owns main and
// every other zone, decides when the program has ended, and joins the partitions' roots.
use crate::json::J;
use crate::rt::*;
use crate::session::*;
use crate::wire::{self, Reader};
use crate::zone::*;
use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Write};
use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What travels between partitions.
pub enum Pm {
    /// A zone message from one zone to another.
    Deliver { from: u32, to: u32, m: Msg },
    /// Start zone `t` with this spawn image; `gen` numbers its incarnations.
    Spawn { t: u32, gen: u64, m: Msg },
    /// Ask the owner of `t`'s parent to spawn `t`.
    SpawnReq { t: u32 },
    /// End zone `t` now; answered by `JoinDone`.
    JoinReq { t: u32 },
    /// A partition root ended: its `mut` captures come back.
    Joined { t: u32, gen: u64, caps: Vec<(String, Stored)> },
    JoinDone { t: u32 },
    /// A partition reports itself: sent when it settles and when it has new errors.
    Status(PStatus),
    Tick,
    Exit(i32),
    Halt,
    /// Not sent: a partition's thread or connection failed.
    Lost(String),
    /// Not sent: a coupled world connected; its frames arrive under this partition id.
    Attach(TcpStream),
}

#[derive(Clone, Default)]
pub struct PStatus {
    pub settled: bool,
    /// Nothing here can move until a message comes.
    pub stalled: bool,
    /// Messages taken from the coordinator before this report.
    pub recv: u64,
    pub gen: u64,
    pub phases: Vec<(u32, u8)>,
    pub errors: Vec<J>,
    pub trace: Vec<J>,
    /// What out spheres show: (zone, sphere, text).
    pub shown: Vec<(u32, u32, String)>,
    pub halted: bool,
}

pub fn phase_code(p: Phase) -> u8 {
    match p {
        Phase::Unspawned => 0,
        Phase::Main => 1,
        Phase::Pulse => 2,
        Phase::Idle => 3,
        Phase::Failed => 4,
        Phase::Exited => 5,
    }
}

pub fn phase_of(c: u8) -> Phase {
    match c {
        1 => Phase::Main,
        2 => Phase::Pulse,
        3 => Phase::Idle,
        4 => Phase::Failed,
        5 => Phase::Exited,
        _ => Phase::Unspawned,
    }
}

pub enum Link {
    Chan(Sender<(u16, Pm)>),
    Net(BufWriter<TcpStream>),
}

pub struct Parts {
    pub me: u16,
    /// Which partition owns each zone.
    pub owner: Vec<u16>,
    /// Each partition's root zone; the coordinator's is main.
    pub roots: Vec<usize>,
    pub links: Vec<Option<Link>>,
    pub rx: Receiver<(u16, Pm)>,
    pub sent: Vec<u64>,
    pub recv: Vec<u64>,
    pub status: Vec<PStatus>,
    /// Incarnation of each partition root, as the coordinator numbers them.
    pub gen: Vec<u64>,
    pub lost: Vec<bool>,
    /// Calls waiting on another partition: call id → partition.
    pub calls: HashMap<u64, u16>,
    /// A worker's last report: (settled, stalled, messages taken).
    pub last: Option<(bool, bool, u64)>,
    pub errors_sent: usize,
    pub trace_sent: usize,
    pub halt: bool,
    /// Partitions that are another world's exported gates.
    pub couple: Vec<bool>,
    /// Links from worlds that call this one's exported gates.
    pub caller: Vec<bool>,
    /// Calls taken from callers: (link, call id) → (their zone, their gate zone).
    pub calls_in: HashMap<(u16, u64), (u32, u32)>,
}

/// Where a partition runs: a thread of this process, or the host of a net zone.
#[derive(Clone, Debug, PartialEq)]
pub enum Place {
    Here,
    Thread,
    Host(String),
    /// An rpc zone coupled to the exported gates of another world.
    Couple(String),
}

/// The partitions of a world. A thread zone, an rpc zone at `core`, a net zone and an rpc
/// zone at a host each start a partition with everything nested in them, unless they are
/// already inside one. With `net` off, net zones run on threads of this process.
pub fn plan(spec: &Spec, net: bool) -> (Vec<u16>, Vec<usize>, Vec<Place>) {
    let n = spec.zones.len();
    let mut owner: Vec<Option<u16>> = vec![None; n];
    let mut roots = vec![0usize];
    let mut places = vec![Place::Here];
    fn own(z: usize, spec: &Spec, net: bool, owner: &mut Vec<Option<u16>>, roots: &mut Vec<usize>, places: &mut Vec<Place>, depth: usize) -> u16 {
        if let Some(o) = owner[z] {
            return o;
        }
        let zs = &spec.zones[z];
        let up = match zs.parent {
            Some(p) if p != z && depth < 64 => own(p, spec, net, owner, roots, places, depth + 1),
            _ => 0,
        };
        let o = if z == 0 || up != 0 {
            up
        } else {
            let at = zs.param("at");
            let host = zs.param("host");
            let place = match zs.kind.as_str() {
                "thread" => Some(Place::Thread),
                "rpc" if zs.param("image").as_deref() == Some("none") => match zs.param("peer") {
                    Some(h) if !h.is_empty() => Some(Place::Couple(h)),
                    _ => None,
                },
                "rpc" => match at.as_deref() {
                    None | Some("core") | Some("") => Some(Place::Thread),
                    Some(h) => Some(if net { Place::Host(h.to_string()) } else { Place::Thread }),
                },
                "net" => match host.as_deref() {
                    Some(h) if net && !h.is_empty() && h != "local" => Some(Place::Host(h.to_string())),
                    _ => Some(Place::Thread),
                },
                _ => None,
            };
            match place {
                Some(pl) => {
                    roots.push(z);
                    places.push(pl);
                    (roots.len() - 1) as u16
                }
                None => 0,
            }
        };
        owner[z] = Some(o);
        o
    }
    for z in 0..n {
        own(z, spec, net, &mut owner, &mut roots, &mut places, 0);
    }
    (owner.into_iter().map(|o| o.unwrap_or(0)).collect(), roots, places)
}

/// The hash a net connection is checked by: FNV-1a 64 of the built spec.
pub fn spec_hash(spec: &Spec) -> u64 {
    wire::fnv1a(spec.to_json().dump().as_bytes())
}

impl Parts {
    pub fn new(me: u16, owner: Vec<u16>, roots: Vec<usize>, rx: Receiver<(u16, Pm)>) -> Parts {
        let np = roots.len();
        let nz = owner.len();
        Parts {
            me,
            owner,
            roots,
            links: (0..np).map(|_| None).collect(),
            rx,
            sent: vec![0; np],
            recv: vec![0; np],
            status: vec![PStatus::default(); np],
            gen: vec![0; nz],
            lost: vec![false; np],
            calls: HashMap::new(),
            last: None,
            errors_sent: 0,
            trace_sent: 0,
            halt: false,
            couple: vec![false; np],
            caller: vec![false; np],
            calls_in: HashMap::new(),
        }
    }

    /// Sends to partition `w`; false when the link is gone.
    pub fn send(&mut self, w: u16, pm: Pm) -> bool {
        let me = self.me;

        let ok = match self.links.get_mut(w as usize) {
            Some(Some(Link::Chan(tx))) => tx.send((me, pm)).is_ok(),
            Some(Some(Link::Net(wr))) => {
                let b = encode_pm(&pm);
                wire::write_frame(wr, &b).is_ok() && wr.flush().is_ok()
            }
            _ => false,
        };
        if ok {
            self.sent[w as usize] += 1;
        }
        ok
    }
}

// encoding, for links that cross machines

fn put_caps(o: &mut Vec<u8>, caps: &[(String, Stored)]) {
    let m = Msg::Joined { caps: caps.iter().map(|(n, s)| (n.clone(), s.copy())).collect() };
    wire::put(o, &encode(0, 0, &m));
}

fn get_caps(r: &mut Reader) -> Result<Vec<(String, Stored)>, String> {
    match decode(r.get()?)?.2 {
        Msg::Joined { caps } => Ok(caps),
        _ => Err("wire: expected captures".into()),
    }
}

pub fn encode_pm(pm: &Pm) -> Vec<u8> {
    let mut o = Vec::new();
    match pm {
        Pm::Deliver { from, to, m } => {
            wire::put_str(&mut o, "deliver");
            wire::put_str(&mut o, &from.to_string());
            wire::put_str(&mut o, &to.to_string());
            wire::put(&mut o, &encode(*to, 0, m));
        }
        Pm::Spawn { t, gen, m } => {
            wire::put_str(&mut o, "spawn");
            wire::put_str(&mut o, &t.to_string());
            wire::put_str(&mut o, &gen.to_string());
            wire::put(&mut o, &encode(*t, 0, m));
        }
        Pm::SpawnReq { t } => {
            wire::put_str(&mut o, "spawnreq");
            wire::put_str(&mut o, &t.to_string());
        }
        Pm::JoinReq { t } => {
            wire::put_str(&mut o, "joinreq");
            wire::put_str(&mut o, &t.to_string());
        }
        Pm::Joined { t, gen, caps } => {
            wire::put_str(&mut o, "joined");
            wire::put_str(&mut o, &t.to_string());
            wire::put_str(&mut o, &gen.to_string());
            put_caps(&mut o, caps);
        }
        Pm::JoinDone { t } => {
            wire::put_str(&mut o, "joindone");
            wire::put_str(&mut o, &t.to_string());
        }
        Pm::Status(s) => {
            wire::put_str(&mut o, "status");
            let j = J::obj()
                .with("settled", s.settled.into())
                .with("stalled", s.stalled.into())
                .with("recv", J::Num(s.recv as f64))
                .with("gen", J::Num(s.gen as f64))
                .with("phases", J::Arr(s.phases.iter().map(|(z, p)| J::Arr(vec![J::Num(*z as f64), J::Num(*p as f64)])).collect()))
                .with("errors", J::Arr(s.errors.clone()))
                .with("trace", J::Arr(s.trace.clone()))
                .with("shown", J::Arr(s.shown.iter().map(|(z, i, t)| J::Arr(vec![J::Num(*z as f64), J::Num(*i as f64), t.clone().into()])).collect()))
                .with("halted", s.halted.into());
            wire::put_str(&mut o, &j.dump());
        }
        Pm::Tick => wire::put_str(&mut o, "tick"),
        Pm::Exit(c) => {
            wire::put_str(&mut o, "exit");
            wire::put_str(&mut o, &c.to_string());
        }
        Pm::Halt => wire::put_str(&mut o, "halt"),
        Pm::Lost(e) => {
            wire::put_str(&mut o, "lost");
            wire::put_str(&mut o, e);
        }
        Pm::Attach(_) => wire::put_str(&mut o, "attach"),
    }
    o
}

pub fn decode_pm(b: &[u8]) -> Result<Pm, String> {
    let mut r = Reader::new(b);
    let kind = r.str()?;
    let num = |r: &mut Reader| -> Result<u64, String> { r.num() };
    Ok(match kind.as_str() {
        "deliver" => {
            let from = num(&mut r)? as u32;
            let to = num(&mut r)? as u32;
            let m = decode(r.get()?)?.2;
            Pm::Deliver { from, to, m }
        }
        "spawn" => {
            let t = num(&mut r)? as u32;
            let gen = num(&mut r)?;
            let m = decode(r.get()?)?.2;
            Pm::Spawn { t, gen, m }
        }
        "spawnreq" => Pm::SpawnReq { t: num(&mut r)? as u32 },
        "joinreq" => Pm::JoinReq { t: num(&mut r)? as u32 },
        "joined" => {
            let t = num(&mut r)? as u32;
            let gen = num(&mut r)?;
            let caps = get_caps(&mut r)?;
            Pm::Joined { t, gen, caps }
        }
        "joindone" => Pm::JoinDone { t: num(&mut r)? as u32 },
        "status" => {
            let j = J::parse(&r.str()?)?;
            Pm::Status(PStatus {
                settled: j.get("settled").b(false),
                stalled: j.get("stalled").b(false),
                recv: j.get("recv").f(0.0) as u64,
                gen: j.get("gen").f(0.0) as u64,
                phases: j.get("phases").arr().iter().map(|p| (p.arr()[0].u(0), p.arr()[1].u(0) as u8)).collect(),
                errors: j.get("errors").arr().to_vec(),
                trace: j.get("trace").arr().to_vec(),
                shown: j.get("shown").arr().iter().map(|x| (x.arr()[0].u(0), x.arr()[1].u(0), x.arr()[2].s(""))).collect(),
                halted: j.get("halted").b(false),
            })
        }
        "tick" => Pm::Tick,
        "exit" => Pm::Exit(r.str()?.parse().unwrap_or(1)),
        "halt" => Pm::Halt,
        "lost" => Pm::Lost(r.str()?),
        k => return Err(format!("wire: unknown partition message {}", k)),
    })
}

/// Reads frames from a connection into the partition's inbox until it closes.
pub fn read_link(stream: TcpStream, from: u16, tx: Sender<(u16, Pm)>) {
    std::thread::spawn(move || {
        let mut r = BufReader::new(stream);
        loop {
            match wire::read_frame(&mut r) {
                Ok(b) => match decode_pm(&b) {
                    Ok(pm) => {
                        if tx.send((from, pm)).is_err() {
                            return;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send((from, Pm::Lost(e)));
                        return;
                    }
                },
                Err(e) => {
                    let _ = tx.send((from, Pm::Lost(format!("connection: {}", e))));
                    return;
                }
            }
        }
    });
}

/// Reads one handshake line.
pub fn read_line(s: &mut TcpStream) -> std::io::Result<String> {
    use std::io::Read;
    let mut line = Vec::new();
    let mut b = [0u8; 1];
    loop {
        if s.read(&mut b)? == 0 {
            break;
        }
        if b[0] == b'\n' {
            break;
        }
        line.push(b[0]);
        if line.len() > 512 {
            break;
        }
    }
    Ok(String::from_utf8_lossy(&line).into_owned())
}

/// The first word and version of a net handshake.
pub const PROTO: &str = "s3 0.2";

impl Session {
    /// A session whose thread and net zones run as partitions. Lockstep keeps one thread.
    pub fn threaded(spec: Spec, prog: &'static Program, std: bool, argv: Vec<String>, net: bool) -> Session {
        Session::threaded_with(spec, prog, std, argv, net, false).0
    }

    /// With `host`, the session has partition links even with no partitions, so worlds that
    /// couple to its exported gates can attach, and the sender they attach through.
    pub fn threaded_with(spec: Spec, prog: &'static Program, std: bool, argv: Vec<String>, net: bool, host: bool) -> (Session, Option<Sender<(u16, Pm)>>) {
        let mut s = Session::bare(spec, prog, std, argv);
        let mut up = None;
        if !s.lockstep || host {
            let (owner, roots, places) = if s.lockstep { (vec![0; s.spec.zones.len()], vec![0], vec![Place::Here]) } else { plan(&s.spec, net) };
            if roots.len() > 1 || host {
                let (tx, rx) = channel();
                if host {
                    up = Some(tx.clone());
                }
                let mut parts = Parts::new(0, owner.clone(), roots.clone(), rx);
                for w in 1..roots.len() {
                    if matches!(places[w], Place::Couple(_)) {
                        parts.couple[w] = true;
                    }
                    let link = match &places[w] {
                        Place::Host(h) => s.connect(h, w as u16, roots[w], &tx),
                        Place::Couple(h) => s.couple_to(h, w as u16, &tx),
                        _ => s.start_thread(w as u16, owner.clone(), roots.clone(), tx.clone()),
                    };
                    match link {
                        Ok(l) => parts.links[w] = Some(l),
                        Err(e) => {
                            parts.lost[w] = true;
                            let msg = format!("zone lost: {}: {}", s.spec.zones[roots[w]].name, e);
                            s.errors.push(J::obj().with("zone", s.spec.zones[roots[w]].name.clone().into()).with("msg", msg.into()));
                        }
                    }
                }
                for z in 0..s.zones.len() {
                    if owner[z] != 0 && parts.lost[owner[z] as usize] {
                        s.zones[z].phase = Phase::Failed;
                    }
                }
                s.parts = Some(Box::new(parts));
            }
        }
        s.start_main(0);
        (s, up)
    }

    fn start_thread(&self, w: u16, owner: Vec<u16>, roots: Vec<usize>, up: Sender<(u16, Pm)>) -> Result<Link, String> {
        let (tx, rx) = channel();
        let spec = self.spec.clone();
        let prog = self.prog;
        let out = self.out.clone();
        let std = self.std;
        let argv = self.argv.clone();
        std::thread::Builder::new()
            .name(format!("zone {}", spec.zones[roots[w as usize]].name))
            .stack_size(MAIN_STACK)
            .spawn(move || {
                let up2 = up.clone();
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    let mut parts = Parts::new(w, owner, roots, rx);
                    parts.links[0] = Some(Link::Chan(up));
                    let mut ws = Session::worker(spec, prog, out, std, argv, parts);
                    ws.worker_loop();
                }));
                if r.is_err() {
                    let _ = up2.send((w, Pm::Lost("thread panicked".into())));
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Link::Chan(tx))
    }

    /// Connects to `s3 serve` on a net zone's host and hands it the partition.
    fn connect(&self, host: &str, w: u16, root: usize, up: &Sender<(u16, Pm)>) -> Result<Link, String> {
        let mut s = TcpStream::connect(host).map_err(|e| format!("{}: {}", host, e))?;
        let _ = s.set_nodelay(true);
        let hello = format!("{} {:016x} {} {}\n", PROTO, spec_hash(&self.spec), w, root);
        s.write_all(hello.as_bytes()).map_err(|e| e.to_string())?;
        let answer = read_line(&mut s).map_err(|e| e.to_string())?;
        if answer != "ok" {
            return Err(if answer.is_empty() { "refused".into() } else { answer });
        }
        read_link(s.try_clone().map_err(|e| e.to_string())?, w, up.clone());
        Ok(Link::Net(BufWriter::new(s)))
    }

    /// Connects an rpc zone to another world's exported gates.
    fn couple_to(&self, host: &str, w: u16, up: &Sender<(u16, Pm)>) -> Result<Link, String> {
        let mut s = TcpStream::connect(host).map_err(|e| format!("{}: {}", host, e))?;
        let _ = s.set_nodelay(true);
        s.write_all(format!("{} couple\n", PROTO).as_bytes()).map_err(|e| e.to_string())?;
        let answer = read_line(&mut s).map_err(|e| e.to_string())?;
        if answer != "ok" {
            return Err(if answer.is_empty() { "refused".into() } else { answer });
        }
        read_link(s.try_clone().map_err(|e| e.to_string())?, w, up.clone());
        Ok(Link::Net(BufWriter::new(s)))
    }

    /// The session of a partition: it owns its root's subtree; other zones are stand-ins.
    pub fn worker(spec: Spec, prog: &'static Program, out: Arc<Mutex<Vec<u8>>>, std: bool, argv: Vec<String>, parts: Parts) -> Session {
        let mut s = Session::bare_with(spec, prog, std, argv, out);
        for z in 0..s.zones.len() {
            if parts.owner[z] != parts.me {
                s.zones[z].phase = Phase::Idle;
            }
        }
        s.parts = Some(Box::new(parts));
        s
    }

    pub fn local(&self, z: usize) -> bool {
        match &self.parts {
            Some(p) => p.owner.get(z).map_or(z < self.zones.len(), |&o| o == p.me),
            None => true,
        }
    }

    pub fn is_worker(&self) -> bool {
        self.parts.as_ref().map_or(false, |p| p.me != 0)
    }

    /// The partition a zone runs on. A caller's link stands in as zone `zones + link`.
    pub(crate) fn owner_of(&self, z: usize) -> u16 {
        match &self.parts {
            Some(p) if z < p.owner.len() => p.owner[z],
            Some(_) => (z - self.zones.len()) as u16,
            None => 0,
        }
    }

    pub(crate) fn psend(&mut self, w: u16, pm: Pm) -> bool {
        let call = match &pm {
            Pm::Deliver { m: Msg::Call { id, .. }, .. } => Some(*id),
            _ => None,
        };
        let ok = match self.parts.as_mut() {
            Some(p) => {
                if p.lost[w as usize] {
                    false
                } else {
                    let ok = p.send(w, pm);
                    if !ok && p.me == 0 {
                        p.lost[w as usize] = true;
                    }
                    if ok && p.me == 0 {
                        if let Some(id) = call {
                            p.calls.insert(id, w);
                        }
                    }
                    ok
                }
            }
            None => false,
        };
        if !ok && !self.is_worker() {
            self.lose(w, "connection closed".into());
        }
        ok
    }

    /// A partition is settled: its last report says so and it had taken everything sent.
    pub(crate) fn remote_settled(&self, z: usize) -> bool {
        match &self.parts {
            Some(p) if p.me == 0 => {
                let w = p.owner[z] as usize;
                p.lost[w] || (p.status[w].settled && p.status[w].recv == p.sent[w])
            }
            _ => false,
        }
    }

    /// Takes waiting partition messages; with `wait`, blocks up to that long for the first.
    pub(crate) fn pump(&mut self, wait: Option<Duration>) -> bool {
        let mut any = false;
        let mut first = wait;
        loop {
            let got = {
                let Some(p) = self.parts.as_ref() else { return any };
                match first.take() {
                    Some(d) => match p.rx.recv_timeout(d) {
                        Ok(x) => Some(x),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => {
                            if p.me != 0 {
                                self.parts.as_mut().unwrap().halt = true;
                            }
                            None
                        }
                    },
                    None => match p.rx.try_recv() {
                        Ok(x) => Some(x),
                        Err(TryRecvError::Empty) => None,
                        Err(TryRecvError::Disconnected) => {
                            if p.me != 0 {
                                self.parts.as_mut().unwrap().halt = true;
                            }
                            None
                        }
                    },
                }
            };
            match got {
                Some((w, pm)) => {
                    self.on_pm(w, pm);
                    any = true;
                }
                None => return any,
            }
        }
    }

    pub(crate) fn on_pm(&mut self, w: u16, pm: Pm) {
        let worker = self.is_worker();

        if let Pm::Attach(c) = pm {
            self.attach(w, c);
            return;
        }
        if !matches!(pm, Pm::Status(_) | Pm::Lost(_)) {
            if let Some(p) = self.parts.as_mut() {
                if (w as usize) < p.recv.len() {
                    p.recv[w as usize] += 1;
                }
            }
        }
        if self.parts.as_ref().map_or(false, |p| p.caller.get(w as usize) == Some(&true)) {
            return self.from_caller(w, pm);
        }
        match pm {
            Pm::Deliver { from, to, m } => {
                if let (false, Msg::Ret { id, .. }) = (worker, &m) {
                    self.parts.as_mut().unwrap().calls.remove(id);
                }
                self.deliver(from as usize, to as usize, m)
            }
            Pm::Spawn { t, gen, m } => {
                let t = t as usize;
                if t < self.zones.len() && self.local(t) {
                    self.apply_spawn(t, m);
                    self.zones[t].gen = gen;
                }
            }
            Pm::SpawnReq { t } => {
                let t = t as usize;
                if t < self.zones.len() {
                    if let Err(e) = self.spawn(t) {
                        self.error(t, e);
                    }
                }
            }
            Pm::JoinReq { t } => {
                let t = t as usize;
                if t < self.zones.len() && self.local(t) {
                    self.join(t);
                }
                self.psend(w, Pm::JoinDone { t: t as u32 });
            }
            Pm::Joined { t, gen, caps } => self.joined(t as usize, gen, caps),
            Pm::JoinDone { .. } => {}
            Pm::Status(st) => {
                let gen_ok = {
                    let p = self.parts.as_ref().unwrap();
                    st.gen == p.gen[p.roots[w as usize]]
                };
                if gen_ok {
                    for (z, ph) in &st.phases {
                        let z = *z as usize;
                        if z < self.zones.len() && !self.local(z) {
                            self.zones[z].phase = phase_of(*ph);
                        }
                    }
                }
                self.errors.extend(st.errors.iter().cloned());
                self.trace.extend(st.trace.iter().cloned());
                for (z, id, t) in &st.shown {
                    if let Some(g) = self.zones.get_mut(*z as usize).and_then(|zr| zr.rt.g.as_mut()) {
                        if let Some(i) = g.idx(*id) {
                            g.spheres[i].shown = Some(t.clone());
                        }
                    }
                }
                if st.halted {
                    self.halted = true;
                }
                let p = self.parts.as_mut().unwrap();
                let mut st = st;
                st.errors.clear();
                st.trace.clear();
                st.shown.clear();
                p.status[w as usize] = st;
            }
            Pm::Tick => self.tick(),
            Pm::Exit(c) => {
                if !worker {
                    self.exit = Some(c);
                    for z in self.zones.iter_mut() {
                        z.phase = Phase::Exited;
                    }
                    self.halt_all();
                }
            }
            Pm::Halt => {
                if let Some(p) = self.parts.as_mut() {
                    p.halt = true;
                }
            }
            Pm::Lost(e) => {
                if !worker {
                    self.lose(w, e)
                } else if let Some(p) = self.parts.as_mut() {
                    p.halt = true;
                }
            }
            Pm::Attach(_) => {}
        }
    }

    /// A world connected to call this one's exported gates.
    fn attach(&mut self, w: u16, c: TcpStream) {
        let Some(p) = self.parts.as_mut() else { return };
        let i = w as usize;
        while p.links.len() <= i {
            p.links.push(None);
            p.roots.push(usize::MAX);
            p.sent.push(0);
            p.recv.push(0);
            p.status.push(PStatus::default());
            p.lost.push(false);
            p.couple.push(false);
            p.caller.push(false);
        }
        p.links[i] = Some(Link::Net(BufWriter::new(c)));
        p.caller[i] = true;
    }

    /// A message from a coupled world: calls go to the exported gate of that name.
    fn from_caller(&mut self, w: u16, pm: Pm) {
        match pm {
            Pm::Deliver { from, to, m: Msg::Call { id, gate, line, .. } } => {
                let target = (0..self.zones.len()).find(|&z| {
                    let zs = &self.spec.zones[z];
                    zs.kind == "rpc" && zs.param("export").as_deref() == Some("true") && self.zones[z].rt.g.as_ref().map_or(false, |g| g.named(&gate).is_some())
                });
                match target {
                    Some(z) => {
                        self.parts.as_mut().unwrap().calls_in.insert((w, id), (from, to));
                        let me = self.zones.len() + w as usize;
                        self.deliver(me, z, Msg::Call { id, gate, line, given: Vec::new(), chain: Vec::new() });
                    }
                    None => {
                        let m = Msg::Ret { id, line: Vec::new(), err: Some(format!("no exported gate {}", gate)) };
                        self.parts.as_mut().unwrap().send(w, Pm::Deliver { from: to, to: from, m });
                        self.caller_status(w);
                    }
                }
            }
            Pm::JoinReq { t } => {
                self.parts.as_mut().unwrap().send(w, Pm::JoinDone { t });
                self.caller_status(w);
            }
            Pm::Lost(_) => {
                let p = self.parts.as_mut().unwrap();
                p.links[w as usize] = None;
                p.lost[w as usize] = true;
                p.calls_in.retain(|k, _| k.0 != w);
            }
            _ => {}
        }
    }

    /// A return for a coupled world, addressed to the zone that called.
    pub(crate) fn to_caller(&mut self, w: u16, m: Msg) {
        if let Msg::Ret { id, line, err } = m {
            let back = self.parts.as_mut().unwrap().calls_in.remove(&(w, id));
            if let Some((zone, gate_zone)) = back {
                self.parts.as_mut().unwrap().send(w, Pm::Deliver { from: gate_zone, to: zone, m: Msg::Ret { id, line, err } });
                self.caller_status(w);
            }
        }
    }

    /// A coupled world counts this one quiet once it has answered everything it was sent.
    fn caller_status(&mut self, w: u16) {
        let p = self.parts.as_mut().unwrap();
        let st = PStatus { settled: true, recv: p.recv[w as usize], ..Default::default() };
        let n = p.sent[w as usize];
        p.send(w, Pm::Status(st));
        p.sent[w as usize] = n;
    }

    /// A partition root's `mut` captures come back; `ref` captures are released.
    fn joined(&mut self, t: usize, gen: u64, caps: Vec<(String, Stored)>) {
        if t >= self.zones.len() || self.local(t) {
            return;
        }
        let cur = self.parts.as_ref().unwrap().gen[t];
        if gen != cur || self.zones[t].phase == Phase::Unspawned {
            return;
        }
        let p = self.spec.zones[t].parent.unwrap_or(0);
        let mut caps = caps;
        for (name, mode) in self.spec.zones[t].captures.clone() {
            let k = self.zones[p].rt.it.sym(&name);
            match mode.as_str() {
                "mut" => {
                    self.zones[p].rt.set_guard(k, G_NONE);
                    if let Some(i) = caps.iter().position(|c| c.0 == name) {
                        let (_, st) = caps.remove(i);
                        put_stored(&mut self.zones[p].rt, &name, st);
                    }
                }
                "ref" => self.zones[p].rt.set_guard(k, G_NONE),
                _ => {}
            }
        }
        self.unspawn_mirror(t);
        self.note(t, "joined", J::Bool(true));
    }

    /// Marks a zone running elsewhere, and what is nested in it, as not spawned.
    fn unspawn_mirror(&mut self, t: usize) {
        let w = self.owner_of(t);
        for z in 0..self.zones.len() {
            if z == t || (self.owner_of(z) == w && self.spec_inside(z, t)) {
                self.zones[z].phase = Phase::Unspawned;
            }
        }
    }

    fn spec_inside(&self, z: usize, t: usize) -> bool {
        let mut c = self.spec.zones[z].parent;
        let mut n = 0;
        while let Some(p) = c {
            if p == t {
                return true;
            }
            if n > 64 || Some(p) == self.spec.zones[p].parent {
                break;
            }
            n += 1;
            c = self.spec.zones[p].parent;
        }
        false
    }

    /// Spawns a partition root on its partition with an image taken here.
    pub(crate) fn spawn_remote(&mut self, t: usize, m: Msg) {
        let w = self.owner_of(t);
        if self.parts.as_ref().unwrap().couple[w as usize] {
            // a coupled world runs already: there is nothing to start
            self.zones[t].phase = Phase::Main;
            return;
        }
        let gen = {
            let p = self.parts.as_mut().unwrap();
            p.gen[t] += 1;
            p.gen[t]
        };
        self.zones[t].phase = Phase::Main;
        self.psend(w, Pm::Spawn { t: t as u32, gen, m });
    }

    /// Ends a zone that runs elsewhere and waits until its partition says it has.
    pub(crate) fn join_remote(&mut self, t: usize) {
        let w = self.owner_of(t);
        if self.is_worker() || self.parts.as_ref().unwrap().lost[w as usize] {
            return;
        }
        if !self.psend(w, Pm::JoinReq { t: t as u32 }) {
            return;
        }
        loop {
            let got = self.parts.as_ref().unwrap().rx.recv_timeout(Duration::from_secs(30));
            match got {
                Ok((from, Pm::JoinDone { t: tt })) if from == w && tt as usize == t => {
                    self.parts.as_mut().unwrap().recv[from as usize] += 1;
                    break;
                }
                Ok((from, pm)) => {
                    self.on_pm(from, pm);
                    if self.parts.as_ref().unwrap().lost[w as usize] {
                        break;
                    }
                }
                Err(_) => {
                    self.lose(w, "no answer to join".into());
                    break;
                }
            }
        }
        if self.zones[t].phase != Phase::Unspawned {
            self.unspawn_mirror(t);
        }
    }

    /// A partition is lost: its `mut` captures become lost names, `ref` ones are released,
    /// and calls waiting on it fail.
    pub(crate) fn lose(&mut self, w: u16, why: String) {
        let Some(p) = self.parts.as_mut() else { return };
        if p.me != 0 || w == 0 || p.caller.get(w as usize) == Some(&true) {
            return;
        }
        let first = !p.lost[w as usize];
        p.lost[w as usize] = true;
        p.links[w as usize] = None;
        let root = p.roots[w as usize];
        let calls: Vec<u64> = p.calls.iter().filter(|(_, &x)| x == w).map(|(&id, _)| id).collect();
        for id in &calls {
            p.calls.remove(id);
        }
        if !first {
            return;
        }
        let name = self.spec.zones[root].name.clone();
        if self.zones[root].phase != Phase::Unspawned {
            let par = self.spec.zones[root].parent.unwrap_or(0);
            for (n, mode) in self.spec.zones[root].captures.clone() {
                let k = self.zones[par].rt.it.sym(&n);
                match mode.as_str() {
                    "mut" => self.zones[par].rt.set_guard(k, G_LOST),
                    "ref" => self.zones[par].rt.set_guard(k, G_NONE),
                    _ => {}
                }
            }
        }
        self.errors.push(J::obj().with("zone", name.clone().into()).with("msg", format!("zone lost: {}: {}", name, why).into()));
        for z in 0..self.zones.len() {
            if self.owner_of(z) == w {
                self.zones[z].phase = Phase::Failed;
            }
        }
        for z in 0..self.zones.len() {
            if !self.local(z) {
                continue;
            }
            let fail = match &self.zones[z].wait {
                Some(ZWait::Call(id, _)) | Some(ZWait::CallCode(id)) => calls.contains(id),
                _ => false,
            };
            if fail {
                let call_code = matches!(self.zones[z].wait, Some(ZWait::CallCode(_)));
                self.zones[z].wait = None;
                let e = format!("zone lost: {}", name);
                if call_code {
                    self.fail_running(z, e);
                } else {
                    self.error(z, e);
                }
            }
        }
    }

    pub(crate) fn halt_all(&mut self) {
        let n = self.parts.as_ref().map_or(0, |p| p.roots.len());
        for w in 1..n {
            let _ = self.parts.as_mut().map(|p| if !p.caller[w] && !p.couple[w] { p.send(w as u16, Pm::Halt); });
        }
    }

    /// Ticks every partition's clock with this one's.
    pub(crate) fn tick_parts(&mut self) {
        if self.is_worker() {
            return;
        }
        let n = self.parts.as_ref().map_or(0, |p| p.roots.len());
        for w in 1..n {
            if self.parts.as_ref().map_or(false, |p| !p.caller[w] && !p.couple[w]) {
                self.psend(w as u16, Pm::Tick);
            }
        }
    }

    /// Whether every partition has taken everything sent to it and can do nothing more.
    pub(crate) fn parts_still(&self) -> bool {
        match &self.parts {
            Some(p) if p.me == 0 => (1..p.roots.len()).all(|w| p.lost[w] || p.caller[w] || ((p.status[w].settled || p.status[w].stalled) && p.status[w].recv == p.sent[w])),
            _ => true,
        }
    }

    fn local_settled(&self) -> bool {
        (0..self.zones.len()).filter(|&z| self.local(z)).all(|z| self.settled(z)) && self.outbox.is_empty()
    }

    /// A worker reports when it settles, and whenever it has new errors.
    fn report_up(&mut self, still: bool) {
        let settled = self.local_settled();
        if settled {
            for z in 0..self.zones.len() {
                if self.local(z) {
                    self.zones[z].rt.flush();
                }
            }
        }
        let p = self.parts.as_ref().unwrap();
        let recv = p.recv[0];
        let fresh_errors = self.errors.len() > p.errors_sent;
        let changed = (settled || still) && p.last != Some((settled, still, recv));
        if !(fresh_errors || changed) {
            return;
        }
        let root = p.roots[p.me as usize];
        let me = p.me;
        let phases: Vec<(u32, u8)> = (0..self.zones.len()).filter(|&z| self.local(z)).map(|z| (z as u32, phase_code(self.zones[z].phase))).collect();
        let mut shown = Vec::new();
        for z in 0..self.zones.len() {
            if !self.local(z) {
                continue;
            }
            if let Some(g) = self.zones[z].rt.g.as_ref() {
                for sp in &g.spheres {
                    if let Some(t) = &sp.shown {
                        shown.push((z as u32, sp.id, t.clone()));
                    }
                }
            }
        }
        let st = PStatus {
            settled,
            stalled: still,
            recv,
            gen: self.zones[root].gen,
            phases,
            errors: self.errors[p.errors_sent..].to_vec(),
            trace: self.trace[p.trace_sent.min(self.trace.len())..].to_vec(),
            shown,
            halted: self.halted,
        };
        let (n, nt) = (self.errors.len(), self.trace.len());
        let p = self.parts.as_mut().unwrap();
        p.errors_sent = n;
        p.trace_sent = nt;
        p.last = Some((settled, still, recv));
        let _ = me;
        // reports do not count as messages: the coordinator compares what it sent with `recv`
        let sent0 = p.sent[0];
        p.send(0, Pm::Status(st));
        p.sent[0] = sent0;
    }

    /// The loop of a partition: take messages, run the owned zones, report, wait.
    pub fn worker_loop(&mut self) {
        loop {
            self.pump(None);
            if self.parts.as_ref().unwrap().halt {
                self.flush();
                return;
            }
            let r = self.run(1 << 16, StepKind::All);
            if let Some(c) = self.exit {
                self.flush();
                self.parts.as_mut().unwrap().send(0, Pm::Exit(c));
                return;
            }
            self.report_up(r == "idle");
            if r == "idle" {
                self.pump(Some(Duration::from_millis(250)));
            }
        }
    }
}

/// Hosts partitions of one world for the net zones of programs that run it elsewhere.
/// Each connection opens with `s3 <version> <hash> <partition> <root>` and gets `ok`, or a
/// reason it was refused, before frames flow both ways.
pub fn serve(addr: &str, spec: Spec, prog: &'static Program, std: bool, argv: Vec<String>, log: bool) -> Result<(), String> {
    let l = std::net::TcpListener::bind(addr).map_err(|e| format!("{}: {}", addr, e))?;
    let hash = format!("{:016x}", spec_hash(&spec));
    let exported: Vec<String> = spec.zones.iter().filter(|z| z.kind == "rpc" && z.param("export").as_deref() == Some("true")).map(|z| z.name.clone()).collect();
    if log {
        eprintln!("s3: serving {} at {}", hash, l.local_addr().map(|a| a.to_string()).unwrap_or_default());
    }
    // a world with exported gates runs here, and coupled worlds call into it
    let host: Option<Sender<(u16, Pm)>> = if exported.is_empty() {
        None
    } else {
        if log {
            eprintln!("s3: exporting {}", exported.join(", "));
        }
        let (ttx, trx) = channel();
        let (spec, argv) = (spec.clone(), argv.clone());
        std::thread::Builder::new()
            .stack_size(MAIN_STACK)
            .spawn(move || {
                let (mut s, up) = Session::threaded_with(spec, prog, std, argv, true, true);
                let _ = ttx.send(up);
                s.serving = true;
                loop {
                    let r = s.run(1 << 16, StepKind::All);
                    if r == "exit" {
                        s.flush();
                        std::process::exit(s.exit.unwrap_or(0));
                    }
                    if r != "ran" {
                        s.flush();
                        s.pump(Some(Duration::from_millis(250)));
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        trx.recv().ok().flatten()
    };
    let next = Arc::new(std::sync::atomic::AtomicU16::new(plan(&spec, true).1.len() as u16));
    for c in l.incoming() {
        let Ok(mut c) = c else { continue };
        let _ = c.set_nodelay(true);
        let spec = spec.clone();
        let hash = hash.clone();
        let argv = argv.clone();
        let host = host.clone();
        let next = next.clone();
        let _ = std::thread::Builder::new().stack_size(MAIN_STACK).spawn(move || {
            let hello = match read_line(&mut c) {
                Ok(h) => h,
                Err(_) => return,
            };
            let f: Vec<&str> = hello.split(' ').collect();
            let (owner, roots, _) = plan(&spec, true);
            let refuse = |c: &mut TcpStream, why: &str| {
                let _ = c.write_all(format!("{}\n", why).as_bytes());
            };
            if f.len() == 3 && f[2] == "couple" && format!("{} {}", f[0], f[1]) == PROTO {
                let Some(tx) = host else { return refuse(&mut c, "refused: no exported gates") };
                if c.write_all(b"ok\n").is_err() {
                    return;
                }
                let w = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let (Ok(wr), Ok(rd)) = (c.try_clone(), c.try_clone()) else { return };
                if log {
                    eprintln!("s3: coupled from {}", c.peer_addr().map(|a| a.to_string()).unwrap_or_default());
                }
                let _ = tx.send((w, Pm::Attach(wr)));
                read_link(rd, w, tx);
                return;
            }
            if f.len() != 5 || format!("{} {}", f[0], f[1]) != PROTO {
                return refuse(&mut c, &format!("refused: expected {}", PROTO));
            }
            if f[2] != hash {
                return refuse(&mut c, "refused: a different world");
            }
            let w: u16 = f[3].parse().unwrap_or(0);
            let root: usize = f[4].parse().unwrap_or(usize::MAX);
            if w == 0 || roots.get(w as usize) != Some(&root) {
                return refuse(&mut c, "refused: no such zone");
            }
            if log {
                eprintln!("s3: zone {} from {}", spec.zones[root].name, c.peer_addr().map(|a| a.to_string()).unwrap_or_default());
            }
            if c.write_all(b"ok\n").is_err() {
                return;
            }
            let (tx, rx) = channel();
            let Ok(rd) = c.try_clone() else { return };
            read_link(rd, 0, tx);
            let mut parts = Parts::new(w, owner, roots, rx);
            parts.links[0] = Some(Link::Net(BufWriter::new(c)));
            let mut ws = Session::worker(spec, prog, Arc::new(Mutex::new(Vec::new())), std, argv, parts);
            ws.worker_loop();
        });
    }
    Ok(())
}

/// The command line of a world program: `[--ticks n] [--lockstep] [--one-thread] [--net-here]
/// [--serve addr] [args...]`. Returns the exit code.
pub fn world_main(mut spec: Spec, prog: &'static Program, args: Vec<String>, me: &str) -> i32 {
    let mut ticks = 0u64;
    let mut threads = true;
    let mut net = true;
    let mut serve_at: Option<String> = None;
    let mut argv = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--ticks" => {
                ticks = args.get(i + 1).and_then(|x| x.parse().ok()).unwrap_or(0);
                i += 2;
                continue;
            }
            "--lockstep" => {
                if !matches!(spec.params, J::Obj(_)) {
                    spec.params = J::obj();
                }
                spec.params.set("lockstep", J::Bool(true));
            }
            "--one-thread" => threads = false,
            "--net-here" => net = false,
            "--serve" => {
                serve_at = args.get(i + 1).cloned();
                i += 2;
                continue;
            }
            _ => argv.push(args[i].clone()),
        }
        i += 1;
    }
    if let Some(addr) = serve_at {
        return match serve(&addr, spec, prog, true, argv, true) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("{}: {}", me, e);
                2
            }
        };
    }
    let mut s = if threads { Session::threaded(spec, prog, true, argv, net) } else { Session::with_out(spec, prog, true, argv) };
    s.run_to_end(u64::MAX);
    for _ in 0..ticks {
        if s.exit.is_some() {
            break;
        }
        s.tick();
        s.run_to_end(u64::MAX);
    }
    s.flush();
    for e in &s.errors {
        eprintln!("\n{}: {}", me, e.get("msg").s(""));
    }
    match s.exit {
        Some(c) => c,
        None => {
            if s.errors.is_empty() {
                0
            } else {
                1
            }
        }
    }
}
