// The room's requests: s files under one root, runs that say which word printed what,
// tokens with their nesting, the layout beside a file, and the build to Rust. These run
// off the core thread, so a long program never holds up the worlds.
use crate::core::INTERP;
use s3_core::json::J;
use s3_core::rt::{Out, Rt, Status};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

pub static ROOT: OnceLock<PathBuf> = OnceLock::new();

pub const CMDS: &[&str] = &["ls", "read", "write", "srun", "lexdeep", "roomget", "roomput", "rust", "rustsrc", "lib", "build"];

fn err(m: impl Into<String>) -> J {
    J::obj().with("error", J::Str(m.into()))
}

fn root() -> PathBuf {
    ROOT.get().cloned().unwrap_or_else(|| PathBuf::from("."))
}

/// A path under the root, given relative with / between parts.
fn under(rel: &str) -> Result<PathBuf, J> {
    let p = Path::new(rel.trim());
    if rel.trim().is_empty() || p.is_absolute() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(err(format!("{}: a path under the folder, without ..", rel)));
    }
    Ok(root().join(p))
}

fn layout_path(rel: &str) -> Result<PathBuf, J> {
    let p = under(rel)?;
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(p.with_file_name(format!(".{}.room", name)))
}

fn walk(dir: &Path, rel: &str, out: &mut Vec<(String, u64)>, depth: usize) {
    if depth > 8 || out.len() > 4000 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut es: Vec<_> = rd.filter_map(|e| e.ok()).collect();
    es.sort_by_key(|e| e.file_name());
    for e in es {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        let r = if rel.is_empty() { name.clone() } else { format!("{}/{}", rel, name) };
        let p = e.path();
        if p.is_dir() {
            walk(&p, &r, out, depth + 1);
        } else if name.ends_with(".s") {
            let t = e.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            out.push((r, t));
        }
    }
}

pub fn handle(cmd: &str, req: &J) -> J {
    match handle_cmd(cmd, req) {
        Ok(j) => j,
        Err(e) => e,
    }
}

