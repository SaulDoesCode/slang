// 0.3: devices on the grid, wires and beams, masts, assemblies, and the world as text.
use s3_core::build::build;
use s3_core::json::J;
use s3_core::ops::apply_op;
use s3_core::rt::Program;
use s3_core::session::run_headless;
use s3_core::textform::{world_from_text, world_text};
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
    w.sphere_named(n).unwrap_or_else(|| panic!("no sphere {}", n)).id
}

fn shown(w: &World, name: &str) -> Option<String> {
    let (_, mut s) = run_headless(build(w).unwrap_or_else(|p| panic!("{:?}", p)), &P, 0);
    let r = s.report(false);
    let errs: Vec<String> = s.errors.iter().map(|e| e.get("msg").s("")).collect();
    assert!(errs.is_empty(), "{:?}", errs);
    for z in r.get("zones").arr() {
        for sp in z.get("graph").get("spheres").arr() {
            if sp.get("name").s("") == name {
                return sp.get("shown").str().map(|x| x.to_string());
            }
        }
    }
    None
}

fn traces(w: &World) -> Vec<String> {
    let (_, s) = run_headless(build(w).unwrap(), &P, 0);
    s.trace.iter().filter(|t| t.has("s")).map(|t| format!("{} {}", t.get("in").dump(), t.get("out").dump())).collect()
}

