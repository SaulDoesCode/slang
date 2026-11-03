// s → Rust: the input programs of tests/bend and the blodra walks over stdin, built with
// `s3 native` and run against the machine.
use std::process::Command;

fn s3() -> String {
    format!("{}/../target/release/s3", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn native_input_programs_match_the_machine() {
    if !std::path::Path::new(&s3()).exists() {
        eprintln!("skipped: needs a release build of s3");
        return;
    }
    let dir = format!("{}/tests/bend", env!("CARGO_MANIFEST_DIR"));
    let o = Command::new("sh").arg(format!("{}/check_native.sh", dir)).env("S3", s3()).output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.lines().last().map_or(false, |l| l.ends_with(", 0 not")), "{}", out);
}

#[test]
fn native_blodra_walks_match_the_js() {
    if !std::path::Path::new(&s3()).exists() {
        eprintln!("skipped: needs a release build of s3");
        return;
    }
    let here = env!("CARGO_MANIFEST_DIR");
    let tmp = std::env::temp_dir().join("s3-native-blodra");
    std::fs::create_dir_all(&tmp).unwrap();
    let bin = tmp.join("blodra_in");
    let st = Command::new(s3()).current_dir(format!("{}/..", here)).args(["native", "examples/blodra_in.s", "-o"]).arg(&bin).status().unwrap();
    assert!(st.success());
    let walks = std::fs::File::open(format!("{}/tests/bend/blodra_in.walks", here)).unwrap();
    let o = Command::new(&bin).stdin(walks).output().unwrap();
    let got: Vec<String> = String::from_utf8_lossy(&o.stdout).lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    let want: Vec<String> = std::fs::read_to_string(format!("{}/tests/blodra.expect", here)).unwrap().lines().map(|l| l.to_string()).collect();
    assert_eq!(got, want);
}

#[test]
fn a_fork_over_numbers_is_a_vector_kernel() {
    if !std::path::Path::new(&s3()).exists() {
        eprintln!("skipped: needs a release build of s3");
        return;
    }
    let here = env!("CARGO_MANIFEST_DIR");
    let out = std::env::temp_dir().join("s3-native-lanes.rs");
    let st = Command::new(s3()).args(["native", &format!("{}/tests/bend/lanes_num.s", here), "-o"]).arg(&out).status().unwrap();
    assert!(st.success());
    let rs = std::fs::read_to_string(&out).unwrap();
    assert!(rs.contains("fn kb0(xs: &[f64]"), "the fork is a kernel over a column of numbers");
    assert!(rs.contains("#[target_feature(enable = \"avx512f\")]"));
    assert!(rs.contains("is_x86_feature_detected!(\"avx2\")"));
}
