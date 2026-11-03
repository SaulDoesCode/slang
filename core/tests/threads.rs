// Partitions: thread and rpc zones on their own threads, net zones over TCP, lost zones.
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
    let s = Session::threaded(spec, &P, false, Vec::new(), false);
    assert!(s.parts.is_some(), "the world has a partition");
    s
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
fn a_thread_zone_runs_on_its_own_thread() {
    let w = world("thread", "[]", "20 1", "", "+ =~");
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
fn move_capture_moves_to_the_thread() {
    let mut w = world("thread", r#"[["x","move"]]"#, "go", "", "// x~");
    column(&mut w, 0, 0, &["5", "x#"]);
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"later","pos":[-5,-5,-5],"aim":[0,-1,0],"code":"x~"}"#);
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"after","pos":[-8,-5,-5],"aim":[0,-1,0],"code":"7 x# x~ >"}"#);
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    assert_eq!(shown(&mut s, &w, "shown").as_deref(), Some("5"), "{:?}", errors(&s));
    let e = errors(&s);
    assert!(e.iter().any(|m| m == "moved: x"), "{:?}", e);
    assert!(s.output().contains(" 7"));
}

#[test]
fn ref_capture_is_released_when_the_thread_joins() {
    let mut w = world("thread", r#"[["x","ref"]]"#, "go", "", "// x~ 9 x#");
    column(&mut w, 0, 0, &["5", "x#"]);
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    let e = errors(&s);
    assert!(e.iter().any(|m| m == "borrowed: x"), "{:?}", e);
    s.zones[0].rt.run_text("3 x#");
    assert_eq!(s.peek("main", "x").1, Some(3.0));
}

#[test]
fn mut_capture_comes_back_from_the_thread() {
    let mut w = world("thread", r#"[["x","mut"]]"#, "go", "", "// x~ 1 x + x~");
    column(&mut w, 0, 0, &["5", "x#"]);
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    assert!(errors(&s).is_empty(), "{:?}", errors(&s));
    assert_eq!(s.peek("main", "x").1, Some(6.0), "the zone's value came back");
}

#[test]
fn give_crosses_to_the_thread() {
    let mut w = world("thread", "[]", "go", "secret give", "secret~");
    column(&mut w, 0, 0, &["42", "secret#"]);
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    assert_eq!(shown(&mut s, &w, "shown").as_deref(), Some("go 42"), "{:?}", errors(&s));
    assert_eq!(s.peek("main", "secret"), (None, None));
}

#[test]
fn a_handle_moves_to_a_thread() {
    let dir = std::env::temp_dir().join("s3-part-handle");
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join("h.txt");
    std::fs::write(&p, "first line\n").unwrap();
    let path = p.to_string_lossy().to_string();
    let mut w = world("thread", r#"[["fh","move"]]"#, "go", "", "// fh got _f got~");
    column(&mut w, 0, 0, &[&format!("'{}'", path), "fh", "'r'", "f"]);
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    assert_eq!(shown(&mut s, &w, "shown").as_deref(), Some("first line"), "{:?}", errors(&s));
}

#[test]
fn an_rpc_zone_at_core_answers_from_its_thread() {
    let mut w = world("rpc", "[]", "21", "", "2 * =~");
    let mut work = w.sphere_named("work").unwrap().clone();
    work.aim = [-1.0, 0.0, 0.0];
    work.strike = Strike::Sticky;
    apply_op(&mut w, &J::parse(&Rec::Sphere(work).to_json().with("op", "put".into()).with("t", "sphere".into()).dump()).unwrap()).unwrap();
    op(&mut w, r#"{"op":"sphere","kind":"out","name":"back","pos":[6,5,5]}"#);
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    assert_eq!(shown(&mut s, &w, "back").as_deref(), Some("42"), "{:?}", errors(&s));
}

#[test]
fn a_call_in_code_waits_for_the_thread() {
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
fn join_waits_for_the_thread() {
    let mut w = world("thread", r#"[["x","mut"]]"#, "go", "", "// x~ 10 x + x~");
    column(&mut w, 0, 0, &["1", "x#"]);
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"after","pos":[-8,-5,-5],"aim":[0,-1,0],"code":"z join x~ >"}"#);
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    assert!(s.output().contains(" 11"), "{:?} {:?}", s.output(), errors(&s));
}

#[test]
fn per_zone_traces_match_the_single_thread_run() {
    let per_zone = |s: &Session| {
        let mut t: Vec<String> = s.trace.iter().filter(|t| t.has("s")).map(|t| format!("{} {} {}", t.get("zone").s("main"), t.get("s").u(0), t.get("out").dump())).collect();
        t.sort();
        t
    };
    let w = world("thread", "[]", "1 2 3", "", "+ =~");
    let spec = build(&w).unwrap();
    let mut one = Session::new(spec, &P);
    one.run_to_end(u64::MAX);
    let mut s = session(&w);
    s.run_to_end(u64::MAX);
    assert_eq!(per_zone(&s), per_zone(&one));
}

#[test]
fn many_threads_work_at_once() {
    // four thread zones side by side; main spawns them and each source sums a long count
    let mut w = World::starter("t");
    let mut words = Vec::new();
    for i in 0..4 {
        let x0 = 10 + i * 12;
        op(&mut w, &format!(r#"{{"op":"zone","name":"z{}","kind":"thread","min":[{},0,0],"max":[{},9,9]}}"#, i, x0, x0 + 9));
        op(&mut w, &format!(r#"{{"op":"sphere","kind":"src","name":"w{}","pos":[{},5,5],"aim":[1,0,0],"code":"{{i~ acc~ acc + i: i~ 0 >> spin.?}} spin$ 100000 i# 0 acc# spin. acc~"}}"#, i, x0 + 2));
        op(&mut w, &format!(r#"{{"op":"sphere","kind":"out","name":"o{}","pos":[{},5,5]}}"#, i, x0 + 8));
        words.push(format!("z{}", i));
        words.push("spawn".to_string());
    }
    let refs: Vec<&str> = words.iter().map(|x| x.as_str()).collect();
    column(&mut w, 0, 0, &refs);
    let mut s = Session::threaded(build(&w).unwrap(), &P, false, Vec::new(), false);
    assert_eq!(s.parts.as_ref().unwrap().roots.len(), 5);
    s.run_to_end(u64::MAX);
    assert!(errors(&s).is_empty(), "{:?}", errors(&s));
    for i in 0..4 {
        assert_eq!(shown(&mut s, &w, &format!("o{}", i)).as_deref(), Some("5000050000"));
    }
}

#[test]
fn an_exit_in_a_thread_ends_the_program() {
    let w = world("thread", "[]", "go", "", "// 3 <--");
    let mut s = session(&w);
    let r = s.run_to_end(u64::MAX);
    assert_eq!(r, "exit");
    assert_eq!(s.exit, Some(3));
}

#[test]
fn an_unreachable_host_loses_the_zone() {
    let mut w = world("net", "[]", "go", "", "// 1");
    let mut zr = w.zones.values().find(|z| z.name == "z").unwrap().clone();
    zr.params = vec![("host".into(), "127.0.0.1:9".into())];
    apply_op(&mut w, &J::parse(&Rec::Zone(zr).to_json().with("op", "put".into()).with("t", "zone".into()).dump()).unwrap()).unwrap();
    let mut s = Session::threaded(build(&w).unwrap(), &P, false, Vec::new(), true);
    s.run_to_end(u64::MAX);
    let e = errors(&s);
    assert!(e.iter().any(|m| m.starts_with("zone lost: z")), "{:?}", e);
}

#[test]
fn a_dropped_connection_loses_the_mut_capture() {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap().to_string();
    let server = std::thread::spawn(move || {
        let (mut c, _) = l.accept().unwrap();
        let mut b = [0u8; 1];
        while c.read(&mut b).unwrap() == 1 && b[0] != b'\n' {}
        c.write_all(b"ok\n").unwrap();
        // take the spawn image, then go away
        let _ = s3_core::wire::read_frame(&mut c);
    });
    let mut w = world("net", r#"[["x","mut"]]"#, "go", "", "// x~");
    let mut zr = w.zones.values().find(|z| z.name == "z").unwrap().clone();
    zr.params = vec![("host".into(), addr)];
    apply_op(&mut w, &J::parse(&Rec::Zone(zr).to_json().with("op", "put".into()).with("t", "zone".into()).dump()).unwrap()).unwrap();
    column(&mut w, 0, 0, &["5", "x#"]);
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"after","pos":[-8,-5,-5],"aim":[0,-1,0],"code":"z join x~ >"}"#);
    let mut s = Session::threaded(build(&w).unwrap(), &P, false, Vec::new(), true);
    s.run_to_end(u64::MAX);
    server.join().unwrap();
    let e = errors(&s);
    assert!(e.iter().any(|m| m.starts_with("zone lost: z")), "{:?}", e);
    assert!(e.iter().any(|m| m == "zone lost: x"), "{:?}", e);
}
