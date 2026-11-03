// 0.3 datum cubes: tiles and bits, the transform parts as schema and as names, the
// mutation log as a program, and the cube in the world text.
use s3_core::build::build;
use s3_core::json::J;
use s3_core::ops::{apply_op, rebuild_log};
use s3_core::rt::Program;
use s3_core::session::Session;
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

fn try_op(w: &mut World, j: &str) -> Result<J, String> {
    apply_op(w, &J::parse(j).unwrap()).map(|x| x.1)
}

fn datum(w: &World, name: &str) -> Datum {
    w.datum_named(name).cloned().unwrap_or_else(|| panic!("no datum {}", name))
}

fn main_text(w: &mut World, text: &str) {
    let p = *w.plates.keys().next().unwrap();
    op(w, &J::obj().with("op", "settext".into()).with("plate", p.into()).with("text", text.into()).dump());
}

fn run(w: &World) -> (String, Session) {
    let mut s = Session::new(build(w).unwrap_or_else(|p| panic!("{:?}", p)), &P);
    s.run_to_end(u64::MAX);
    s.end_zones();
    s.flush();
    let errs: Vec<String> = s.errors.iter().map(|e| e.get("msg").s("")).collect();
    assert!(errs.is_empty(), "{:?}", errs);
    (s.output(), s)
}

