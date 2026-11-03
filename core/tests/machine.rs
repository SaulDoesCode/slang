use s3_core::rt::*;
use std::sync::{Arc, Mutex};

static P: Program = Program::with_lib(s3_core::LIB);

fn rt() -> (Rt, Arc<Mutex<Vec<u8>>>) {
    let buf = Arc::new(Mutex::new(Vec::new()));
    (Rt::with_out(&P, Out::Buf(buf.clone())), buf)
}

fn text(b: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8(b.lock().unwrap().clone()).unwrap()
}

#[test]
fn budget_steps_one_op_at_a_time() {
    let (mut r, out) = rt();
    r.run_src("1 2 + =~ > {3 4 + =~ >} f$ f.");
    let mut steps = 0;
    loop {
        match r.run(1) {
            Status::Ran => steps += 1,
            Status::Done => break,
            _ => panic!("unexpected status"),
        }
    }
    assert_eq!(text(&out), "\n 3\n 7");
    assert!(steps >= 10, "steps {}", steps);
}

#[test]
fn errors_end_the_run_with_a_status() {
    let (mut r, _) = rt();
    r.run_src("1 x# {nope.} f$ f.");
    match r.run(u64::MAX) {
        Status::Error(e) => assert!(e.contains("no script or file named 'nope'"), "{}", e),
        _ => panic!("expected an error"),
    }
    assert!(r.fs.is_empty());
    match r.run_text("x~ >") {
        Status::Done => {}
        _ => panic!("machine should be usable after an error"),
    }
}

#[test]
fn exit_is_a_status() {
    let (mut r, _) = rt();
    match r.run_text("7 <--") {
        Status::Exit(7) => {}
        _ => panic!("expected exit 7"),
    }
}

#[test]
fn breakpoints_stop_before_the_op() {
    let (mut r, out) = rt();
    r.unit_ids.insert("1 > 2 > 3 >".to_string(), 5);
    r.run_src("1 > 2 > 3 >");
    r.breaks.insert((5u64 << 32) | 2);
    assert!(matches!(r.run(u64::MAX), Status::Break));
    assert_eq!(text(&out), "\n 1");
    assert!(matches!(r.run(u64::MAX), Status::Done));
    assert_eq!(text(&out), "\n 1\n 2\n 3");
}

#[test]
fn deep_non_tail_recursion_uses_no_host_stack() {
    let (mut r, out) = rt();
    let st = r.run_text("0 c# {c; c~ 300000 << down.? 1 c:} down$ down. c~ >");
    assert!(matches!(st, Status::Done));
    assert_eq!(text(&out), "\n 0");
}