fn handle_cmd(cmd: &str, req: &J) -> Result<J, J> {
    Ok(match cmd {
        "ls" => {
            let mut v = Vec::new();
            walk(&root(), "", &mut v, 0);
            J::obj()
                .with("root", root().display().to_string().into())
                .with("files", J::Arr(v.into_iter().map(|(p, t)| J::obj().with("path", p.into()).with("mtime", J::Num(t as f64))).collect()))
        }
        "read" => {
            let rel = req.get("path").s("");
            let p = under(&rel)?;
            match std::fs::read_to_string(&p) {
                Ok(t) => J::obj().with("path", rel.into()).with("text", t.into()),
                Err(_) if req.get("create").b(false) => J::obj().with("path", rel.into()).with("text", "".into()).with("new", true.into()),
                Err(e) => return Err(err(format!("{}: {}", rel, e))),
            }
        }
        "write" => {
            let rel = req.get("path").s("");
            if !rel.ends_with(".s") {
                return Err(err("the room writes .s files"));
            }
            let p = under(&rel)?;
            if let Some(d) = p.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let tmp = p.with_extension("s.tmp");
            std::fs::write(&tmp, req.get("text").s("")).map_err(|e| err(e.to_string()))?;
            std::fs::rename(&tmp, &p).map_err(|e| err(e.to_string()))?;
            J::obj().with("saved", rel.into())
        }
        "roomget" => {
            let p = layout_path(&req.get("path").s(""))?;
            J::obj().with("text", std::fs::read_to_string(p).unwrap_or_default().into())
        }
        "roomput" => {
            let p = layout_path(&req.get("path").s(""))?;
            std::fs::write(p, req.get("text").s("")).map_err(|e| err(e.to_string()))?;
            J::obj()
        }
        "lexdeep" => lexdeep(&req.get("text").s("")),
        "srun" => {
            let text = req.get("text").s("");
            let prelude = req.get("prelude").s("");
            let trace = req.get("trace").b(true);
            let ms = req.get("ms").f(10000.0).clamp(10.0, 120000.0) as u64;
            let args: Vec<String> = req.get("args").arr().iter().map(|a| a.s("")).collect();
            let stdin = req.get("stdin").str().map(|s| s.to_string());
            // files the program names are found beside it
            let dir = match req.get("path").str() {
                Some(rel) if !rel.is_empty() => under(rel)?.parent().map(|d| d.to_path_buf()).unwrap_or_else(root),
                _ => root(),
            };
            let _ = std::env::set_current_dir(&dir);
            let h = std::thread::Builder::new()
                .stack_size(s3_core::rt::MAIN_STACK)
                .spawn(move || srun(&text, &prelude, trace, Duration::from_millis(ms), args, stdin))
                .map_err(|e| err(e.to_string()))?;
            h.join().unwrap_or_else(|_| err("the run panicked"))
        }
        "rust" => {
            let rel = req.get("path").s("");
            let p = under(&rel)?;
            let text = req.get("text").s("");
            let rs = s3_core::compile::compile((&rel, &text), &[], s3_core::LIB, None).map_err(err)?;
            let rsp = p.with_extension("rs");
            std::fs::write(&rsp, &rs).map_err(|e| err(e.to_string()))?;
            let bin = p.with_extension("");
            if !req.get("build").b(false) {
                return Ok(J::obj().with("rs", rsp.display().to_string().into()));
            }
            let o = std::process::Command::new("rustc")
                .args(["-O", "--edition", "2021", "--crate-name", "s_program", "-o"])
                .arg(&bin)
                .arg(&rsp)
                .output()
                .map_err(|e| err(format!("rustc: {}", e)))?;
            if !o.status.success() {
                return Err(err(format!("rustc failed:\n{}", String::from_utf8_lossy(&o.stderr).chars().take(4000).collect::<String>())));
            }
            J::obj().with("rs", rsp.display().to_string().into()).with("bin", bin.display().to_string().into())
        }
        "build" => build(req)?,
        // the program as Rust, for reading
        "rustsrc" => {
            let rel = req.get("path").s("");
            let text = req.get("text").s("");
            let (rs, note) = s3_core::compile::compile_with((&rel, &text), &[], s3_core::LIB, None, req.get("fold").b(true)).map_err(err)?;
            J::obj().with("rs", rs.into()).with("note", note.into())
        }
        // a file of the standard library, as the machine has it
        "lib" => {
            let p = req.get("path").s("");
            match s3_core::LIB.iter().find(|(n, _)| *n == p) {
                Some((_, t)) => J::obj().with("path", p.into()).with("text", (*t).into()),
                None => J::obj().with("files", J::Arr(s3_core::LIB.iter().map(|(n, _)| (*n).into()).collect())),
            }
        }
        _ => return Err(err(format!("unknown room command {}", cmd))),
    })
}

