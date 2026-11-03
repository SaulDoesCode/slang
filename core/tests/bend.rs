// s → Bend: the input programs of tests/bend and the blodra walks over stdin, built with
// `s3 bend` and run against the machine. Skipped when no `bend` is on the PATH.
use std::path::Path;
use std::process::Command;

fn have_bend() -> bool {
    Command::new("sh").arg("-c").arg("command -v bend").output().map(|o| o.status.success()).unwrap_or(false)
}

fn s3() -> String {
    format!("{}/../target/release/s3", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn input_programs_match_the_machine() {
    if !have_bend() || !Path::new(&s3()).exists() {
        eprintln!("skipped: needs bend on the PATH and a release build of s3");
        return;
    }
    let dir = format!("{}/tests/bend", env!("CARGO_MANIFEST_DIR"));
    let o = Command::new("sh").arg(format!("{}/check.sh", dir)).env("S3", s3()).output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.lines().last().map_or(false, |l| l.ends_with(", 0 not")), "{}", out);
}

#[test]
fn blodra_walks_from_stdin_match_the_js() {
    if !have_bend() || !Path::new(&s3()).exists() {
        eprintln!("skipped: needs bend on the PATH and a release build of s3");
        return;
    }
    let here = env!("CARGO_MANIFEST_DIR");
    let tmp = std::env::temp_dir().join("s3-bend-blodra");
    std::fs::create_dir_all(&tmp).unwrap();
    let bin = tmp.join("blodra_in");
    let st = Command::new(s3()).current_dir(format!("{}/..", here)).args(["bend", "examples/blodra_in.s", "-o"]).arg(&bin).status().unwrap();
    assert!(st.success());
    let walks = std::fs::File::open(format!("{}/tests/bend/blodra_in.walks", here)).unwrap();
    let o = Command::new(&bin).stdin(walks).output().unwrap();
    let got: Vec<String> = String::from_utf8_lossy(&o.stdout).lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    let want: Vec<String> = std::fs::read_to_string(format!("{}/tests/blodra.expect", here)).unwrap().lines().map(|l| l.to_string()).collect();
    assert_eq!(got, want);
}
