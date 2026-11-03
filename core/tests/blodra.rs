// examples/blodra.s, the cube journey in s, against what the JS blodra gives for 300 walks.
use s3_core::rt::*;
use std::sync::{Arc, Mutex};

static P: Program = Program::with_lib(s3_core::LIB);

#[test]
fn blodra_walks_match_the_js() {
    let here = env!("CARGO_MANIFEST_DIR");
    let cube = std::fs::read_to_string(format!("{}/../examples/blodra.s", here)).unwrap();
    let walks = std::fs::read_to_string(format!("{}/tests/blodra.walks", here)).unwrap();
    let want = std::fs::read_to_string(format!("{}/tests/blodra.expect", here)).unwrap();
    let buf = Arc::new(Mutex::new(Vec::new()));
    let mut rt = Rt::with_out(&P, Out::Buf(buf.clone()));
    let st = rt.run_text(&format!("{}\n{}", cube, walks));
    rt.flush();
    assert!(!matches!(st, Status::Error(_)), "{:?}", match st { Status::Error(e) => e, _ => String::new() });
    let out = String::from_utf8_lossy(&buf.lock().unwrap()).into_owned();
    let got: Vec<&str> = out.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    let want: Vec<&str> = want.lines().collect();
    assert_eq!(got.len(), want.len());
    for (i, (g, w)) in got.iter().zip(&want).enumerate() {
        assert_eq!(g, w, "walk {}", i + 1);
    }
}