/// Runs s text; with trace on, each piece of output is tied to the character offset of the
/// word in the text that was running when it was written (a word of the text, or the word
/// that called the replacement or script that wrote it).
/// Also: the program's arguments (`args`, `argc`), and the lines `<_` reads.
pub fn srun(text: &str, prelude: &str, trace: bool, limit: Duration, args: Vec<String>, stdin: Option<String>) -> J {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let mut rt = Rt::with_out(&INTERP, Out::Buf(buf.clone()));
    rt.set_argv(args);
    rt.input = Some(stdin.unwrap_or_default().lines().map(|l| l.to_string()).collect());
    rt.fork_log = Some(Vec::new());
    let start = Instant::now();
    if !prelude.trim().is_empty() {
        if let Status::Error(e) = rt.run_text(prelude) {
            return J::obj().with("out", "".into()).with("error", J::obj().with("msg", format!("in the definitions: {}", e).into()));
        }
        buf.lock().unwrap().clear();
    }
    rt.unit_ids.insert(text.to_string(), 0);
    let code = match rt.code_of(text) {
        Ok(c) => c,
        Err(e) => return J::obj().with("out", "".into()).with("error", J::obj().with("msg", format!("parse: {}", e).into())),
    };
    let mut error = J::Null;
    let mut exit = J::Null;
    let mut tags: Vec<(u32, String)> = Vec::new();
    let mut last: Option<u32> = None;
    let mut seen = 0usize;
    let mut ops: u64 = 0;
    if let Some(Status::Error(e)) = rt.guarded(|rt| rt.run_src(text)) {
        error = J::obj().with("msg", e.into());
    } else {
        loop {
            let step: u64 = if trace && ops < 4_000_000 { 1 } else { 1 << 18 };
            let st = rt.run(step);
            ops += step;
            if rt.site.0 == 0 {
                if let Some(at) = code.spans.get(rt.site.1 as usize) {
                    last = Some(*at);
                }
            }
            let n = buf.lock().unwrap().len();
            if n > seen {
                if trace {
                    let s = String::from_utf8_lossy(&buf.lock().unwrap()[seen..n]).into_owned();
                    let at = last.unwrap_or(0);
                    match tags.last_mut() {
                        Some(t) if t.0 == at => t.1.push_str(&s),
                        _ => tags.push((at, s)),
                    }
                }
                seen = n;
            }
            match st {
                Status::Done => break,
                Status::Ran | Status::Break => {}
                Status::Wait => std::thread::sleep(Duration::from_millis(1)),
                Status::Exit(c) => {
                    exit = J::Num(c as f64);
                    break;
                }
                Status::Error(e) => {
                    error = J::obj().with("msg", e.into());
                    // the op that failed
                    if rt.site.0 == 0 {
                        if let Some(at) = code.spans.get(rt.site.1 as usize) {
                            last = Some(*at);
                        }
                    }
                    if let Some(at) = last {
                        error.set("at", J::Num(at as f64));
                    }
                    break;
                }
            }
            if start.elapsed() > limit {
                error = J::obj().with("msg", format!("stopped after {} ms ({} ops)", limit.as_millis(), ops).into());
                if let Some(at) = last {
                    error.set("at", J::Num(at as f64));
                }
                break;
            }
        }
    }
    rt.flush();
    let out = String::from_utf8_lossy(&buf.lock().unwrap()).into_owned();
    let forks: Vec<J> = rt
        .fork_log
        .take()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|f| {
            let lines = J::Arr(f.lines.into_iter().map(|l| J::Arr(l.into_iter().map(|v| v.into()).collect())).collect());
            place(text, &f.src, f.at).map(|at| J::obj().with("at", J::Num(at as f64)).with("how", f.how.into()).with("n", J::Num(f.n as f64)).with("lines", lines))
        })
        .collect();
    J::obj()
        .with("out", out.into())
        .with("forks", J::Arr(forks))
        .with("tags", J::Arr(tags.into_iter().map(|(a, s)| J::obj().with("at", J::Num(a as f64)).with("text", s.into())).collect()))
        .with("error", error)
        .with("exit", exit)
        .with("ops", J::Num(rt.ops as f64))
        .with("ms", J::Num(start.elapsed().as_secs_f64() * 1000.0))
}

/// Tokens with the depth of braces they sit in; a box gives an open token, its inside,
/// and a close token carrying whatever follows the brace.
pub fn lexdeep(text: &str) -> J {
    let cs: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut error = J::Null;
    deep(&cs, 0, cs.len(), 0, &mut out, &mut error);
    J::obj().with("tokens", J::Arr(out)).with("error", error)
}

