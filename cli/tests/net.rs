// A net zone over TCP: one process serves the world, another runs it.
use s3_core::json::J;
use s3_core::ops::apply_op;
use s3_core::world::*;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn s() -> &'static str {
    env!("CARGO_BIN_EXE_s3")
}

fn op(w: &mut World, j: &str) {
    let e = J::parse(j).unwrap();
    apply_op(w, &e).unwrap_or_else(|e| panic!("{}: {}", j, e));
}

fn column(w: &mut World, x: i32, z: i32, words: &[&str]) {
    for (i, word) in words.iter().enumerate() {
        op(w, &format!(r#"{{"op":"place","space":0,"cell":[{},{},{}],"word":{}}}"#, x, i + 1, z, J::from(*word).dump()));
    }
}

fn net_world(host: &str, inner: &str) -> World {
    let mut w = World::starter("t");
    op(&mut w, &format!(r#"{{"op":"zone","name":"far","kind":"net","min":[10,0,0],"max":[19,9,9],"captures":[["x","mut"]],"params":{{"host":"{}"}}}}"#, host));
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"go","pos":[2,8,5],"code":"go"}"#);
    op(&mut w, r#"{"op":"sphere","kind":"gate","name":"g","pos":[10,5,5],"aim":[1,0,0]}"#);
    op(&mut w, &format!(r#"{{"op":"sphere","kind":"fn","name":"work","pos":[15,5,5],"aim":[1,0,0],"code":{}}}"#, J::from(inner).dump()));
    op(&mut w, r#"{"op":"sphere","kind":"out","name":"shown","pos":[18,5,5]}"#);
    let go = w.sphere_named("go").unwrap().clone();
    let g = w.sphere_named("g").unwrap().clone();
    let mut go2 = go.clone();
    go2.aim = norm([g.pos[0] - go.pos[0], g.pos[1] - go.pos[1], g.pos[2] - go.pos[2]]);
    apply_op(&mut w, &J::parse(&Rec::Sphere(go2).to_json().with("op", "put".into()).with("t", "sphere".into()).dump()).unwrap()).unwrap();
    column(&mut w, 0, 0, &["1", "x#"]);
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"after","pos":[-8,-5,-5],"aim":[0,-1,0],"code":"far join x~ >"}"#);
    w
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

#[test]
fn a_net_zone_runs_in_the_serving_process() {
    let port = free_port();
    let host = format!("127.0.0.1:{}", port);
    let w = net_world(&host, "// x~ 10 x + 'far side' > x~");
    let dir = std::env::temp_dir().join(format!("s3-net-{}", port));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("w.json");
    std::fs::write(&path, w.to_json().pretty()).unwrap();
    let mut server = Command::new(s()).arg("serve").arg(&host).arg(&path).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let t0 = Instant::now();
    while std::net::TcpStream::connect(&host).is_err() {
        assert!(t0.elapsed() < Duration::from_secs(10), "server did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    let o = Command::new(s()).arg("run").arg(&path).output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    let err = String::from_utf8_lossy(&o.stderr).into_owned();
    let _ = server.kill();
    let mut sout = String::new();
    server.stdout.take().unwrap().read_to_string(&mut sout).unwrap();
    assert!(o.status.success(), "{} {}", out, err);
    assert!(out.contains(" 11"), "the mut capture came back over the wire: {:?} {:?}", out, err);
    assert!(sout.contains("far side"), "the zone printed where it ran: {:?}", sout);
    assert!(!out.contains("far side"));

    // the same world with net zones kept in this process
    let o = Command::new(s()).arg("run").arg(&path).arg("--net-here").output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(out.contains("far side") && out.contains(" 11"), "{:?}", out);
}

#[test]
fn a_different_world_is_refused() {
    let port = free_port();
    let host = format!("127.0.0.1:{}", port);
    let dir = std::env::temp_dir().join(format!("s3-net-{}", port));
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.json");
    let b = dir.join("b.json");
    std::fs::write(&a, net_world(&host, "// x~ 10 x + x~").to_json().pretty()).unwrap();
    std::fs::write(&b, net_world(&host, "// x~ 20 x + x~").to_json().pretty()).unwrap();
    let mut server = Command::new(s()).arg("serve").arg(&host).arg(&a).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let t0 = Instant::now();
    while std::net::TcpStream::connect(&host).is_err() {
        assert!(t0.elapsed() < Duration::from_secs(10), "server did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    let o = Command::new(s()).arg("run").arg(&b).output().unwrap();
    let _ = server.kill();
    let err = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(err.contains("zone lost: far") && err.contains("a different world"), "{:?}", err);
}

#[test]
fn a_coupled_world_calls_another_worlds_exported_gate() {
    let port = free_port();
    let host = format!("127.0.0.1:{}", port);
    // the serving world: an rpc zone that exports `double`
    let mut b = World::starter("b");
    op(&mut b, r#"{"op":"zone","name":"math","kind":"rpc","min":[10,0,0],"max":[19,9,9],"params":{"export":"true"}}"#);
    op(&mut b, r#"{"op":"sphere","kind":"gate","name":"double","pos":[10,5,5],"aim":[1,0,0]}"#);
    op(&mut b, r#"{"op":"sphere","kind":"fn","name":"work","pos":[15,5,5],"aim":[-1,0,0],"code":"+ =~ 2 * =~"}"#);
    let mut work = b.sphere_named("work").unwrap().clone();
    work.strike = Strike::Sticky;
    apply_op(&mut b, &J::parse(&Rec::Sphere(work).to_json().with("op", "put".into()).with("t", "sphere".into()).dump()).unwrap()).unwrap();
    // the calling world: an rpc zone with only the gate, coupled to the server
    let mut a = World::starter("a");
    op(&mut a, &format!(r#"{{"op":"zone","name":"far","kind":"rpc","min":[10,0,0],"max":[19,9,9],"params":{{"peer":"{}","image":"none"}}}}"#, host));
    op(&mut a, r#"{"op":"sphere","kind":"gate","name":"double","pos":[10,5,5],"aim":[1,0,0]}"#);
    column(&mut a, 0, 0, &["20", "1", "double.", ">", "3", "1", "double.", ">"]);
    let dir = std::env::temp_dir().join(format!("s3-couple-{}", port));
    std::fs::create_dir_all(&dir).unwrap();
    let (pa, pb) = (dir.join("a.json"), dir.join("b.json"));
    std::fs::write(&pa, a.to_json().pretty()).unwrap();
    std::fs::write(&pb, b.to_json().pretty()).unwrap();
    let mut server = Command::new(s()).arg("serve").arg(&host).arg(&pb).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    let t0 = Instant::now();
    while std::net::TcpStream::connect(&host).is_err() {
        assert!(t0.elapsed() < Duration::from_secs(10), "server did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    let o = Command::new(s()).arg("run").arg(&pa).output().unwrap();
    let _ = server.kill();
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    let err = String::from_utf8_lossy(&o.stderr).into_owned();
    assert!(o.status.success(), "{:?} {:?}", out, err);
    assert!(out.contains(" 42") && out.contains(" 8"), "{:?} {:?}", out, err);
}
