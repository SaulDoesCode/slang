// Zone conformance: captures, gates, give, calls, joins, walls, schedules.
use s3_core::build::build;
use s3_core::json::J;
use s3_core::ops::apply_op;
use s3_core::rt::Program;
use s3_core::session::{Session, Spec};
use s3_core::world::*;

static P: Program = Program::with_lib(s3_core::LIB);

fn op(w: &mut World, j: &str) -> u32 {
    let e = J::parse(j).unwrap_or_else(|e| panic!("{}: {}", j, e));
    match apply_op(w, &e) {
        Ok((_, r)) => r.get("id").u(0),
        Err(e) => panic!("{}: {}", j, e),
    }
}

fn column(w: &mut World, x: i32, z: i32, words: &[&str]) {
    for (i, word) in words.iter().enumerate() {
        op(w, &format!(r#"{{"op":"place","space":0,"cell":[{},{},{}],"word":{}}}"#, x, i + 1, z, J::from(*word).dump()));
    }
}

/// main plate at the origin; zone `z` of `kind` spanning x 10…19; a gate on its west wall
/// at (10,5,5) aimed in; a source outside aimed at the gate; inside, `inner` code on a fn
/// aimed at an out sphere.
fn world(kind: &str, caps: &str, src: &str, gate: &str, inner: &str) -> World {
    let mut w = World::starter("t");
    op(&mut w, &format!(r#"{{"op":"zone","name":"z","kind":"{}","min":[10,0,0],"max":[19,9,9],"captures":{}}}"#, kind, caps));
    op(&mut w, &format!(r#"{{"op":"sphere","kind":"src","name":"go","pos":[2,8,5],"code":{}}}"#, J::from(src).dump()));
    op(&mut w, &format!(r#"{{"op":"sphere","kind":"gate","name":"g","pos":[10,5,5],"aim":[1,0,0],"code":{}}}"#, J::from(gate).dump()));
    op(&mut w, &format!(r#"{{"op":"sphere","kind":"fn","name":"work","pos":[15,5,5],"aim":[1,0,0],"code":{}}}"#, J::from(inner).dump()));
    op(&mut w, r#"{"op":"sphere","kind":"out","name":"shown","pos":[18,5,5]}"#);
    let go = w.sphere_named("go").unwrap().clone();
    let g = w.sphere_named("g").unwrap().clone();
    let mut go2 = go.clone();
    go2.aim = norm([g.pos[0] - go.pos[0], g.pos[1] - go.pos[1], g.pos[2] - go.pos[2]]);
    apply_op(&mut w, &J::parse(&Rec::Sphere(go2).to_json().with("op", "put".into()).with("t", "sphere".into()).dump()).unwrap()).unwrap();
    w
}

fn session(w: &World) -> Session {
    let spec = build(w).unwrap_or_else(|p| panic!("{:?}", p));
    Session::new(spec, &P)
}

fn shown(s: &mut Session, w: &World, name: &str) -> Option<String> {
    let id = w.sphere_named(name)?.id;
    let r = s.report(false);
    for z in r.get("zones").arr() {
        for sp in z.get("graph").get("spheres").arr() {
            if sp.get("id").u(0) == id {
                if let Some(t) = sp.get("shown").str() {
                    return Some(t.to_string());
                }
            }
        }
    }
    None
}

fn errors(s: &Session) -> Vec<String> {
    s.errors.iter().map(|e| e.get("msg").s("")).collect()
}

#[test]
fn strikes_cross_the_wall_only_through_the_gate() {
    let w = world("async", "[]", "1", "", "");
    let id = |n: &str| w.sphere_named(n).unwrap().id;
    let has = |a: &str, b: &str| w.link_between(id(a), id(b)).is_some();
    assert!(has("go", "g"), "outside source strikes the gate");
    assert!(has("g", "work"), "the gate strikes inward");
    assert!(has("work", "shown"));
    // a sphere outside aimed through the wall at an inside sphere strikes nothing
    let mut w2 = w.clone();
    op(&mut w2, r#"{"op":"sphere","kind":"src","name":"peek","pos":[2,2,2],"aim":[1,0,0]}"#);
    op(&mut w2, r#"{"op":"sphere","kind":"out","name":"far","pos":[15,2,2]}"#);
    let a = w2.sphere_named("peek").unwrap().id;
    assert!(w2.links.values().all(|l| l.from != a));
}

#[test]
fn a_line_crosses_a_gate_and_runs_inside() {
    let w = world("async", "[]", "20 1", "", "+ =~");
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    assert_eq!(shown(&mut s, &w, "shown").as_deref(), Some("21"), "{:?}", errors(&s));
}

#[test]
fn copy_capture_leaves_the_parent_alone() {
    let mut w = world("thread", r#"[["x","copy"]]"#, "go", "", "// x~ 1 x + x~");
    column(&mut w, 0, 0, &["5", "x#"]);
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    assert_eq!(shown(&mut s, &w, "shown").as_deref(), Some("6"), "{:?}", errors(&s));
    assert_eq!(s.peek("main", "x").1, Some(5.0));
}

#[test]
fn move_capture_moves_and_reading_after_is_an_error() {
    let mut w = world("async", r#"[["x","move"]]"#, "go", "", "// x~");
    column(&mut w, 0, 0, &["5", "x#"]);
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"later","pos":[-5,-5,-5],"aim":[0,-1,0],"code":"x~"}"#);
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"after","pos":[-8,-5,-5],"aim":[0,-1,0],"code":"7 x# x~ >"}"#);
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    assert_eq!(shown(&mut s, &w, "shown").as_deref(), Some("5"));
    let e = errors(&s);
    assert!(e.iter().any(|m| m == "moved: x"), "{:?}", e);
    assert!(s.output().contains(" 7"), "writing a moved name sets it fresh");
}

#[test]
fn ref_capture_freezes_both_sides_until_join() {
    let mut w = world("async", r#"[["x","ref"]]"#, "go", "", "// x~ 9 x#");
    column(&mut w, 0, 0, &["5", "x#"]);
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"later","pos":[-5,-5,-5],"aim":[0,-1,0],"code":"8 x#"}"#);
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    let e = errors(&s);
    assert!(e.iter().any(|m| m == "borrowed: x"), "{:?}", e);
    assert!(e.iter().any(|m| m == "lent: x"), "{:?}", e);
    // the program ended, the zone joined: the parent writes again
    s.zones[0].rt.run_text("3 x#");
    assert_eq!(s.peek("main", "x").1, Some(3.0));
}

#[test]
fn mut_capture_moves_back_at_join() {
    let mut w = world("async", r#"[["x","mut"]]"#, "go", "", "// x~ 1 x + x~");
    column(&mut w, 0, 0, &["5", "x#"]);
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"later","pos":[-5,-5,-5],"aim":[0,-1,0],"code":"x~"}"#);
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    assert!(errors(&s).iter().any(|m| m == "lent: x"), "{:?}", errors(&s));
    assert_eq!(s.peek("main", "x").1, Some(6.0), "the zone's value came back");
}

#[test]
fn reborrow_rules_refuse_a_mut_from_a_ref() {
    let mut w = world("async", r#"[["x","ref"]]"#, "go", "", "// x~");
    column(&mut w, 0, 0, &["5", "x#"]);
    op(&mut w, r#"{"op":"zone","name":"inner","kind":"async","min":[12,1,1],"max":[13,2,2],"captures":[["x","mut"]]}"#);
    op(&mut w, r#"{"op":"sphere","kind":"fn","name":"poke","pos":[16,8,8],"aim":[0,0,0.000001],"code":"inner spawn"}"#);
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    s.fire_named("poke");
    s.run_to_end(u64::MAX);
    assert!(errors(&s).iter().any(|m| m.contains("cannot take x as mut from a ref capture")), "{:?}", errors(&s));
}

#[test]
fn give_moves_a_name_with_the_line() {
    let mut w = world("async", "[]", "go", "secret give", "secret~");
    column(&mut w, 0, 0, &["42", "secret#"]);
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    assert_eq!(shown(&mut s, &w, "shown").as_deref(), Some("go 42"), "{:?}", errors(&s));
    assert_eq!(s.peek("main", "secret"), (None, None));
}

#[test]
fn handles_move_within_a_host_and_not_beyond() {
    let dir = std::env::temp_dir().join("s3-zone-handle");
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("h.txt");
    std::fs::write(&p, "first line\n").unwrap();
    let path = p.to_string_lossy().to_string();
    for (kind, ok) in [("thread", true), ("net", false)] {
        let mut w = world(kind, r#"[["fh","move"]]"#, "go", "", "// fh got _f got~");
        column(&mut w, 0, 0, &[&format!("'{}'", path), "fh", "'r'", "f"]);
        let mut s = session(&w);
        s.run_to_end(u64::MAX);
        if ok {
            assert_eq!(shown(&mut s, &w, "shown").as_deref(), Some("first line"), "{:?}", errors(&s));
        } else {
            assert!(errors(&s).iter().any(|m| m == "handle cannot leave host: fh"), "{:?}", errors(&s));
        }
    }
}

#[test]
fn an_rpc_zone_returns_what_reaches_its_gate() {
    let mut w = world("rpc", "[]", "21", "", "2 * =~");
    // inside, the worker aims back at the gate so its result reaches the gate again
    let mut work = w.sphere_named("work").unwrap().clone();
    work.aim = [-1.0, 0.0, 0.0];
    work.strike = Strike::Sticky;
    apply_op(&mut w, &J::parse(&Rec::Sphere(work).to_json().with("op", "put".into()).with("t", "sphere".into()).dump()).unwrap()).unwrap();
    op(&mut w, r#"{"op":"sphere","kind":"out","name":"back","pos":[6,5,5]}"#);
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    assert_eq!(shown(&mut s, &w, "back").as_deref(), Some("42"), "{:?} {:?}", errors(&s), s.trace.iter().map(|t| t.dump()).collect::<Vec<_>>());
}

#[test]
fn a_call_in_code_replaces_the_line_with_the_return() {
    let mut w = world("rpc", "[]", "1", "", "+ =~");
    let mut work = w.sphere_named("work").unwrap().clone();
    work.aim = [-1.0, 0.0, 0.0];
    apply_op(&mut w, &J::parse(&Rec::Sphere(work).to_json().with("op", "put".into()).with("t", "sphere".into()).dump()).unwrap()).unwrap();
    column(&mut w, 0, 0, &["21", "22", "g.", ">"]);
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    assert!(s.output().starts_with("\n 43"), "{:?} {:?}", s.output(), errors(&s));
}

#[test]
fn join_waits_for_quiet_and_releases() {
    let mut w = world("async", r#"[["x","mut"]]"#, "go", "", "// x~ 10 x + x~");
    column(&mut w, 0, 0, &["1", "x#"]);
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"after","pos":[-8,-5,-5],"aim":[0,-1,0],"code":"z join x~ >"}"#);
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    assert!(s.output().contains(" 11"), "{:?} {:?}", s.output(), errors(&s));
}

#[test]
fn schedules_and_kinds_give_the_same_trace() {
    let mut traces = Vec::new();
    for kind in ["async", "thread"] {
        for lock in [false, true] {
            let mut w = world(kind, "[]", "1 2 3", "", "+ =~");
            if lock {
                op(&mut w, r#"{"op":"meta","params":{"lockstep":true}}"#);
            }
            let mut s = session(&w);
            s.run_to_end(u64::MAX);
            let t: Vec<String> = s.trace.iter().filter(|t| t.has("s")).map(|t| format!("{} {} {}", t.get("zone").s("main"), t.get("s").u(0), t.get("out").dump())).collect();
            traces.push(t);
        }
    }
    for t in &traces[1..] {
        assert_eq!(t, &traces[0]);
    }
}

#[test]
fn wire_round_trips_every_message() {
    use s3_core::zone::*;
    let st = || Stored { mem: Some("a b".into()), num: Some(1.5), list: Some(vec!["x".into(), "y z".into()]), script: Some("1 2 +".into()), handle: None };
    let msgs = vec![
        Msg::Spawn { defs: vec![("w".into(), "{2 *}".into())], guards: vec!["_core.s".into()], caps: vec![("x".into(), "mut".into(), st())] },
        Msg::Line { gate: "g".into(), line: vec!["1".into(), "two words".into()], given: vec![("s".into(), st())] },
        Msg::Call { id: 7, gate: "g".into(), line: vec!["".into()], given: vec![], chain: vec![0, 3] },
        Msg::Ret { id: 7, line: vec!["42".into()], err: Some("oops".into()) },
        Msg::Join,
        Msg::Joined { caps: vec![("x".into(), st())] },
        Msg::Stop,
        Msg::Err("bad".into()),
        Msg::Report(J::obj().with("k", J::Num(1.0))),
    ];
    for m in &msgs {
        let b = encode(3, 9, m);
        let (z, seq, back) = decode(&b).unwrap();
        assert_eq!((z, seq), (3, 9));
        assert_eq!(encode(3, 9, &back), b, "{}", m.kind());
    }
}

#[test]
fn spec_json_round_trips() {
    let w = world("rpc", r#"[["x","copy"]]"#, "1", "", "2 * =~");
    let spec = build(&w).unwrap();
    let j = spec.to_json();
    let back = Spec::from_json(&J::parse(&j.dump()).unwrap());
    assert_eq!(back.to_json().dump(), j.dump());
}



#[test]
fn a_live_edit_takes_effect_at_the_next_event() {
    let mut w = world("async", "[]", "20 1", "", "+ =~");
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    assert_eq!(shown(&mut s, &w, "shown").as_deref(), Some("21"));
    let body = w.sphere_named("work").unwrap().body;
    op(&mut w, &format!(r#"{{"op":"settext","space":{},"text":"* =~"}}"#, body));
    s.patch(build(&w).unwrap(), s3_core::build::defs(&w)).unwrap();
    s.fire_named("go");
    s.run_to_end(u64::MAX);
    assert_eq!(shown(&mut s, &w, "shown").as_deref(), Some("20"), "{:?}", errors(&s));
    // a new zone is not a live edit
    op(&mut w, r#"{"op":"zone","name":"more","kind":"async","min":[30,0,0],"max":[39,9,9]}"#);
    assert_eq!(s.patch(build(&w).unwrap(), s3_core::build::defs(&w)), Err("the zones changed".to_string()));
}
