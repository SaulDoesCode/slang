use s3_core::build::build;
use s3_core::compile::{compile_spec, compile_with};
use s3_core::json::J;
use s3_core::ops::import_text;
use s3_core::rt::{s_main, Entry, Program};
use s3_core::session::Spec;
use s3_core::world::World;
use std::path::Path;
use std::process::{exit, Command};

static INTERP: Program = Program::with_lib(s3_core::LIB);

const USAGE: &str = "usage:
  s3 file.s [args...]                     run an s program
  s3 run file.s|world.json [args...] [--ticks n] [--lockstep] [--one-thread] [--net-here]
                                          run a program or a world headless; thread and
                                          rpc zones get threads, net zones connect to hosts
  s3 serve addr world.json                host the net zones of a world for programs
                                          that run it elsewhere
  s3 rs file.s|world.json [-o out.rs] [-e file]... [-n native.rs] [--no-fold]
                                          emit one Rust program; what the program does
                                          before it reaches outside (input, files, shell,
                                          threads, network, randomness, the arguments) is
                                          done at build time, unless --no-fold
  s3 build file.s|world.json [-o bin] [-e file]... [-n native.rs] [--keep] [--no-fold]
                                          emit and compile with rustc -O
  s3 import file.s [-o world.json]        an s program as a world
  s3 text world.json [-o w.world.s]       the whole world as s text (plates, spheres, devices,
                                          wires, zones, assemblies); runs and builds as a world
  s3 json w.world.s [-o world.json]       a world text as world JSON
  s3 export world.json [-o dir]           a world as s text, one file per zone
  s3 version";

fn die(m: &str) -> ! {
    eprintln!("s3: {}", m);
    exit(2)
}

fn read(p: &str) -> String {
    match std::fs::read(p) {
        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
        Err(e) => die(&format!("{}: {}", p, e)),
    }
}

struct Opts {
    file: String,
    out: Option<String>,
    embeds: Vec<(String, String)>,
    native: Option<String>,
    keep: bool,
    fold: bool,
}

fn opts(a: &[String]) -> Opts {
    let mut o = Opts { file: String::new(), out: None, embeds: Vec::new(), native: None, keep: false, fold: true };
    let mut i = 0;
    while i < a.len() {
        match a[i].as_str() {
            "-o" => {
                i += 1;
                o.out = Some(a.get(i).cloned().unwrap_or_else(|| die("-o needs a path")));
            }
            "-e" => {
                i += 1;
                let p = a.get(i).cloned().unwrap_or_else(|| die("-e needs a path"));
                let t = read(&p);
                o.embeds.push((p, t));
            }
            "-n" | "--native" => {
                i += 1;
                let p = a.get(i).cloned().unwrap_or_else(|| die("-n needs a path"));
                o.native = Some(read(&p));
            }
            "--keep" => o.keep = true,
            "--no-fold" => o.fold = false,
            f if o.file.is_empty() => o.file = f.to_string(),
            f => die(&format!("unexpected argument '{}'\n{}", f, USAGE)),
        }
        i += 1;
    }
    if o.file.is_empty() {
        die(USAGE)
    }
    o
}

fn load_world(p: &str) -> World {
    if p.ends_with(".world.s") {
        return s3_core::textform::world_from_text(&read(p)).unwrap_or_else(|e| die(&format!("{}: {}", p, e)));
    }
    let j = J::parse(&read(p)).unwrap_or_else(|e| die(&format!("{}: {}", p, e)));
    World::from_json(&j).unwrap_or_else(|e| die(&format!("{}: {}", p, e)))
}

fn spec_of(w: &World) -> Spec {
    match build(w) {
        Ok(s) => s,
        Err(ps) => die(&ps.iter().map(|p| p.msg.clone()).collect::<Vec<_>>().join("\n")),
    }
}

fn is_world(p: &str) -> bool {
    p.ends_with(".json") || p.ends_with(".world.s")
}

fn emit(o: &Opts) -> String {
    if is_world(&o.file) {
        let w = load_world(&o.file);
        let spec = spec_of(&w);
        let units: Vec<String> = spec.units.iter().map(|u| u.text.clone()).collect();
        let main = spec.units[spec.zones[0].unit].text.clone();
        return compile_spec(&spec.to_json().dump(), &units, &main, s3_core::LIB, o.native.as_deref()).unwrap_or_else(|e| die(&e));
    }
    let src = read(&o.file);
    let (rs, note) = compile_with((&o.file, &src), &o.embeds, s3_core::LIB, o.native.as_deref(), o.fold).unwrap_or_else(|e| die(&e));
    if !note.is_empty() {
        eprintln!("s3: {}", note);
    }
    rs
}