fn deep(cs: &[char], from: usize, to: usize, depth: usize, out: &mut Vec<J>, error: &mut J) {
    let sub: String = cs[from..to].iter().collect();
    let r = s3_core::query::lex(&sub);
    if error.is_null() && !r.get("error").is_null() {
        *error = r.get("error").clone();
        let at = error.get("at").f(0.0) + from as f64;
        error.set("at", J::Num(at));
    }
    for t in r.get("tokens").arr() {
        let a = from + t.get("from").u(0) as usize;
        let b = from + t.get("to").u(0) as usize;
        let kind = t.get("kind").s("word");
        if kind == "brace" && depth < 64 {
            // the matching close brace
            let mut d = 0i32;
            let mut close = None;
            for i in a..b {
                if cs[i] == '{' {
                    d += 1;
                } else if cs[i] == '}' {
                    d -= 1;
                    if d == 0 {
                        close = Some(i);
                        break;
                    }
                }
            }
            let tok = |f: usize, e: usize, k: &str, dp: usize| J::obj().with("from", f.into()).with("to", e.into()).with("kind", k.into()).with("depth", dp.into()).with("w", cs[f..e].iter().collect::<String>().into());
            match close {
                Some(c) => {
                    out.push(tok(a, a + 1, "open", depth));
                    deep(cs, a + 1, c, depth + 1, out, error);
                    out.push(tok(c, b, "close", depth));
                }
                None => {
                    out.push(tok(a, a + 1, "open", depth));
                    deep(cs, a + 1, b, depth + 1, out, error);
                }
            }
            continue;
        }
        out.push(J::obj().with("from", a.into()).with("to", b.into()).with("kind", kind.into()).with("depth", depth.into()).with("w", t.get("w").clone()));
    }
}

/// Where a fork stands in the program text: its offset in the text it was read from, moved by
/// where that text sits in the program (a brace of it, a definition); in characters.
fn place(text: &str, src: &str, at: u32) -> Option<u32> {
    if src == text {
        return Some(at);
    }
    if src.is_empty() {
        return None;
    }
    let b = text.find(src)?;
    Some(text[..b].chars().count() as u32 + at)
}

