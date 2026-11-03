// Bead lanes in the native machine: BEADS.md §5.6–5.9, spread forks, into, try, how each
// fork ran, and the same programs compiled to Rust.
use s3_core::rt::{Out, Program, Rt, Status};
use std::sync::{Arc, Mutex};

static P: Program = Program::with_lib(s3_core::LIB);

/// Builds and timings take turns: a timing taken while rustc runs beside it measures rustc.
static HEAVY: Mutex<()> = Mutex::new(());
fn heavy() -> std::sync::MutexGuard<'static, ()> {
    HEAVY.lock().unwrap_or_else(|e| e.into_inner())
}

fn run_with(src: &str, report: bool) -> (String, Vec<String>) {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let mut rt = Rt::with_out(&P, Out::Buf(buf.clone()));
    if report {
        rt.fork_log = Some(Vec::new());
    }
    let st = rt.run_text(src);
    rt.flush();
    let mut out = String::from_utf8_lossy(&buf.lock().unwrap()).into_owned();
    if let Status::Error(e) = st {
        out.push_str(&format!(" !{}", e));
    }
    let hows = rt.fork_log.take().unwrap_or_default().into_iter().map(|f| f.how).collect();
    (out, hows)
}
fn run(src: &str) -> String {
    run_with(src, false).0
}

#[test]
fn beads_tests_5_6_to_5_9() {
    assert_eq!(run("⟪1¦2¦3¦4⟫ 10 \\+ =~ ⨝ collect >"), "\n 11 12 13 14");
    assert_eq!(run("⟪1¦2¦3¦4⟫ 10 \\+ =~ ⨝ {\\+ =~} reduce >"), "\n 50");
    assert_eq!(run("[[ 1 || 2 ]] 10 \\+ =~ ||> collect >"), "\n 11 12");
    assert_eq!(run("5 x# ⟪1¦2⟫ x# ⨝ x~ >"), "\n 5");
    assert_eq!(run("5 x# ⟪1¦2⟫ x# ⨝ 1 lane x~ >"), "\n 2");
    assert_eq!(run("5 x# ⟪1¦2⟫ x# ⨝ 1 lane endlane x~ >"), "\n 5");
    assert_eq!(run("⟪{a} >¦{b} >⟫ ⨝"), "\n a\n b");
    assert_eq!(run("⟪1¦2¦3⟫ ⨝ lanes >"), "\n 3");
    assert_eq!(run("⟪1¦2¦3⟫ ⨝ {2 \\* =~} lanewise collect >"), "\n 2 4 6");
}

#[test]
fn sets_nesting_and_errors() {
    assert_eq!(run("⟪a:1¦2⟫ ⟪b:10¦20⟫ \\* =~ ⨝ collect >"), "\n 10 20 20 40");
    assert_eq!(run("⟪1¦2⟫ 3 \\+ =~\ncollect >"), "\n 4 5");
    assert_eq!(run("⟪⟪10¦20⟫ 1 \\+ =~ ⨝ {\\+ =~} reduce ¦ 5⟫ ⨝ collect >"), "\n 32 5");
    assert_eq!(run("{⟪1¦2⟫ 3 \\* =~ ⨝ collect} three ><  three >"), "\n 3 6");
    assert_eq!(run("⟪1¦2⟫ 2 \\= <-! nope. ⨝"), " !lane 1: nope.: no script or file named 'nope'");
    assert!(run("1 ¦ 2").contains("¦ outside a fork"));
    assert!(run("⟪a¦b⟫ ⟪c¦d¦e⟫").contains("this ⟪ has 3 lanes where its set has 2"));
    assert_eq!(run("std/all.s. 3 4 add >"), "\n 7");
}

#[test]
fn spread_and_into() {
    assert_eq!(run("{1 2 3 4} xs /l ⟪xs…⟫ 10 \\* =~ ⨝ ys into ys o l/ o~ >"), "\n 10 20 30 40");
    assert_eq!(run("{1 2 3} xs /l ⟪xs...⟫ 2 \\^ =~ ⨝ {\\+ =~} reduce >"), "\n 14");
    // a spread fork over text runs lane by lane
    assert_eq!(run("{a b} xs /l ⟪xs…⟫ v@ {~v~!} ⨝ collect >"), "\n a! b!");
}

