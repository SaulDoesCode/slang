// 0.3 tracks and machines: items advance a fixed number of cells per tick and queue,
// machines take them at a track's end, and a line of machines is a program.
use s3_core::build::build;
use s3_core::json::J;
use s3_core::machine::{rotate_cube, Item};
use s3_core::ops::apply_op;
use s3_core::rt::Program;
use s3_core::session::{run_headless, Session};
use s3_core::textform::{world_from_text, world_text};
use s3_core::world::*;

static P: Program = Program::with_lib(s3_core::LIB);

fn op(w: &mut World, j: &str) -> u32 {
    let e = J::parse(j).unwrap_or_else(|e| panic!("{}: {}", j, e));
    match apply_op(w, &e) {
        Ok((_, r)) => r.get("id").u(0),
        Err(e) => panic!("{}: {}", j, e),
    }
}

fn device(w: &mut World, kind: &str, cell: [i32; 3], code: &str) -> u32 {
    op(w, &J::obj().with("op", "sphere".into()).with("kind", kind.into()).with("cell", cell.into()).with("code", code.into()).dump())
}

fn wire(w: &mut World, a: u32, b: u32) {
    // devices in a row strike each other already
    if w.link_between(a, b).is_some() {
        return;
    }
    op(w, &format!(r#"{{"op":"link","from":{},"to":{}}}"#, a, b));
}

fn track(w: &mut World, from: [i32; 3], to: [i32; 3], speed: u32) -> u32 {
    op(w, &format!(r#"{{"op":"track","from":[{},{},{}],"to":[{},{},{}],"speed":{}}}"#, from[0], from[1], from[2], to[0], to[1], to[2], speed))
}

fn items(s: &Session, t: u32) -> Vec<(Item, u32)> {
    let g = s.zones[0].rt.g.as_ref().unwrap();
    g.tracks.iter().find(|x| x.id == t).unwrap().items.iter().map(|i| (i.item.clone(), i.pos)).collect()
}

fn shown(s: &Session, id: u32) -> Option<String> {
    let g = s.zones[0].rt.g.as_ref().unwrap();
    g.spheres.iter().find(|x| x.id == id).unwrap().shown.clone()
}

// a writer fed once with three values, onto a track with nothing at its end
fn feed(values: &str, speed: u32) -> (World, u32) {
    let mut w = World::starter("belt");
    let src = device(&mut w, "src", [0, 1, 5], values);
    let wr = device(&mut w, "fn", [1, 1, 5], "writer");
    wire(&mut w, src, wr);
    let t = track(&mut w, [2, 1, 5], [9, 1, 5], speed);
    (w, t)
}

#[test]
fn track_time() {
    let (w, t) = feed("5 7 9", 1);
    let (_, s) = run_headless(build(&w).unwrap(), &P, 1);
    assert_eq!(items(&s, t), vec![(Item::Cube(5), 0)], "one item enters a tick");
    let (_, s) = run_headless(build(&w).unwrap(), &P, 3);
    assert_eq!(items(&s, t), vec![(Item::Cube(5), 2), (Item::Cube(7), 1), (Item::Cube(9), 0)]);
    // nothing past the end: they queue at it, a cell apart
    let (_, s) = run_headless(build(&w).unwrap(), &P, 40);
    assert_eq!(items(&s, t), vec![(Item::Cube(5), 7), (Item::Cube(7), 6), (Item::Cube(9), 5)]);
    // a faster track carries further per tick, and still queues without overlap
    let (w, t) = feed("5 7 9", 3);
    let (_, s) = run_headless(build(&w).unwrap(), &P, 2);
    assert_eq!(items(&s, t), vec![(Item::Cube(5), 3), (Item::Cube(7), 0)]);
    let (_, s) = run_headless(build(&w).unwrap(), &P, 40);
    assert_eq!(items(&s, t).iter().map(|x| x.1).collect::<Vec<_>>(), vec![7, 6, 5]);
}

#[test]
fn tracks_are_checked() {
    let mut w = World::starter("t");
    track(&mut w, [0, 1, 0], [4, 1, 0], 1);
    for bad in [
        r#"{"op":"track","from":[2,1,-2],"to":[2,1,2]}"#,
        r#"{"op":"track","cells":[[0,1,3],[2,1,3]]}"#,
        r#"{"op":"track","cells":[[0,1,4],[1,1,4],[0,1,4]]}"#,
        r#"{"op":"track","from":[0,1,6],"to":[3,1,6],"speed":0}"#,
    ] {
        assert!(apply_op(&mut w, &J::parse(bad).unwrap()).is_err(), "{}", bad);
    }
    // a device cannot stand on a track
    assert!(apply_op(&mut w, &J::parse(r#"{"op":"sphere","kind":"fn","cell":[1,1,0]}"#).unwrap()).is_err());
}

#[test]
fn cutter_then_stacker_is_the_identity() {
    for v in [0x12AB7Fu32, 0, 0xFFFFFF, 0x800001] {
        let mut w = World::starter("line");
        let src = device(&mut w, "src", [0, 1, 0], &v.to_string());
        let wr = device(&mut w, "fn", [1, 1, 0], "writer");
        wire(&mut w, src, wr);
        track(&mut w, [2, 1, 0], [4, 1, 0], 1);
        device(&mut w, "fn", [5, 1, 0], "cutter");
        let bars = track(&mut w, [6, 1, 0], [9, 1, 0], 1);
        device(&mut w, "fn", [10, 1, 0], "stacker");
        track(&mut w, [11, 1, 0], [13, 1, 0], 2);
        let rd = device(&mut w, "fn", [14, 1, 0], "reader");
        let out = device(&mut w, "out", [14, 1, 2], "");
        wire(&mut w, rd, out);
        let (_, s) = run_headless(build(&w).unwrap(), &P, 8);
        let on: Vec<Item> = items(&s, bars).into_iter().map(|x| x.0).collect();
        assert!(on.iter().all(|i| matches!(i, Item::Bar(..))), "the cutter puts out bars: {:?}", on);
        let (_, s) = run_headless(build(&w).unwrap(), &P, 40);
        assert_eq!(shown(&s, out).as_deref(), Some(v.to_string().as_str()), "value {}", v);
    }
}

#[test]
fn painter_rotator_splitter_bin() {
    assert_eq!(rotate_cube(rotate_cube(rotate_cube(rotate_cube(0x12AB7F)))), 0x12AB7F);
    assert_eq!(rotate_cube(0xF000A0) & 0x00FF00, 0, "green stays where it is");
    let mut w = World::starter("m");
    let src = device(&mut w, "src", [0, 1, 0], "1 2 3 4");
    let wr = device(&mut w, "fn", [1, 1, 0], "writer");
    wire(&mut w, src, wr);
    track(&mut w, [2, 1, 0], [3, 1, 0], 1);
    let p = device(&mut w, "fn", [4, 1, 0], "painter");
    let mut sp = w.spheres.get(&p).unwrap().clone();
    sp.params.push(("or".into(), 0xFF0000 as f64));
    w.apply(vec![Edit::Put(Rec::Sphere(sp))]).unwrap();
    track(&mut w, [5, 1, 0], [6, 1, 0], 1);
    device(&mut w, "fn", [7, 1, 0], "splitter");
    let a = track(&mut w, [7, 1, 1], [7, 1, 4], 1);
    let b = track(&mut w, [7, 1, -1], [7, 1, -4], 1);
    let (_, s) = run_headless(build(&w).unwrap(), &P, 60);
    let va: Vec<Item> = items(&s, a).into_iter().map(|x| x.0).collect();
    let vb: Vec<Item> = items(&s, b).into_iter().map(|x| x.0).collect();
    assert_eq!(va, vec![Item::Cube(0xFF0001), Item::Cube(0xFF0003)], "the splitter takes turns");
    assert_eq!(vb, vec![Item::Cube(0xFF0002), Item::Cube(0xFF0004)]);
    // with a bin past the end, nothing stays
    device(&mut w, "fn", [7, 1, 5], "bin");
    let (_, s) = run_headless(build(&w).unwrap(), &P, 60);
    assert!(items(&s, a).is_empty());
}

#[test]
fn tracks_in_the_world_text() {
    let (w, _) = feed("5 7 9", 2);
    let t = world_text(&w);
    assert!(t.contains("'track1' 2 2 1 5 3 1 5"), "{}", t);
    let back = world_from_text(&t).unwrap();
    assert_eq!(world_text(&back), t);
    assert_eq!(back.tracks.values().next().unwrap().cells.len(), 8);
}

#[test]
fn a_live_edit_keeps_the_items_riding() {
    let (mut w, t) = feed("5 7 9", 1);
    let mut s = Session::new(build(&w).unwrap(), &P);
    s.run_to_end(u64::MAX);
    for _ in 0..3 {
        s.tick();
        s.run_to_end(u64::MAX);
    }
    let before = items(&s, t);
    assert_eq!(before.len(), 3);
    // a brick placed elsewhere: the track is the same, so its items stay where they are
    let p = *w.plates.keys().next().unwrap();
    let _ = p;
    op(&mut w, r#"{"op":"place","space":0,"cell":[0,1,0],"word":"1"}"#);
    let spec = build(&w).unwrap();
    s.patch(spec, Vec::new()).unwrap();
    s.tick();
    s.run_to_end(u64::MAX);
    let after = items(&s, t);
    assert_eq!(after.iter().map(|x| x.1).collect::<Vec<_>>(), vec![3, 2, 1], "{:?}", after);
}