/// seed (src) → double (fn) → show (out), in a row along +x
fn chain() -> World {
    let mut w = World::starter("chain");
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"seed","pos":[-6,4,6],"aim":[1,0,0],"code":"21"}"#);
    op(&mut w, r#"{"op":"sphere","kind":"fn","name":"double","pos":[0,4,6],"aim":[1,0,0],"code":"2 * =~"}"#);
    op(&mut w, r#"{"op":"sphere","kind":"out","name":"show","pos":[6,4,6]}"#);
    w
}

#[test]
fn the_world_text_is_a_fixed_point() {
    let mut worlds = vec![chain()];
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../levels");
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let j = J::parse(&std::fs::read_to_string(&p).unwrap()).unwrap();
        worlds.push(World::from_json(&j).unwrap());
    }
    let mut z = chain();
    op(&mut z, r#"{"op":"zone","name":"inside","kind":"thread","min":[10,0,0],"max":[19,9,9],"captures":[["x","mut"]],"params":{"budget":"500"}}"#);
    let dd = id(&z, "double");
    op(&mut z, &format!(r#"{{"op":"setdown","ids":[{}]}}"#, dd));
    op(&mut z, r#"{"op":"sphere","kind":"ratchet","name":"turn","pos":[3,8,3],"params":{"n":3},"reach":20}"#);
    worlds.push(z);
    for w in worlds {
        let t = world_text(&w);
        let back = world_from_text(&t).unwrap_or_else(|e| panic!("{}\n{}", e, t));
        assert_eq!(world_text(&back), t, "{}", w.name);
        let units = |w: &World| build(w).map(|s| s.zones.iter().map(|z| s.units[z.unit].text.clone()).collect::<Vec<_>>()).unwrap_or_default();
        assert_eq!(units(&back), units(&w), "{}", w.name);
    }
}

#[test]
fn setting_down_and_lifting_keep_the_program() {
    let w0 = chain();
    let before = traces(&w0);
    let mut w = w0.clone();
    let ids: Vec<u32> = w.spheres.keys().copied().collect();
    op(&mut w, &J::obj().with("op", "setdown".into()).with("ids", J::Arr(ids.iter().map(|x| J::Num(*x as f64)).collect())).dump());
    assert!(w.spheres.values().all(|s| s.cell.is_some()));
    assert_eq!(traces(&w), before, "sticky links are wires on the grid");
    op(&mut w, &J::obj().with("op", "lift".into()).with("ids", J::Arr(ids.iter().map(|x| J::Num(*x as f64)).collect())).dump());
    for s in w0.spheres.values() {
        assert_eq!(Some(s), w.spheres.get(&s.id), "lifting returns the sphere as it was");
    }
    assert_eq!(traces(&w), before);
}

#[test]
fn a_mast_carries_lines_between_the_ground_and_the_sphere_layer() {
    // ground: a source device wired to a mast; air: the mast's top aims at a floating out
    let mut w = World::starter("mast");
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"ground","cell":[0,1,0],"aim":[0,0,1],"code":"'up'"}"#);
    op(&mut w, r#"{"op":"sphere","kind":"mast","name":"pole","cell":[4,1,0],"aim":[1,0,0]}"#);
    op(&mut w, r#"{"op":"sphere","kind":"out","name":"air","pos":[9,5.5,0.5]}"#);
    let (g, p) = (id(&w, "ground"), id(&w, "pole"));
    op(&mut w, &format!(r#"{{"op":"link","from":{},"to":{},"route":[[1,1,0],[2,1,0],[3,1,0]]}}"#, g, p));
    assert!(w.link_between(p, id(&w, "air")).is_some(), "the mast's top strikes the floating sphere");
    assert_eq!(shown(&w, "air").as_deref(), Some("up"));
    // and down: a floating source strikes the mast top, which wires to an out device
    let mut w = World::starter("mast2");
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"sky","pos":[4.5,5.5,-6],"aim":[0,0,1],"code":"'down'"}"#);
    op(&mut w, r#"{"op":"sphere","kind":"mast","name":"pole","cell":[4,1,0],"aim":[0,-1,0],"reach":0.1}"#);
    op(&mut w, r#"{"op":"sphere","kind":"out","name":"lamp","cell":[8,1,0]}"#);
    assert!(w.link_between(id(&w, "sky"), id(&w, "pole")).is_some());
    let (p, l) = (id(&w, "pole"), id(&w, "lamp"));
    op(&mut w, &format!(r#"{{"op":"link","from":{},"to":{}}}"#, p, l));
    assert_eq!(shown(&w, "lamp").as_deref(), Some("down"));
}

#[test]
fn an_assembly_keeps_the_program_and_edits_reach_every_placement() {
    let mut w = chain();
    assert_eq!(shown(&w, "show").as_deref(), Some("42"));
    let d = id(&w, "double");
    let r = op(&mut w, &format!(r#"{{"op":"assemble","ids":[{}],"name":"doubler"}}"#, d));
    let place = r.get("id").u(0);
    assert_eq!(w.spheres[&place].kind, SKind::Asm);
    assert_eq!(shown(&w, "show").as_deref(), Some("42"), "{}", world_text(&w));
    // a second placement fed by its own source
    op(&mut w, r#"{"op":"instance","assembly":"doubler","name":"twice","pos":[0,10,6]}"#);
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"seed2","pos":[-6,10,6],"aim":[1,0,0],"code":"5"}"#);
    op(&mut w, r#"{"op":"sphere","kind":"out","name":"show2","pos":[6,10,6]}"#);
    let (s2, t, o2) = (id(&w, "seed2"), id(&w, "twice"), id(&w, "show2"));
    let out_pin = w.links.values().find(|l| l.from == place).unwrap().from_pin.clone();
    assert!(w.link_between(s2, t).is_some(), "aiming at a placement feeds its first input port");
    op(&mut w, &format!(r#"{{"op":"link","from":{},"to":{},"from_pin":"{}"}}"#, t, o2, out_pin));
    assert_eq!(shown(&w, "show2").as_deref(), Some("10"));
    // one edit inside changes both
    let body = w.spheres.values().find(|s| s.name == "double").unwrap().body;
    op(&mut w, &format!(r#"{{"op":"settext","space":{},"text":"3 * =~"}}"#, body));
    assert_eq!(shown(&w, "show").as_deref(), Some("63"));
    assert_eq!(shown(&w, "show2").as_deref(), Some("15"));
    // and the world text carries it
    let back = world_from_text(&world_text(&w)).unwrap();
    assert_eq!(shown(&back, "show2").as_deref(), Some("15"));
}

#[test]
fn a_wire_holds_and_a_beam_follows_what_it_strikes() {
    let mut w = World::starter("beam");
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"lens","cell":[0,1,0],"aim":[1,0,0],"strike":"ray","code":"'lit'"}"#);
    op(&mut w, r#"{"op":"sphere","kind":"out","name":"a","cell":[5,1,0]}"#);
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"feed","cell":[0,1,3],"aim":[1,0,0],"code":"'fed'"}"#);
    op(&mut w, r#"{"op":"sphere","kind":"out","name":"b","cell":[5,1,3]}"#);
    assert_eq!(w.link_between(id(&w, "lens"), id(&w, "a")).map(|l| l.mode), Some(Strike::Ray));
    assert_eq!(shown(&w, "a").as_deref(), Some("lit"));
    // move a out of the beam and c into it; move b off its wire's line
    let a = id(&w, "a");
    op(&mut w, &format!(r#"{{"op":"move","ids":[{}],"delta":[0,0,1]}}"#, a));
    op(&mut w, r#"{"op":"sphere","kind":"out","name":"c","cell":[7,1,0]}"#);
    let b = id(&w, "b");
    op(&mut w, &format!(r#"{{"op":"move","ids":[{}],"delta":[0,0,2]}}"#, b));
    assert_eq!(shown(&w, "c").as_deref(), Some("lit"), "the beam reaches what it strikes now");
    assert_eq!(shown(&w, "a"), None);
    assert_eq!(shown(&w, "b").as_deref(), Some("fed"), "the wire keeps its connection");
}