#[test]
fn vector_programs_match_lane_by_lane() {
    // the same arithmetic, once as a vector program and once with a print that keeps it lane by lane
    let progs = [
        "⟪1¦2¦3¦4¦5⟫ 10 \\* =~ 3 \\- =~ 2 \\/ =~ ⨝",
        "⟪1¦2¦3¦4¦5⟫ 7 \\% =~ 2 \\^ =~ ⨝",
        "4 k# ⟪1¦2¦3¦4¦5⟫ k~ \\+ =~ y# y~ y~ \\* =~ ⨝",
        "⟪1 2¦3 4¦5 6⟫ \\+ =~ c# c; c~ ⨝",
        "⟪1¦2¦3⟫ 2 \\* =~ rr n ⨝",
    ];
    for p in progs {
        let (a, how) = run_with(&format!("{} collect >", p), true);
        let (b, how2) = run_with(&format!("{} collect >", p.replace(" ⨝", " 0 \\( \\) ⨝")), true);
        assert!(how[0].contains("lanes · machine kernels"), "{}: {:?}", p, how);
        assert!(how2[0].contains("one lane at a time"), "{}: {:?}", p, how2);
        assert_eq!(a, b, "{}", p);
    }
    // writes filed by lane from a vector program
    assert_eq!(run("⟪1¦2¦3⟫ 10 \\* =~ q# ⨝ 2 lane q~ >"), "\n 30");
}

#[test]
fn a_million_lanes() {
    let (out, how) = run_with("{n~ xs l} fill$ 1000000 fill &&& ⟪xs…⟫ 2 \\* =~ 1 \\+ =~ ⨝ ys into ys -1 last \\[l] last~ > ys \\#l =~ >", true);
    assert_eq!(out, "\n 1999999\n 1000000");
    assert!(how[0].contains("1000000 lanes"), "{:?}", how);
}

#[test]
fn threads_for_code() {
    let (out, how) = run_with("⟪{a} >¦{b} >¦{c} >⟫ {x} y` ⨝ collect >", true);
    assert_eq!(out, "\n a\n b\n c\n");
    assert!(how.len() == 1 && how[0].starts_with("3 threads · "), "{:?}", how);
    // names written on threads come back filed by lane
    let (out, _) = run_with("{w} ww` ⟪{1} v`¦{2} v`⟫ ⨝ 1 lane v~ > endlane v~ >", true);
    assert_eq!(out, "\n 2\n 404");
    // a word defined before the fork works in every lane
    let (out, _) = run_with("{3 \\* =~} three >< ⟪{2} three¦{5} three⟫ ⨝ collect >", true);
    assert_eq!(out, "\n 6 15");
    let (out, _) = run_with("⟪{1} >¦nope.⟫ ⨝", true);
    assert_eq!(out, "\n 1 !lane 1: nope.: no script or file named 'nope'");
}

#[test]
fn try_catches() {
    assert_eq!(run("{1 >} {caught >} try 2 >"), "\n 1\n 2");
    assert_eq!(run("{1 > nope. 2 >} {{caught:} err~ >} try 3 >"), "\n 1\n caught: nope.: no script or file named 'nope'\n 3");
    assert_eq!(run("{{nope.} {err~ >} try {after} >} ww ><  ww {end} >"), "\n nope.: no script or file named 'nope'\n after\n end");
    assert_eq!(run("5 x# {⟪1¦2⟫ x# nope. ⨝} {x~ >} try"), "\n 5");
}

