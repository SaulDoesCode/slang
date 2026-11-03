use s3_core::build::build;
use s3_core::json::J;
use s3_core::ops::{apply_op, import_text};
use s3_core::rt::Program;
use s3_core::session::{run_headless, Session, StepKind};
use s3_core::world::*;

static P: Program = Program::with_lib(s3_core::LIB);

fn op(w: &mut World, j: &str) -> J {
    let e = J::parse(j).unwrap();
    match apply_op(w, &e) {
        Ok((_, r)) => r,
        Err(e) => panic!("{}: {}", j, e),
    }
}

fn run(w: &World) -> String {
    let spec = build(w).map_err(|p| p.iter().map(|x| x.msg.clone()).collect::<Vec<_>>().join("; ")).unwrap();
    run_headless(spec, &P, 0).0
}

fn column(w: &mut World, x: i32, z: i32, words: &[&str]) {
    for (i, word) in words.iter().enumerate() {
        op(w, &format!(r#"{{"op":"place","space":0,"cell":[{},{},{}],"word":{}}}"#, x, i + 1, z, J::from(*word).dump()));
    }
}

#[test]
fn a_column_reads_bottom_to_top() {
    let mut w = World::starter("t");
    column(&mut w, 0, 0, &["1", "2", "+", "=~", ">"]);
    assert_eq!(run(&w), "\n 3");
}

#[test]
fn reading_order_follows_the_arrow() {
    let mut w = World::starter("t");
    column(&mut w, 1, 0, &["'b'", ">"]);
    column(&mut w, 0, 0, &["'a'", ">"]);
    column(&mut w, 0, 1, &["'c'", ">"]);
    assert_eq!(run(&w), "\n a\n b\n c");
    let p = *w.plates.keys().next().unwrap();
    let mut pl = w.plates[&p].clone();
    pl.arrow = Arrow::Nx;
    w.apply(vec![Edit::Put(Rec::Plate(pl))]).unwrap();
    assert_eq!(run(&w), "\n c\n b\n a");
}

#[test]
fn caps_and_collars_write_the_sigils() {
    let mut w = World::starter("t");
    column(&mut w, 0, 0, &["5", "x#"]);
    column(&mut w, 1, 0, &["x~", "3", ">>", "'big'", ">?"]);
    column(&mut w, 2, 0, &["⟨y.⟩~?"]);
    let b = w.brick_at(0, [0, 2, 0]).unwrap().clone();
    assert_eq!((b.kind, b.word.as_str(), b.cap.as_str()), (BKind::Name, "x", "#"));
    let c = w.brick_at(0, [1, 5, 0]).unwrap().clone();
    assert_eq!((c.kind, c.word.as_str(), c.cond.as_str()), (BKind::Cmd, ">", "?"));
    let d = w.brick_at(0, [2, 1, 0]).unwrap().clone();
    assert_eq!((d.kind, d.word.as_str(), d.cap.as_str(), d.cond.as_str()), (BKind::Name, "y.", "~", "?"));
    assert_eq!(word_text(&d), "y.~?");
    assert_eq!(run(&w), "\n big");
}

#[test]
fn boxes_hold_plates_and_shelves_define_by_height() {
    let mut w = World::starter("t");
    let shelf = op(&mut w, r#"{"op":"plate","space":0,"kind":"shelf","at":[0,-6,-4],"w":6,"d":1}"#).get("id").u(0);
    let _ = shelf;
    // generation 0: twice = 2 *; generation 1 (higher shelf): twice = 3 * (newest wins on main)
    for (y, body) in [(-5, "{2 * =~}"), (-5 + 0, "")] {
        if body.is_empty() {
            continue;
        }
        op(&mut w, &format!(r#"{{"op":"place","space":0,"cell":[0,{},-4],"word":{}}}"#, y, J::from(body).dump()));
        op(&mut w, &format!(r#"{{"op":"place","space":0,"cell":[0,{},-4],"word":"'twice'"}}"#, y + 1));
        op(&mut w, &format!(r#"{{"op":"place","space":0,"cell":[0,{},-4],"word":"><"}}"#, y + 2));
    }
    column(&mut w, 0, 0, &["21", "twice", ">"]);
    assert_eq!(run(&w), "\n 42");
    let bx = w.brick_at(0, [0, -5, -4]).unwrap().clone();
    assert_eq!(bx.kind, BKind::Box);
    assert_eq!(w.space_text(bx.inner), "2 * =~");
    // a higher shelf is a later generation
    op(&mut w, r#"{"op":"plate","space":0,"kind":"shelf","at":[0,0,-4],"w":6,"d":1}"#);
    op(&mut w, r#"{"op":"place","space":0,"cell":[0,1,-4],"word":"{3 * =~}"}"#);
    op(&mut w, r#"{"op":"place","space":0,"cell":[0,2,-4],"word":"'twice'"}"#);
    op(&mut w, r#"{"op":"place","space":0,"cell":[0,3,-4],"word":"><"}"#);
    assert_eq!(run(&w), "\n 63");
    let r = s3_core::query::resolve(&w, w.brick_at(0, [0, 2, 0]).unwrap().id);
    assert_eq!(r.get("kind").s(""), "def");
    assert_eq!(r.get("gen").u(9), 1);
}

#[test]
fn spheres_strike_and_pulses_carry_lines() {
    let mut w = World::starter("t");
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"seed","pos":[0,4,0],"aim":[1,0,0],"code":"1 2"}"#);
    op(&mut w, r#"{"op":"sphere","kind":"fn","name":"add","pos":[5,4,0],"aim":[1,0,0],"code":"+ =~ 10"}"#);
    op(&mut w, r#"{"op":"sphere","kind":"out","name":"show","pos":[10,4,0]}"#);
    assert_eq!(w.links.len(), 2, "strikes made links");
    let spec = build(&w).unwrap();
    let mut s = Session::new(spec, &P);
    s.run_to_end(u64::MAX);
    let r = s.report(false);
    let shown: Vec<String> = r.get("zones").arr()[0].get("graph").get("spheres").arr().iter().filter_map(|x| x.get("shown").str().map(|t| t.to_string())).collect();
    assert_eq!(shown, vec!["3 10".to_string()]);
    assert_eq!(r.get("trace").arr().len(), 3);
}

#[test]
fn a_ratchet_loops_n_times_then_exits() {
    let mut w = World::starter("t");
    let seed = op(&mut w, r#"{"op":"sphere","kind":"src","name":"seed","pos":[-9,0,0],"aim":[0,1,0],"code":"1"}"#).get("id").u(0);
    let lp = op(&mut w, r#"{"op":"sphere","kind":"ratchet","name":"loop","pos":[-4,0,0],"aim":[0,1,0],"params":{"n":5}}"#).get("id").u(0);
    let dbl = op(&mut w, r#"{"op":"sphere","kind":"fn","name":"double","pos":[0,3,0],"aim":[0,1,0],"code":"2 * =~"}"#).get("id").u(0);
    let res = op(&mut w, r#"{"op":"sphere","kind":"out","name":"result","pos":[0,-3,0]}"#).get("id").u(0);
    op(&mut w, &format!(r#"{{"op":"link","from":{},"to":{},"mode":"sticky"}}"#, seed, lp));
    op(&mut w, &format!(r#"{{"op":"link","from":{},"to":{},"mode":"sticky","port":"loop"}}"#, lp, dbl));
    op(&mut w, &format!(r#"{{"op":"link","from":{},"to":{},"mode":"sticky","port":"exit"}}"#, lp, res));
    op(&mut w, &format!(r#"{{"op":"link","from":{},"to":{},"mode":"sticky"}}"#, dbl, lp));
    let (_, mut s) = run_headless(build(&w).unwrap(), &P, 0);
    let r = s.report(false);
    let g = r.get("zones").arr()[0].get("graph").clone();
    let shown = g.get("spheres").arr().iter().find(|x| x.get("id").u(0) == res).unwrap().get("shown").s("");
    assert_eq!(shown, "32");
}

#[test]
fn ratchet_bodies_see_their_count_as_a_local() {
    let mut w = World::starter("t");
    let seed = op(&mut w, r#"{"op":"sphere","kind":"src","name":"seed","pos":[-9,0,0],"aim":[0,1,0],"code":"go"}"#).get("id").u(0);
    let lp = op(&mut w, r#"{"op":"sphere","kind":"ratchet","name":"loop","pos":[-4,0,0],"aim":[0,0,1],"params":{"n":3},"code":"0 ( :i~ > )"}"#).get("id").u(0);
    let back = op(&mut w, r#"{"op":"sphere","kind":"fn","name":"back","pos":[-4,5,0],"aim":[0,1,0]}"#).get("id").u(0);
    op(&mut w, &format!(r#"{{"op":"link","from":{},"to":{}}}"#, seed, lp));
    op(&mut w, &format!(r#"{{"op":"link","from":{},"to":{},"port":"loop"}}"#, lp, back));
    op(&mut w, &format!(r#"{{"op":"link","from":{},"to":{}}}"#, back, lp));
    let (out, _) = run_headless(build(&w).unwrap(), &P, 0);
    assert_eq!(out, "\n 0\n 1\n 2\n 3");
}

#[test]
fn moving_reroutes_a_ray() {
    let mut w = World::starter("t");
    let clock = op(&mut w, r#"{"op":"sphere","kind":"src","name":"go","pos":[0,0,-5],"aim":[0,0,-1],"code":"1 2 3 4","reach":2}"#).get("id").u(0);
    let scan = op(&mut w, r#"{"op":"sphere","kind":"fn","name":"scan","pos":[0,0,0],"aim":[0,0,-1],"strike":"ray","reach":30,"code":"3 0 0 move hit"}"#).get("id").u(0);
    op(&mut w, &format!(r#"{{"op":"link","from":{},"to":{}}}"#, clock, scan));
    for k in 0..3 {
        op(&mut w, &format!(r#"{{"op":"sphere","kind":"out","name":"t{}","pos":[{},0,-10]}}"#, k, 3 * k));
    }
    let (_, mut s) = run_headless(build(&w).unwrap(), &P, 0);
    let r = s.report(false);
    let g = r.get("zones").arr()[0].get("graph").clone();
    let shown: Vec<(String, String)> = g
        .get("spheres")
        .arr()
        .iter()
        .filter(|x| x.has("shown"))
        .map(|x| (w.spheres[&x.get("id").u(0)].name.clone(), x.get("shown").s("")))
        .collect();
    // scan starts aimed at t0; after moving 3 along x its ray strikes t1, and the line goes there
    assert_eq!(shown, vec![("t1".to_string(), "1 2 3 4 t1".to_string())]);
}

#[test]
fn errors_end_the_branch_and_point_at_the_brick() {
    let mut w = World::starter("t");
    op(&mut w, r#"{"op":"sphere","kind":"src","name":"seed","pos":[0,0,0],"aim":[1,0,0],"code":"nope."}"#);
    let spec = build(&w).unwrap();
    let mut s = Session::new(spec, &P);
    s.run_to_end(u64::MAX);
    let r = s.report(false);
    let e = &r.get("errors").arr()[0];
    assert!(e.get("msg").s("").contains("nope"));
    let b = e.get("brick").u(0);
    assert_eq!(w.bricks[&b].word, "nope");
}

#[test]
fn stepping_token_by_token_reports_the_brick() {
    let mut w = World::starter("t");
    column(&mut w, 0, 0, &["1", "2", "+"]);
    let spec = build(&w).unwrap();
    let mut s = Session::new(spec, &P);
    let mut seen = Vec::new();
    for _ in 0..3 {
        s.run(1, StepKind::Token);
        let r = s.report(true);
        seen.push(r.get("zones").arr()[0].get("brick").u(0));
    }
    let ids: Vec<u32> = (1..=3).map(|y| w.brick_at(0, [0, y, 0]).unwrap().id).collect();
    assert_eq!(seen, ids);
}

#[test]
fn undo_restores_the_world_exactly() {
    let mut w = World::starter("t");
    column(&mut w, 0, 0, &["1", "{2 3 +}", "x$"]);
    let before = w.to_json().dump();
    let (a, _) = apply_op(&mut w, &J::parse(r#"{"op":"remove","ids":[3]}"#).unwrap()).unwrap();
    assert_ne!(w.to_json().dump(), before);
    w.apply_raw(a.inverse);
    let mut after = w.to_json();
    after.set("next", J::parse(&before).unwrap().get("next").clone());
    assert_eq!(after.dump(), before);
}

#[test]
fn json_round_trip() {
    let mut w = World::starter("t");
    column(&mut w, 0, 0, &["1", "{2 { 3 } +}", "x$", "'a b'"]);
    op(&mut w, r#"{"op":"sphere","kind":"ratchet","name":"r","pos":[0,4,0]}"#);
    let j = w.to_json();
    let w2 = World::from_json(&J::parse(&j.pretty()).unwrap()).unwrap();
    assert_eq!(w2.to_json().dump(), j.dump());
}

#[test]
fn importing_s_text_gives_it_back() {
    let src = "{2 *} 'dbl' ><\n{x~ >} show$\n5 x#\nshow.\n7 dbl =~ >\n{ } 'sp' ><\n";
    let w = import_text("t", src).unwrap();
    let spec = build(&w).unwrap();
    let text = &spec.units[spec.zones[0].unit].text;
    assert_eq!(text, "{2 *} 'dbl' ><\n{x~ >} show$\n5 x#\nshow.\n7 dbl =~ >\n{ } 'sp' ><");
}

#[test]
fn rotate_turns_bricks_and_spheres_about_the_pivot() {
    let mut w = World::starter("t");
    column(&mut w, 2, 2, &["'r'", ">"]);
    let ids: Vec<u32> = w.bricks.values().map(|b| b.id).collect();
    op(&mut w, r#"{"op":"sphere","kind":"fn","name":"s","pos":[6.5,3,4.5],"aim":[1,0,0]}"#);
    let sid = w.sphere_named("s").unwrap().id;
    let all: Vec<J> = ids.iter().chain([sid].iter()).map(|&i| J::Num(i as f64)).collect();
    op(&mut w, &J::obj().with("op", "rotate".into()).with("ids", J::Arr(all.clone())).with("pivot", J::Arr(vec![J::Num(4.0), J::Num(1.0), J::Num(4.0)])).dump());
    let mut cells: Vec<[i32; 3]> = w.bricks.values().map(|b| b.cell).collect();
    cells.sort();
    assert_eq!(cells, vec![[2, 1, 6], [2, 2, 6]]);
    let s = w.sphere_named("s").unwrap();
    assert_eq!((s.pos, s.aim.map(|x| x.round())), ([4.5, 3.0, 2.5], [0.0, 0.0, -1.0]));
    assert_eq!(run(&w), "\n r");
    // three more quarter turns come back
    op(&mut w, &J::obj().with("op", "rotate".into()).with("ids", J::Arr(all)).with("pivot", J::Arr(vec![J::Num(4.0), J::Num(1.0), J::Num(4.0)])).with("turns", J::Num(3.0)).dump());
    let mut cells: Vec<[i32; 3]> = w.bricks.values().map(|b| b.cell).collect();
    cells.sort();
    assert_eq!(cells, vec![[2, 1, 2], [2, 2, 2]]);
}