/// The program compiled to Rust, built with rustc (once per text), and run with a time limit:
/// what it printed, how long it ran, and how each of its forks ran.
fn build(req: &J) -> Result<J, J> {
    let rel = req.get("path").s("");
    let text = req.get("text").s("");
    let ms = req.get("ms").f(20000.0).clamp(100.0, 300000.0) as u64;
    let fold = req.get("fold").b(true);
    let dir = match rel.is_empty() {
        true => root(),
        false => under(&rel)?.parent().map(|d| d.to_path_buf()).unwrap_or_else(root),
    };
    let cache = root().join(".s3-build");
    std::fs::create_dir_all(&cache).map_err(|e| err(e.to_string()))?;
    // the build is the s3 command beside this server, run on the text: `s3 build file.s -o bin`
    let s3 = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join(if cfg!(windows) { "s3.exe" } else { "s3" })))
        .filter(|p| p.is_file())
        .ok_or_else(|| err("no s3 beside s3-studio: cargo build --release builds both"))?;
    let stamp = std::fs::metadata(&s3).and_then(|m| m.modified()).ok();
    let name = format!("{:016x}", {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut h);
        env!("CARGO_PKG_VERSION").hash(&mut h);
        stamp.hash(&mut h);
        fold.hash(&mut h);
        h.finish()
    });
    let bin = cache.join(&name);
    let mut built_ms = 0.0;
    let mut how;
    let fresh = !bin.exists();
    if fresh {
        let t = Instant::now();
        let src = cache.join(format!("{}.s", name));
        std::fs::write(&src, &text).map_err(|e| err(e.to_string()))?;
        let mut cmd = std::process::Command::new(&s3);
        cmd.arg("build").arg(&src).arg("-o").arg(&bin).current_dir(&dir);
        if !fold {
            cmd.arg("--no-fold");
        }
        let o = cmd.output().map_err(|e| err(format!("{}: {}", s3.display(), e)))?;
        let said = String::from_utf8_lossy(&o.stderr).into_owned();
        if !o.status.success() || !bin.exists() {
            let _ = std::fs::remove_file(&bin);
            return Err(err(format!("s3 build failed:\n{}", said.chars().take(4000).collect::<String>())));
        }
        built_ms = t.elapsed().as_secs_f64() * 1000.0;
        let notes: Vec<&str> = said.lines().filter_map(|l| l.strip_prefix("s3: ")).collect();
        how = "s3 build".to_string();
        for n in notes {
            how = format!("{} · {}", how, n);
        }
        let _ = std::fs::write(cache.join(format!("{}.how", name)), &how);
    } else {
        how = std::fs::read_to_string(cache.join(format!("{}.how", name))).unwrap_or_default();
    }
    let units = s3_core::compile::compile_units((&rel, &text), &[], &[], s3_core::LIB).map(|c| c.srcs).unwrap_or_default();
    let t = Instant::now();
    let args: Vec<String> = req.get("args").arr().iter().map(|a| a.s("")).collect();
    let input = req.get("stdin").s("");
    let mut child = std::process::Command::new(&bin)
        .args(&args)
        .current_dir(&dir)
        .env("S_FORKS", "1")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| err(format!("{}: {}", bin.display(), e)))?;
    if let Some(mut si) = child.stdin.take() {
        std::thread::spawn(move || {
            let _ = std::io::Write::write_all(&mut si, input.as_bytes());
        });
    }
    // read both pipes while it runs, so a program that prints a lot does not stall
    let mut so = child.stdout.take().unwrap();
    let mut se = child.stderr.take().unwrap();
    let ho = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = std::io::Read::read_to_end(&mut so, &mut b);
        b
    });
    let he = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = std::io::Read::read_to_end(&mut se, &mut b);
        b
    });
    let mut killed = false;
    let code = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st.code(),
            Ok(None) => {
                if t.elapsed() > Duration::from_millis(ms) {
                    let _ = child.kill();
                    let _ = child.wait();
                    killed = true;
                    break None;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(e) => return Err(err(e.to_string())),
        }
    };
    let run_ms = t.elapsed().as_secs_f64() * 1000.0;
    let out = String::from_utf8_lossy(&ho.join().unwrap_or_default()).into_owned();
    let errt = String::from_utf8_lossy(&he.join().unwrap_or_default()).into_owned();
    let mut forks = Vec::new();
    let mut error = J::Null;
    let mut rest = String::new();
    for line in errt.lines() {
        let f: Vec<&str> = line.splitn(6, '\t').collect();
        if f.len() >= 4 && f[0] == "s-fork" {
            let unit: i64 = f[1].parse().unwrap_or(-1);
            let at: u32 = f[2].parse().unwrap_or(0);
            let src = if unit >= 0 { units.get(unit as usize).map(|s| s.as_str()).unwrap_or("") } else { "" };
            if let Some(p) = place(&text, src, at) {
                let n: f64 = f.get(4).and_then(|x| x.parse().ok()).unwrap_or(0.0);
                let lines = f.get(5).map(|x| if x.is_empty() { Vec::new() } else { x.split('\x1e').map(|l| J::Arr(if l.is_empty() { Vec::new() } else { l.split('\x1f').map(|v| v.to_string().into()).collect() })).collect() }).unwrap_or_default();
                forks.push(J::obj().with("at", J::Num(p as f64)).with("how", f[3].to_string().into()).with("n", J::Num(n)).with("lines", J::Arr(lines)));
            }
        } else if let Some(m) = line.strip_prefix("s: ") {
            error = J::obj().with("msg", m.to_string().into());
        } else if !line.trim().is_empty() {
            rest.push_str(line);
            rest.push('\n');
        }
    }
    if killed {
        error = J::obj().with("msg", format!("stopped after {} ms", ms).into());
    }
    Ok(J::obj()
        .with("out", out.into())
        .with("stderr", rest.into())
        .with("error", error)
        .with("exit", code.map(|c| J::Num(c as f64)).unwrap_or(J::Null))
        .with("forks", J::Arr(forks))
        .with("ms", J::Num(run_ms))
        .with("built", J::Bool(fresh))
        .with("how", how.into())
        .with("build_ms", J::Num(built_ms)))
}
