// cargo test --release --test bench_lanes -- --ignored --nocapture: one fork over ten million
// numbers, lane by lane, with the machine's SIMD kernels, and as a compiled kernel.
use s3_core::rt::{List, Out, Program, Rt};
use std::sync::{Arc, Mutex};
use std::time::Instant;

static P: Program = Program::with_lib(s3_core::LIB);
const N: usize = 10_000_000;
const FORK: &str = "⟪xs…⟫ 2 \\* =~ 1 \\+ =~ 3 \\/ =~ ⨝ ys into";

fn timed(fork: &str) -> (f64, String) {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let mut rt = Rt::with_out(&P, Out::Buf(buf));
    let k = rt.it.sym("xs");
    rt.set_list(k, List::nums_of((0..N).map(|i| i as f64).collect()));
    rt.fork_log = Some(Vec::new());
    let t = Instant::now();
    rt.run_text(fork);
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    (ms, rt.fork_log.take().unwrap().remove(0).how)
}

#[test]
#[ignore]
fn ten_million_lanes() {
    let (a, ha) = timed(FORK);
    let (b, hb) = timed(&FORK.replace(" ⨝", " 0 \\( \\) ⨝"));
    println!("{:>9.1} ms  {}", a, ha);
    let (c, _) = timed(&FORK.replace(" ys into", ""));
    println!("{:>9.1} ms  the same without into", c);
    println!("{:>9.1} ms  {}", b, hb);
    // compiled: the binary reports the time of its own fork
    let dir = std::env::temp_dir().join(format!("s3-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = format!("{{n~ xs l}} fill$ {} fill &&& {} ys \\#l =~ >", N, FORK);
    let rs = s3_core::compile::compile(("b.s", &src), &[], s3_core::LIB, None).unwrap();
    let p = dir.join("b.rs");
    std::fs::write(&p, rs).unwrap();
    let bin = dir.join("b");
    let o = std::process::Command::new("rustc").args(["-O", "--edition", "2021", "-o"]).arg(&bin).arg(&p).output().unwrap();
    assert!(o.status.success());
    for _ in 0..3 {
        let o = std::process::Command::new(&bin).env("S_FORKS", "1").output().unwrap();
        let e = String::from_utf8_lossy(&o.stderr);
        println!("{:>9}  {}", "", e.trim().rsplit('\t').next().unwrap_or(""));
    }
}
