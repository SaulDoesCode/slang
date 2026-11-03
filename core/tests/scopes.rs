// 0.3 scopes: dialects resolve inside their region only; the logic dialect runs on ticks;
// ports convert at the edge.
use s3_core::build::build;
use s3_core::json::J;
use s3_core::ops::apply_op;
use s3_core::rt::Program;
use s3_core::session::Session;
use s3_core::world::*;

static P: Program = Program::with_lib(s3_core::LIB);

fn op(w: &mut World, j: &str) -> J {
    let e = J::parse(j).unwrap_or_else(|e| panic!("{}: {}", j, e));
    match apply_op(w, &e) {
        Ok((_, r)) => r,
        Err(e) => panic!("{}: {}", j, e),
    }
}

fn id(w: &World, n: &str) -> u32 {
    w.sphere_named(n).unwrap().id
}

fn wire(w: &mut World, a: &str, b: &str) {
    let (x, y) = (id(w, a), id(w, b));
    if w.link_between(x, y).is_none() {
        op(w, &format!(r#"{{"op":"link","from":{},"to":{}}}"#, x, y));
    }
}

fn shown(s: &mut Session, name: &str) -> Option<String> {
    let r = s.report(false);
    for z in r.get("zones").arr() {
        for sp in z.get("graph").get("spheres").arr() {
            if sp.get("name").s("") == name {
                return sp.get("shown").str().map(|x| x.to_string());
            }
        }
    }
    None
}

fn errors(s: &Session) -> Vec<String> {
    s.errors.iter().map(|e| e.get("msg").s("")).collect()
}

fn logic_world() -> World {
    let mut w = World::starter("logic");
    op(&mut w, r#"{"op":"zone","name":"gates","kind":"scope","min":[20,-4,0],"max":[40,30,20],"params":{"dialect":"logic"}}"#);
    w
}

fn device(w: &mut World, name: &str, kind: &str, cell: [i32; 3], code: &str, params: &str) {
    op(w, &format!(r#"{{"op":"sphere","kind":"{}","name":"{}","cell":[{},{},{}],"aim":[0,1,0],"reach":0.01,"code":{},"params":{}}}"#, kind, name, cell[0], cell[1], cell[2], J::from(code).dump(), params));
}

fn session(w: &World) -> Session {
    let mut s = Session::new(build(w).unwrap_or_else(|p| panic!("{:?}", p)), &P);
    s.run_to_end(u64::MAX);
    s
}

#[test]
fn a_gate_is_one_tick_of_delay() {
    let mut w = logic_world();
    device(&mut w, "a", "src", [22, 1, 2], "", r#"{"on":1,"run":0}"#);
    device(&mut w, "b", "src", [22, 1, 4], "", r#"{"on":1,"run":0}"#);
    device(&mut w, "g", "fn", [25, 1, 3], "and", "{}");
    device(&mut w, "lamp", "out", [28, 1, 3], "", "{}");
    wire(&mut w, "a", "g");
    wire(&mut w, "b", "g");
    wire(&mut w, "g", "lamp");
    let mut s = session(&w);
    let mut seen = Vec::new();
    for _ in 0..4 {
        s.tick();
        s.run_to_end(u64::MAX);
        seen.push(shown(&mut s, "lamp").unwrap_or_default());
    }
    assert!(errors(&s).is_empty(), "{:?}", errors(&s));
    // tick 1: the switches; tick 2: the and gate; tick 3: the lamp
    assert_eq!(seen, vec!["0", "0", "1", "1"]);
}

#[test]
fn an_sr_latch_holds() {
    let mut w = logic_world();
    device(&mut w, "set", "src", [22, 1, 2], "", r#"{"on":1,"run":0}"#);
    device(&mut w, "reset", "src", [22, 1, 6], "", r#"{"on":0,"run":0}"#);
    device(&mut w, "q", "fn", [26, 1, 2], "nor", "{}");
    device(&mut w, "qbar", "fn", [26, 1, 6], "nor", "{}");
    // q = nor(reset, qbar), qbar = nor(set, q)
    wire(&mut w, "reset", "q");
    wire(&mut w, "qbar", "q");
    wire(&mut w, "set", "qbar");
    wire(&mut w, "q", "qbar");
    let mut s = session(&w);
    for _ in 0..6 {
        s.tick();
    }
    assert_eq!(shown(&mut s, "q").as_deref(), Some("1"), "{:?}", errors(&s));
    // set goes low: the latch keeps q
    let set = w.sphere_named("set").unwrap().clone();
    let mut off = set.clone();
    off.params = vec![("on".into(), 0.0), ("run".into(), 0.0)];
    op(&mut w, &Rec::Sphere(off).to_json().with("op", "put".into()).with("t", "sphere".into()).dump());
    s.patch(build(&w).unwrap(), s3_core::build::defs(&w)).unwrap();
    s.run_to_end(u64::MAX);
    for _ in 0..6 {
        s.tick();
    }
    assert_eq!(shown(&mut s, "q").as_deref(), Some("1"), "held");
    assert_eq!(shown(&mut s, "qbar").as_deref(), Some("0"));
}

#[test]
fn a_ring_of_three_inverters_oscillates_with_period_six() {
    let mut w = logic_world();
    device(&mut w, "n1", "fn", [22, 1, 2], "not", r#"{"init":1}"#);
    device(&mut w, "n2", "fn", [25, 1, 2], "not", "{}");
    device(&mut w, "n3", "fn", [28, 1, 2], "not", "{}");
    wire(&mut w, "n1", "n2");
    wire(&mut w, "n2", "n3");
    wire(&mut w, "n3", "n1");
    let mut s = session(&w);
    let mut seq = Vec::new();
    for _ in 0..18 {
        s.tick();
        seq.push(shown(&mut s, "n1").unwrap_or_default());
    }
    assert!(errors(&s).is_empty(), "{:?}", errors(&s));
    assert_eq!(&seq[0..6], &seq[6..12]);
    assert_eq!(&seq[6..12], &seq[12..18]);
    assert_ne!(&seq[0..3], &seq[3..6], "the period is six, not three: {:?}", seq);
    assert_ne!(&seq[0..2], &seq[2..4]);
}

#[test]
fn scope_words_stay_inside_and_siblings_differ() {
    // two scopes in plain s, each defining + its own way on its own plate
    let mut w = World::starter("scopes");
    op(&mut w, r#"{"op":"zone","name":"left","kind":"scope","min":[20,-4,0],"max":[29,30,9],"params":{"dialect":"s"}}"#);
    op(&mut w, r#"{"op":"zone","name":"right","kind":"scope","min":[30,-4,0],"max":[39,30,9],"params":{"dialect":"s"}}"#);
    for (x, def, tag) in [(20, "{2 \\( _R \\- _R~ \\)}", "L"), (30, "{2 \\( _R \\* _R~ \\)}", "R")] {
        let r = op(&mut w, &format!(r#"{{"op":"plate","space":0,"kind":"main","at":[{},0,0],"w":8,"d":4}}"#, x));
        let text = format!("{} '+' ><\n'{}' 7 3 + >", def, tag);
        op(&mut w, &J::obj().with("op", "settext".into()).with("plate", r.get("id").clone()).with("text", text.into()).dump());
    }
    // main uses + after both scopes ran
    let r = op(&mut w, r#"{"op":"plate","space":0,"kind":"main","at":[0,0,12],"w":8,"d":4}"#);
    op(&mut w, &J::obj().with("op", "settext".into()).with("plate", r.get("id").clone()).with("text", "7 3 + =~ >".into()).dump());
    let mut s = session(&w);
    s.end_zones();
    s.flush();
    let out = s.output();
    assert!(errors(&s).is_empty(), "{:?}", errors(&s));
    assert!(out.contains("L 4"), "{:?}", out);
    assert!(out.contains("R 21"), "{:?}", out);
    assert!(out.lines().any(|l| l.trim() == "10"), "the scopes' + stays inside them: {:?}", out);
}

#[test]
fn a_dialect_loads_in_its_scope_only() {
    let mut w = logic_world();
    let r = op(&mut w, r#"{"op":"plate","space":0,"kind":"main","at":[22,0,10],"w":8,"d":4}"#);
    op(&mut w, &J::obj().with("op", "settext".into()).with("plate", r.get("id").clone()).with("text", "'in' 1 0 or >".into()).dump());
    let r = op(&mut w, r#"{"op":"plate","space":0,"kind":"main","at":[0,0,12],"w":8,"d":4}"#);
    op(&mut w, &J::obj().with("op", "settext".into()).with("plate", r.get("id").clone()).with("text", "_logic.s~ >".into()).dump());
    let mut s = session(&w);
    s.end_zones();
    s.flush();
    let out = s.output();
    assert!(out.contains("in 1"), "{:?} {:?}", out, errors(&s));
    assert!(!out.contains(" 1\n") || out.matches(" 1").count() == 1, "{:?}", out);
}

#[test]
fn ports_convert_at_the_edge() {
    // a pulse of 5 crosses a port whose body makes it a bit; inside, a lamp shows it;
    // the lamp's inverse crosses back out through a second port to an out sphere
    let mut w = logic_world();
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"five","pos":[12,5.5,5.5],"aim":[1,0,0],"code":"5"}"#);
    op(&mut w, r#"{"op":"sphere","kind":"gate","name":"inport","pos":[20,5.5,5.5],"aim":[1,0,0],"code":"bit"}"#);
    device(&mut w, "lamp", "out", [24, 5, 5], "", "{}");
    device(&mut w, "inv", "fn", [24, 5, 9], "not", "{}");
    op(&mut w, r#"{"op":"sphere","kind":"gate","name":"outport","pos":[20,5.5,12.5],"aim":[-1,0,0],"reach":0.01}"#);
    op(&mut w, r#"{"op":"sphere","kind":"out","name":"outside","pos":[12,5.5,12.5]}"#);
    wire(&mut w, "inport", "inv");
    wire(&mut w, "inv", "outport");
    wire(&mut w, "outport", "outside");
    let mut s = session(&w);
    for _ in 0..4 {
        s.tick();
        s.run_to_end(u64::MAX);
    }
    assert!(errors(&s).is_empty(), "{:?}", errors(&s));
    assert_eq!(shown(&mut s, "lamp").as_deref(), Some("1"));
    assert_eq!(shown(&mut s, "outside").as_deref(), Some("0"), "{:?}", s.trace.iter().map(|t| t.dump()).collect::<Vec<_>>());
}
