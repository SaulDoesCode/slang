// Folding: a program built with what it does before it reaches outside done at build time
// prints the same, ends the same, and reads input the same as the machine running it.
use s3_core::rt::*;
use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Arc, Mutex};

static P: Program = Program::with_lib(s3_core::LIB);

/// stdout, the error line, and the exit code of the machine running src.
fn machine(src: &str, args: &[&str], stdin: &str) -> (String, String, i32) {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let mut rt = Rt::with_out(&P, Out::Buf(buf.clone()));
    rt.set_argv(args.iter().map(|a| a.to_string()).collect());
    rt.input = Some(stdin.lines().map(|l| l.to_string()).collect::<VecDeque<_>>());
    let st = rt.run_text(src);
    rt.flush();
    let out = String::from_utf8_lossy(&buf.lock().unwrap()).into_owned();
    match st {
        Status::Error(e) => (out, format!("s: {}", e), 1),
        Status::Exit(c) => (out, String::new(), c),
        _ => (out, String::new(), 0),
    }
}

fn build(src: &str, fold: bool, tag: &str) -> (std::path::PathBuf, String) {
    let exe = std::env::current_exe().unwrap();
    let (rlib, deps) = s3_core::compile::rlib_near(&exe).expect("libs3_core.rlib");
    let (rs, note) = s3_core::compile::compile_linked_with(("fold.s", src), &[], s3_core::LIB, None, fold).unwrap();
    let dir = std::env::temp_dir().join(format!("s3-fold-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rsp = dir.join(format!("{}.rs", tag));
    std::fs::write(&rsp, &rs).unwrap();
    let bin = dir.join(tag);
    let o = std::process::Command::new("rustc")
        .args(["-O", "--edition", "2021", "-o"])
        .arg(&bin)
        .arg(&rsp)
        .args(s3_core::compile::link_args(&rlib, &deps))
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    (bin, note)
}

fn binary(bin: &std::path::Path, args: &[&str], stdin: &str) -> (String, String, i32) {
    let mut c = std::process::Command::new(bin).args(args).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();
    c.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    let o = c.wait_with_output().unwrap();
    let err = String::from_utf8_lossy(&o.stderr).lines().find(|l| l.starts_with("s: ")).unwrap_or("").to_string();
    (String::from_utf8_lossy(&o.stdout).into_owned(), err, o.status.code().unwrap_or(-1))
}

/// (program, args, stdin, what folding does: "whole", "line", or "none")
const CASES: &[(&str, &[&str], &str, &str)] = &[
    ("std/core.s. {2 * =~} double$ 0 i# {i~ ys l i;} step$ 10 step &&& ys o l/ o~ > 21 double.. >", &[], "", "whole"),
    ("std/core.s. {2 * =~} double$ 5 x# {a} >\nargs 0 a [l] a~ > x~ double.. >", &["hi"], "", "line"),
    ("{1 2 3} xs /l xs o l/ o~ >\n<_ _~ > xs #l =~ >", &[], "line one\n", "line"),
    ("{3 + =~} 'add3' >< 4 add3 >\n<_ _~ add3 >", &[], "5\n", "line"),
    ("{a} >\n{<_ _~ >} {caught} try >\n{b} >", &[], "x\n", "line"),
    ("1 >\nnope.", &[], "", "whole"),
    ("1 > 3 <--", &[], "", "whole"),
    ("⟪1¦2¦3⟫ 10 * =~ ⨝ collect >\n<_ _~ >", &[], "z\n", "line"),
    ("⟪1¦2⟫ x# ⨝\n<_ _~ > collect >", &[], "z\n", "none"),
    ("⟪{echo a} < r~ >¦{echo b} < r~ >⟫ ⨝", &[], "", "none"),
    ("{n~ big l} fill$ 100000 fill &&&\n<_ _~ > big #l =~ >", &[], "q\n", "line"),
    ("1 2 1 ( 7 x# )\n<_ _~ x~ >", &[], "w\n", "line"),
    ("{:a@ :a~ :a~ * =~} sq$ 6 sq.. y#\n7 #prng y~ >", &[], "", "line"),
];

#[test]
fn folded_programs_do_what_the_machine_does() {
    for (i, (src, args, stdin, kind)) in CASES.iter().enumerate() {
        let want = machine(src, args, stdin);
        let (b0, n0) = build(src, false, &format!("u{}", i));
        let (b1, n1) = build(src, true, &format!("f{}", i));
        assert_eq!(n0, "");
        let got_kind = if n1.starts_with("folded whole") { "whole" } else if n1.starts_with("folded to line") { "line" } else { "none" };
        assert_eq!(got_kind, *kind, "case {}: {} ({})", i, src, n1);
        assert_eq!(binary(&b0, args, stdin), want, "case {} unfolded: {}", i, src);
        assert_eq!(binary(&b1, args, stdin), want, "case {} folded: {} ({})", i, src, n1);
    }
}

#[test]
fn blodra_folds_whole() {
    let here = env!("CARGO_MANIFEST_DIR");
    let src = format!("{}\n{}", std::fs::read_to_string(format!("{}/../examples/blodra.s", here)).unwrap(), std::fs::read_to_string(format!("{}/tests/blodra.walks", here)).unwrap());
    let want = machine(&src, &[], "");
    let (b, note) = build(&src, true, "blodra");
    assert!(note.starts_with("folded whole"), "{}", note);
    assert_eq!(binary(&b, &[], ""), want);
}