fn stem(p: &str) -> String {
    Path::new(p).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "out".into())
}

fn run_world(path: &str, rest: &[String]) -> ! {
    let w = load_world(path);
    let spec = spec_of(&w);
    if let Some(d) = Path::new(path).parent() {
        let _ = s3_core::rt::BASE.set(d.to_path_buf());
    }
    s3_core::rt::CLI.store(true, std::sync::atomic::Ordering::Relaxed);
    let args = rest.to_vec();
    let h = std::thread::Builder::new()
        .stack_size(s3_core::rt::MAIN_STACK)
        .spawn(move || s3_core::part::world_main(spec, &INTERP, args, "s3"))
        .expect("spawn");
    exit(h.join().unwrap_or(101))
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(|s| s.as_str()) {
        None => s_main(&INTERP, Entry::File("./main.s".into()), Vec::new()),
        Some("-h") | Some("--help") | Some("help") => {
            println!("{}", USAGE);
        }
        Some("version") | Some("-V") | Some("--version") => {
            println!("s3 {}", env!("CARGO_PKG_VERSION"));
        }
        Some("run") | Some("--src") => {
            let f = a.get(1).cloned().unwrap_or_else(|| die(USAGE));
            if is_world(&f) {
                run_world(&f, &a[2..])
            }
            s_main(&INTERP, Entry::File(f), a[2..].to_vec())
        }
        Some("text") => {
            let o = opts(&a[1..]);
            let w = load_world(&o.file);
            let t = s3_core::textform::world_text(&w);
            match &o.out {
                Some(p) => std::fs::write(p, t).unwrap_or_else(|e| die(&format!("{}: {}", p, e))),
                None => print!("{}", t),
            }
        }
        Some("json") => {
            let o = opts(&a[1..]);
            let w = load_world(&o.file);
            let t = w.to_json().pretty();
            match &o.out {
                Some(p) => std::fs::write(p, t).unwrap_or_else(|e| die(&format!("{}: {}", p, e))),
                None => println!("{}", t),
            }
        }
        Some("serve") => {
            let (addr, f) = match (a.get(1), a.get(2)) {
                (Some(x), Some(f)) => (x.clone(), f.clone()),
                _ => die(USAGE),
            };
            let mut rest = vec!["--serve".to_string(), addr];
            rest.extend(a[3..].iter().cloned());
            run_world(&f, &rest)
        }
        Some("rs") => {
            let o = opts(&a[1..]);
            let rs = emit(&o);
            match &o.out {
                Some(p) => std::fs::write(p, rs).unwrap_or_else(|e| die(&format!("{}: {}", p, e))),
                None => print!("{}", rs),
            }
        }
        Some("build") => {
            let o = opts(&a[1..]);
            let rs = emit(&o);
            let bin = o.out.clone().unwrap_or_else(|| stem(&o.file));
            let rsp = format!("{}.rs", bin);
            std::fs::write(&rsp, rs).unwrap_or_else(|e| die(&format!("{}: {}", rsp, e)));
            let st = Command::new("rustc")
                .args(["-O", "--edition", "2021", "--crate-name", "s_program", "-o", &bin, &rsp])
                .status()
                .unwrap_or_else(|e| die(&format!("rustc: {}", e)));
            if !o.keep {
                let _ = std::fs::remove_file(&rsp);
            }
            if !st.success() {
                die("rustc failed")
            }
        }
        Some("bend") => {
            let o = opts(&a[1..]);
            if o.native.is_some() {
                die("s3 bend: words written in Rust (-n) are not built here yet; s3 build takes them")
            }
            if o.file.ends_with(".json") {
                die("s3 bend: worlds are not built here yet; s3 build takes them")
            }
            let src = read(&o.file);
            let (text, effs) = s3_core::bend::build(&src, &o.file).unwrap_or_else(|e| die(&format!("s3 bend: {}", e)));
            let out = o.out.clone().unwrap_or_else(|| stem(&o.file));
            let bend = if out.ends_with(".bend") { out.clone() } else { format!("{}.bend", out.trim_end_matches(".c").trim_end_matches(".js").trim_end_matches(".mjs")) };
            std::fs::write(&bend, &text).unwrap_or_else(|e| die(&format!("{}: {}", bend, e)));
            let dir = Path::new(&bend).parent().map(|d| d.to_path_buf()).unwrap_or_default();
            for (n, t) in effs {
                let p = dir.join(n);
                std::fs::write(&p, t).unwrap_or_else(|e| die(&format!("{}: {}", p.display(), e)));
            }
            if out != bend {
                let cmd = std::env::var("S3_BEND").unwrap_or_else(|_| "bend".to_string());
                let mut c = if cmd.ends_with(".ts") {
                    let mut c = Command::new("bun");
                    c.arg(&cmd);
                    c
                } else {
                    Command::new(&cmd)
                };
                let st = c.arg(&bend).arg("-o").arg(&out).status().unwrap_or_else(|e| die(&format!("{}: {}", cmd, e)));
                if !st.success() {
                    die(&format!("bend failed on {}", bend))
                }
            }
        }
        Some("native") => {
            let o = opts(&a[1..]);
            // what the machine-free build cannot write is built with the machine in it, as
            // s3 build builds it: every program builds, and says which way it was built
            let with_machine = |why: &str| -> ! {
                eprintln!("s3 native: {}; built with the machine in it, as s3 build does", why);
                let me = std::env::current_exe().unwrap_or_else(|e| die(&e.to_string()));
                let out = o.out.clone().unwrap_or_default();
                let how = if out.ends_with(".rs") { "rs" } else { "build" };
                let st = Command::new(me).arg(how).args(&a[1..]).status().unwrap_or_else(|e| die(&e.to_string()));
                exit(st.code().unwrap_or(1))
            };
            if o.native.is_some() {
                with_machine("words written in Rust (-n) run on the machine")
            }
            if o.file.ends_with(".json") {
                with_machine("a world runs on the machine")
            }
            let src = read(&o.file);
            let text = match s3_core::bend::build_rs(&src, &o.file) {
                Ok(t) => t,
                Err(e) => with_machine(&e),
            };
            let out = o.out.clone().unwrap_or_else(|| stem(&o.file));
            let rs = if out.ends_with(".rs") { out.clone() } else { format!("{}.rs", out) };
            std::fs::write(&rs, &text).unwrap_or_else(|e| die(&format!("{}: {}", rs, e)));
            if out != rs {
                let rustc = std::env::var("S3_RUSTC").unwrap_or_else(|_| "rustc".to_string());
                let st = Command::new(&rustc)
                    .args(["--edition", "2021", "--crate-name", "s_native", "-O", "-C", "codegen-units=1", "-C", "panic=abort", "-o"])
                    .arg(&out)
                    .arg(&rs)
                    .status()
                    .unwrap_or_else(|e| die(&format!("{}: {}", rustc, e)));
                if !st.success() {
                    die(&format!("rustc failed on {}", rs))
                }
            }
        }
        Some("import") => {
            let o = opts(&a[1..]);
            let w = import_text(&stem(&o.file), &read(&o.file)).unwrap_or_else(|e| die(&e));
            let out = o.out.clone().unwrap_or_else(|| format!("{}.json", stem(&o.file)));
            std::fs::write(&out, w.to_json().pretty()).unwrap_or_else(|e| die(&format!("{}: {}", out, e)));
        }
        Some("export") => {
            let o = opts(&a[1..]);
            let w = load_world(&o.file);
            let spec = spec_of(&w);
            let dir = o.out.clone().unwrap_or_else(|| ".".into());
            let _ = std::fs::create_dir_all(&dir);
            let base = stem(&o.file);
            for z in &spec.zones {
                let name = if z.name == "main" { format!("{}.s", base) } else { format!("{}.{}.s", base, z.name) };
                let p = Path::new(&dir).join(name);
                std::fs::write(&p, &spec.units[z.unit].text).unwrap_or_else(|e| die(&format!("{}: {}", p.display(), e)));
            }
            let g = J::Arr(spec.zones.iter().map(|z| z.graph.clone().with("zone", z.name.clone().into())).collect());
            let p = Path::new(&dir).join(format!("{}.graph.json", base));
            std::fs::write(&p, g.pretty()).unwrap_or_else(|e| die(&format!("{}: {}", p.display(), e)));
        }
        Some(_) => {
            if is_world(&a[0]) {
                run_world(&a[0].clone(), &a[1..])
            }
            s_main(&INTERP, Entry::File(a[0].clone()), a[1..].to_vec())
        }
    }
}