fn cube() -> (World, u32) {
    let mut w = World::starter("cubes");
    let r = op(&mut w, r#"{"op":"datum","cell":[3,1,20]}"#);
    (w, r.get("id").u(0))
}

#[test]
fn cube_bits_each_tile_toggles_its_own_bit() {
    let (mut w, id) = cube();
    assert_eq!(datum(&w, "c1").value, 0);
    let mut seen = 0u32;
    for (f, face) in FACE_NAMES.iter().enumerate() {
        for v in 0..2 {
            for u in 0..2 {
                let before = datum(&w, "c1").value;
                let r = op(&mut w, &format!(r#"{{"op":"tile","id":{},"face":"{}","u":{},"v":{}}}"#, id, face, u, v));
                let bit = r.get("bit").u(99);
                assert_eq!(bit, tile_bit(f, u as usize, v as usize));
                assert_eq!(datum(&w, "c1").value, before ^ (1 << bit), "face {} tile {} {}", face, u, v);
                seen |= 1 << bit;
            }
        }
    }
    assert_eq!(seen, 0xFF_FFFF, "24 tiles cover 24 bits");
    assert_eq!(datum(&w, "c1").value, 0xFF_FFFF);
    // face and nibble order: +x high red, -x low red, ±y green, ±z blue; tile (0, 0) highest
    assert_eq!(tile_bit(0, 0, 0), 23);
    assert_eq!(tile_bit(1, 1, 1), 16);
    assert_eq!(tile_bit(2, 0, 0), 15);
    assert_eq!(tile_bit(3, 0, 0), 11);
    assert_eq!(tile_bit(4, 0, 0), 7);
    assert_eq!(tile_bit(5, 1, 1), 0);
    // toggling again clears
    op(&mut w, &format!(r#"{{"op":"tile","id":{},"face":"+x","u":0,"v":0}}"#, id));
    assert_eq!(datum(&w, "c1").value, 0x7F_FFFF);
}

#[test]
fn datum_cells_are_exclusive_and_values_bounded() {
    let (mut w, _) = cube();
    assert!(try_op(&mut w, r#"{"op":"datum","cell":[3,1,20]}"#).is_err());
    assert!(try_op(&mut w, r#"{"op":"datum","cell":[4,1,20],"value":16777216}"#).is_err());
    assert!(try_op(&mut w, r#"{"op":"datum","cell":[4,1,20],"name":"7up"}"#).is_err());
    assert!(try_op(&mut w, r#"{"op":"sphere","kind":"fn","cell":[3,1,20]}"#).is_err());
    let r = op(&mut w, r#"{"op":"datum","cell":[4,1,20],"value":255}"#);
    assert_eq!(datum(&w, "c2").value, 255);
    assert!(try_op(&mut w, &format!(r#"{{"op":"transform","id":{},"parts":{{"sx":0}}}}"#, r.get("id").u(0))).is_err());
    assert!(try_op(&mut w, &format!(r#"{{"op":"transform","id":{},"parts":{{"sw":2}}}}"#, r.get("id").u(0))).is_err());
}

#[test]
fn datum_schema_holds_only_present_parts() {
    let (mut w, id) = cube();
    let j = w.rec(id).unwrap().to_json();
    assert!(j.get("scale").is_null() && j.get("rot").is_null(), "standard parts are not stored: {}", j.dump());
    op(&mut w, &format!(r#"{{"op":"transform","id":{},"parts":{{"sx":2.5,"ry":45}}}}"#, id));
    let d = datum(&w, "c1");
    assert_eq!(d.parts(), vec![("sx", 2.5), ("ry", 45.0)]);
    op(&mut w, &format!(r#"{{"op":"transform","id":{},"parts":{{"sx":1}}}}"#, id));
    assert_eq!(datum(&w, "c1").parts(), vec![("ry", 45.0)], "setting the default removes the part");
    let j = w.rec(id).unwrap().to_json();
    assert!(j.get("scale").is_null() && !j.get("rot").is_null());

    // code sees the parts as names: absent ones read as nothing, schema lists the present
    main_text(&mut w, "c1 schema >\n'v' c1~ >\n'ry' c1/ry~ >\n3 c1/sy#\n'after' c1 schema >\n1 c1/sy#\n'back' c1 schema >");
    let (out, mut s) = run(&w);
    let lines: Vec<&str> = out.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    assert_eq!(lines[0], "c1/ry");
    assert_eq!(lines[1], "v 0");
    assert_eq!(lines[2], "ry 45");
    assert_eq!(lines[3], "after c1/sy c1/ry");
    assert_eq!(lines[4], "back c1/ry", "{:?}", out);
    // the report carries what code left
    let r = s.report(false);
    let d = r.get("zones").arr()[0].get("datums").arr()[0].clone();
    assert_eq!(d.get("id").u(0), id);
    assert_eq!(d.get("parts").get("ry").num(), Some(45.0));
    assert!(d.get("parts").get("sy").is_null());
}

#[test]
fn code_writes_value_and_parts_into_the_report() {
    let (mut w, id) = cube();
    main_text(&mut w, "16711680 c1#\n2 c1/sz#\n-90 c1/rx#");
    let (_, mut s) = run(&w);
    let r = s.report(false);
    let d = r.get("zones").arr()[0].get("datums").arr()[0].clone();
    assert_eq!(d.get("id").u(0), id);
    assert_eq!(d.get("value").num(), Some(16711680.0));
    assert_eq!(d.get("parts").get("sz").num(), Some(2.0));
    assert_eq!(d.get("parts").get("rx").num(), Some(-90.0));
}

#[test]
fn datum_frame_rotation_keeps_the_value() {
    let (mut w, id) = cube();
    op(&mut w, &format!(r#"{{"op":"tile","id":{},"face":"+x","u":0,"v":0}}"#, id));
    let v = datum(&w, "c1").value;
    op(&mut w, &format!(r#"{{"op":"rotate","ids":[{}],"pivot":[0,1,20],"turns":1}}"#, id));
    let d = datum(&w, "c1");
    assert_eq!(d.value, v, "turning a datum leaves its bits");
    assert_eq!(d.rot, [0.0, 90.0, 0.0]);
    assert_eq!(d.cell, [0, 1, 17]);
    op(&mut w, &format!(r#"{{"op":"rotate","ids":[{}],"pivot":[0,1,20],"turns":3}}"#, id));
    let d = datum(&w, "c1");
    assert_eq!(d.rot, [0.0; 3]);
    assert_eq!(d.cell, [3, 1, 20]);
    assert!(d.parts().is_empty());
    // angles stay in (−180, 180]
    op(&mut w, &format!(r#"{{"op":"transform","id":{},"rot":[0,270,-540]}}"#, id));
    assert_eq!(datum(&w, "c1").rot, [0.0, -90.0, 180.0]);
}

#[test]
fn mutation_log_replays_to_the_same_state() {
    let (mut w, id) = cube();
    for (f, u, v) in [("+x", 0, 0), ("-y", 1, 0), ("+z", 1, 1), ("+x", 0, 0), ("-z", 0, 1)] {
        op(&mut w, &format!(r#"{{"op":"tile","id":{},"face":"{}","u":{},"v":{}}}"#, id, f, u, v));
    }
    op(&mut w, &format!(r#"{{"op":"transform","id":{},"parts":{{"sx":2.5,"sz":0.0625,"ry":45}}}}"#, id));
    op(&mut w, &format!(r#"{{"op":"transform","id":{},"parts":{{"sz":1}}}}"#, id));
    let d = datum(&w, "c1");
    assert_eq!(d.log[0], "0 c1#");
    assert_eq!(d.log[1], "8388608 c1;");
    assert!(d.log.contains(&"8388608 c1:".to_string()));
    assert!(d.log.contains(&"2.5 c1/sx#".to_string()));
    assert!(d.log.contains(&"45 c1/ry#".to_string()));
    // replayed in a world without the datum, the log rebuilds its value and parts
    let mut fresh = World::starter("replay");
    let text = format!("{}\n'v' c1~ >\n'sx' c1/sx~ >\n'sz' c1/sz~ >\n'ry' c1/ry~ >\n", d.log.join("\n"));
    main_text(&mut fresh, &text);
    let (out, _) = run(&fresh);
    let lines: Vec<&str> = out.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    assert_eq!(lines, vec![format!("v {}", d.value).as_str(), "sx 2.5", "sz 1", "ry 45"]);
    // folding the log keeps it a rebuild
    let short = rebuild_log(&d);
    assert_eq!(short, vec![format!("{} c1#", d.value), "2.5 c1/sx#".to_string(), "45 c1/ry#".to_string()]);
    op(&mut w, &format!(r#"{{"op":"clearlog","id":{}}}"#, id));
    assert_eq!(datum(&w, "c1").log, short);
}

#[test]
fn a_long_log_folds_into_a_rebuild() {
    let (mut w, id) = cube();
    for _ in 0..300 {
        op(&mut w, &format!(r#"{{"op":"tile","id":{},"face":"+y","u":1,"v":1}}"#, id));
    }
    let d = datum(&w, "c1");
    assert!(d.log.len() <= s3_core::ops::LOG_MAX, "{}", d.log.len());
    assert_eq!(d.value, 0);
}

#[test]
fn datums_round_trip_through_json_and_text() {
    let (mut w, id) = cube();
    op(&mut w, &format!(r#"{{"op":"tile","id":{},"face":"-x","u":1,"v":0}}"#, id));
    op(&mut w, &format!(r#"{{"op":"transform","id":{},"parts":{{"sy":3,"rz":15}}}}"#, id));
    op(&mut w, r#"{"op":"datum","cell":[5,2,20],"name":"wall","value":12345}"#);
    let j = World::from_json(&J::parse(&w.to_json().dump()).unwrap()).unwrap();
    assert_eq!(j.datums, w.datums);
    let t = world_text(&w);
    assert!(t.contains("'c1' 3 1 20 "), "{}", t);
    assert!(t.contains("'c1' 'sy' 3 part.") && t.contains("'c1' 'rz' 15 part."), "{}", t);
    let back = world_from_text(&t).unwrap_or_else(|e| panic!("{}\n{}", e, t));
    let a: Vec<Datum> = w.datums.values().cloned().map(|mut d| { d.id = 0; d }).collect();
    let b: Vec<Datum> = back.datums.values().cloned().map(|mut d| { d.id = 0; d }).collect();
    assert_eq!(a, b);
    assert_eq!(world_text(&back), t);
}

#[test]
fn a_live_edit_seeds_what_the_player_changed() {
    let (mut w, id) = cube();
    op(&mut w, r#"{"op":"datum","cell":[5,1,20],"name":"keep","value":7}"#);
    main_text(&mut w, "99 keep#");
    let mut s = Session::new(build(&w).unwrap(), &P);
    s.run_to_end(u64::MAX);
    assert_eq!(s.peek("main", "keep").1, Some(99.0));
    op(&mut w, &format!(r#"{{"op":"tile","id":{},"face":"+z","u":0,"v":0}}"#, id));
    s.patch(build(&w).unwrap(), s3_core::build::defs(&w)).unwrap();
    s.tick();
    s.run_to_end(u64::MAX);
    assert_eq!(s.peek("main", "c1").1, Some(128.0), "the toggled cube takes the player's value");
    assert_eq!(s.peek("main", "keep").1, Some(99.0), "an untouched cube keeps what code wrote");
}

#[test]
fn datum_library_words() {
    let mut w = World::starter("lib");
    main_text(&mut w, "std/datum.s.\n'r' 16753920 red >\n'g' 16753920 green >\n'b' 16753920 blue >\n'v' 255 165 0 rgb >\n'bit' 16753920 23 bitof 16753920 0 bitof >\n'flip' 0 23 flip 8388608 23 flip >");
    let (out, _) = run(&w);
    assert!(out.contains("r 255"), "{:?}", out);
    assert!(out.contains("g 165"), "{:?}", out);
    assert!(out.contains("b 0"), "{:?}", out);
    assert!(out.contains("v 16753920"), "{:?}", out);
    assert!(out.contains("bit 1 0"), "{:?}", out);
    assert!(out.contains("flip 8388608 0"), "{:?}", out);
}