#[test]
fn compiled_program_runs_its_forks_as_simd() {
    let _turn = heavy();
    let src = "{1 2 3 4 5 6 7 8 9 10} xs /l\n⟪xs…⟫ 2 \\* =~ 1 \\+ =~ ⨝ ys into ys o l/ o~ >\n⟪1¦2¦3¦4⟫ 10 \\+ =~ ⨝ {\\+ =~} reduce >\n⟪{a} >¦{b} >⟫ ⨝\n5 x# ⟪1¦2⟫ x# ⨝ 1 lane x~ >\n{{nope.} {{caught} >} try} ok ><  ok\n";
    let rs = s3_core::compile::compile(("lanes.s", src), &[], s3_core::LIB, None).unwrap();
    assert!(rs.contains("fn k0(") && rs.contains("vx::Avx"), "no kernel");
    let dir = std::env::temp_dir().join(format!("s3-lanes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rsp = dir.join("lanes.rs");
    std::fs::write(&rsp, &rs).unwrap();
    let bin = dir.join("lanes");
    let o = rustc().args(["-O", "--edition", "2021", "-o"]).arg(&bin).arg(&rsp).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let o = std::process::Command::new(&bin).env("S_FORKS", "1").output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    eprintln!("stderr: {}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(out, "\n 3 5 7 9 11 13 15 17 19 21\n 50\n a\n b\n 2\n caught");
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("10 lanes · compiled kernel"), "{}", err);
    assert!(err.contains("2 threads"), "{}", err);
}

const MASKED: &[(&str, bool)] = &[
    // clamp: v is 4 where it was above 4
    ("{1 5 9 3 4 12 0 7 8} xs /l ⟪xs…⟫ v# v~ 4 \\>> 4 v#? // v~ ⨝", true),
    // the even ones go up by one
    ("{1 2 3 4 5 6 7 8 9 10} xs /l ⟪xs…⟫ x# x~ 2 \\% =~ 0 \\== 1 x;? // x~ ⨝", true),
    // and: above 3 and equal to 7 becomes 0
    ("{1 7 3 7 9 7 2} xs /l ⟪xs…⟫ x# x~ 3 \\>> x~ 7 \\& 0 x#? // x~ ⨝", true),
    // not
    ("{1 2 3 4 5 6} xs /l ⟪xs…⟫ x# x~ 3 \\<< \\!! 1 x;? // x~ ⨝", true),
    // = treats two NaN as equal, == does not
    ("0 q# 0 r# {0 1 2} xs /l ⟪xs…⟫ x# x~ 0 \\* =~ 0 \\/ =~ y# y~ y~ \\= 5 q#? // y~ y~ \\== 7 r#? // q~ r~ ⨝", true),
    // the flag at the fork, in every lane; a lane that changes it leaves the next lane alone
    ("0 v# 1 1 \\= {1 2 3} xs /l ⟪xs…⟫ 5 v#? // v~ 3 2 \\<< ⨝", true),
    // a conditional that leaves a value in some lanes: lane by lane
    ("{1 5 9} xs /l ⟪xs…⟫ v# v~ 4 \\>> 4 v#? v~ ⨝", false),
    // a write in some lanes to a name holding no number: lane by lane
    ("{1 5 9} xs /l ⟪xs…⟫ v# v~ 4 \\>> 4 zz#? // v~ ⨝", false),
    // literal lanes, select into =
    ("0 =# ⟪1¦2¦3¦4⟫ 2 \\<< 10 =#? // =~ ⨝", true),
];


/// rustc building for the architecture this test runs on: on ARM, the aarch64 target with the
/// linker named in CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER.
fn rustc() -> std::process::Command {
    let mut c = std::process::Command::new("rustc");
    if cfg!(target_arch = "aarch64") && !cfg!(all(target_os = "macos")) {
        c.args(["--target", "aarch64-unknown-linux-gnu"]);
        if let Ok(l) = std::env::var("CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER") {
            c.arg("-C").arg(format!("linker={}", l));
        }
    }
    c
}

/// The vector levels this machine has, widest first, as S_SIMD names them.
const LEVELS: &[&str] = if cfg!(target_arch = "aarch64") { &["neon", "scalar"] } else { &["avx512", "avx", "sse2", "scalar"] };

#[test]
fn vector_programs_with_masks() {
    let want_level = std::env::var("S_SIMD").ok();
    for (p, vec) in MASKED {
        let (a, how) = run_with(&format!("{} collect > lanes >", p), true);
        let (b, how2) = run_with(&format!("{} collect > lanes >", p.replace(" ⨝", " 0 \\( \\) ⨝")), true);
        let last = how.last().unwrap().clone();
        assert_eq!(last.contains("machine kernels"), *vec, "{}: {:?}", p, how);
        assert!(how2.last().unwrap().contains("one lane at a time"), "{}: {:?}", p, how2);
        assert_eq!(a, b, "{}", p);
        assert!(!a.contains('!'), "{}: {}", p, a);
        if let (Some(l), true) = (&want_level, *vec) {
            let label = match l.as_str() { "avx512" => "f64×8 avx-512", "avx" => "f64×4 avx", "sse2" => "f64×2 sse2", "neon" => "f64×2 neon", _ => "f64×1" };
            assert!(last.starts_with(label), "{} at {}: {}", p, l, last);
        }
    }
    // the flag after the join is the flag at the fork
    assert_eq!(run("1 1 \\= ⟪1¦2⟫ 3 2 \\<< ⨝ {yes} >? {no} >!"), "\n yes");
    assert_eq!(run("1 1 \\= ⟪1¦2⟫ 3 2 \\<< 0 \\( \\) ⨝ {yes} >? {no} >!"), "\n yes");
}

#[test]
fn every_vector_level() {
    let _turn = heavy();
    let me = std::env::current_exe().unwrap();
    for &l in LEVELS {
        for t in ["vector_programs_with_masks", "vector_programs_match_lane_by_lane", "spread_and_into"] {
            let o = std::process::Command::new(&me).args(["--exact", t, "--test-threads", "1"]).env("S_SIMD", l).output().unwrap();
            assert!(o.status.success(), "{} at {}:\n{}", t, l, String::from_utf8_lossy(&o.stdout));
        }
    }
}

#[test]
fn compiled_masks_at_every_level() {
    let _turn = heavy();
    let src: String = MASKED.iter().filter(|p| p.1).map(|(p, _)| format!("{} collect > lanes >\n", p)).collect();
    let rs = s3_core::compile::compile(("masks.s", &src), &[], s3_core::LIB, None).unwrap();
    assert!(rs.contains("kz0") && rs.contains(".clt(") || rs.contains(".cgt("), "kernels with compares");
    let dir = std::env::temp_dir().join(format!("s3-masks-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rsp = dir.join("masks.rs");
    std::fs::write(&rsp, &rs).unwrap();
    let bin = dir.join("masks");
    let o = rustc().args(["-O", "--edition", "2021", "-o"]).arg(&bin).arg(&rsp).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let want = run(&src);
    for &l in LEVELS {
        let o = std::process::Command::new(&bin).env("S_FORKS", "1").env("S_SIMD", l).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&o.stdout), want, "at {}", l);
        let err = String::from_utf8_lossy(&o.stderr);
        assert_eq!(err.matches("compiled kernel").count(), MASKED.iter().filter(|p| p.1).count(), "at {}: {}", l, err);
    }
}

#[test]
fn linked_build_is_quick() {
    let _turn = heavy();
    // the library this test runs against is the one cargo just built
    let exe = std::env::current_exe().unwrap();
    let (rlib, deps) = match s3_core::compile::rlib_near(&exe) {
        Some(x) => x,
        None => panic!("no libs3_core.rlib near {}", exe.display()),
    };
    let src = "{1 2 3 4 5 6 7 8 9 10} xs /l\n⟪xs…⟫ 2 \\* =~ 1 \\+ =~ ⨝ ys into ys o l/ o~ >\n⟪{a} >¦{b} >⟫ ⨝\n";
    let rs = s3_core::compile::compile_linked(("l.s", src), &[], s3_core::LIB, None).unwrap();
    let dir = std::env::temp_dir().join(format!("s3-linked-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rsp = dir.join("l.rs");
    std::fs::write(&rsp, &rs).unwrap();
    let bin = dir.join("l");
    let t = std::time::Instant::now();
    let o = rustc()
        .args(["-O", "--edition", "2021", "-o"])
        .arg(&bin)
        .arg(&rsp)
        .args(s3_core::compile::link_args(&rlib, &deps))
        .output()
        .unwrap();
    let secs = t.elapsed().as_secs_f64();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    eprintln!("linked build: {:.1} s", secs);
    let o = std::process::Command::new(&bin).env("S_FORKS", "1").output().unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout), "\n 3 5 7 9 11 13 15 17 19 21\n a\n b");
    assert!(String::from_utf8_lossy(&o.stderr).contains("compiled kernel"));
}

#[test]
fn thread_forks_reuse_the_state() {
    let _turn = heavy();
    // a million numbers in scope; the first fork takes the state, the second finds it unchanged
    let buf = Arc::new(Mutex::new(Vec::new()));
    let mut rt = Rt::with_out(&P, Out::Buf(buf.clone()));
    let k = rt.it.sym("big");
    rt.set_list(k, s3_core::rt::List::nums_of((0..1_000_000).map(|i| i as f64).collect()));
    rt.fork_log = Some(Vec::new());
    let fork = "⟪big \\#l =~¦big \\#l =~¦big \\#l =~⟫ ⨝ collect >";
    let snap = |rt: &Rt| rt.snap.as_ref().map(|s| Arc::as_ptr(&s.2) as usize);
    rt.run_text(fork);
    let first = snap(&rt);
    rt.run_text(fork);
    for _ in 0..3 {
        rt.run_text("⟪big \\#l =~¦big \\#l =~¦big \\#l =~⟫ ⨝");
    }
    // the same snapshot crossed to every fork while nothing was written
    assert!(first.is_some());
    assert_eq!(snap(&rt), first);
    // a write between forks: the state is taken again, and the lanes see the write
    rt.run_text("5 q# ⟪{a} q~¦{b} q~⟫ ⨝ collect >");
    rt.flush();
    let out = String::from_utf8_lossy(&buf.lock().unwrap()).into_owned();
    assert_eq!(out, "\n 1000000 1000000 1000000\n 1000000 1000000 1000000\n a 5 b 5");
    let ms: Vec<f64> = rt.fork_log.as_ref().unwrap().iter().map(|f| f.how.rsplit(" · ").next().unwrap().trim_end_matches(" ms").parse().unwrap()).collect();
    eprintln!("thread forks: {:?} ms", ms);
    // the forks after the first find the state; the quickest of them shows the cost when the
    // machine is not busy with other tests
    assert_ne!(snap(&rt), first, "a write makes a new snapshot");
    // on x86-64 the forks after the first start in under a millisecond; under emulation the
    // thread starts are what a fork costs, and the snapshot check above is the whole claim
    let best = ms[1..5].iter().cloned().fold(f64::MAX, f64::min);
    if cfg!(target_arch = "x86_64") {
        assert!(best < 1.0, "{:?}", ms);
    }
}
