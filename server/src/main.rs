// s3-studio: serves the studio page and answers its requests from one core thread.
mod core;
mod room;

use s3_core::json::J;
use salvo::prelude::*;
use salvo::serve_static::StaticDir;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};

static CORE: OnceLock<Mutex<Sender<core::Req>>> = OnceLock::new();

async fn ask(req: J) -> J {
    let (tx, rx) = channel();
    let ok = CORE.get().map(|c| c.lock().unwrap().send((req, tx)).is_ok()).unwrap_or(false);
    if !ok {
        return J::obj().with("error", "the core thread is gone".into());
    }
    tokio::task::spawn_blocking(move || rx.recv().unwrap_or_else(|_| J::obj().with("error", "no answer".into())))
        .await
        .unwrap_or_else(|_| J::obj().with("error", "no answer".into()))
}

#[handler]
async fn api(req: &mut Request, res: &mut Response) {
    let body = match req.payload_with_max_size(32 << 20).await {
        Ok(b) => String::from_utf8_lossy(b).into_owned(),
        Err(e) => {
            res.status_code(StatusCode::BAD_REQUEST);
            res.render(Text::Plain(e.to_string()));
            return;
        }
    };
    let j = match J::parse(&body) {
        Ok(j) => j,
        Err(e) => J::obj().with("cmd", "".into()).with("parse", e.into()),
    };
    let cmd = j.get("cmd").s("");
    let out = if room::CMDS.contains(&cmd.as_str()) {
        tokio::task::spawn_blocking(move || room::handle(&cmd, &j)).await.unwrap_or_else(|_| J::obj().with("error", "no answer".into()))
    } else {
        ask(j).await
    };
    res.add_header("cache-control", "no-store", true).ok();
    res.render(Text::Json(out.dump()));
}

fn arg(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1).cloned())
}

fn find_dir(name: &str) -> PathBuf {
    let here = PathBuf::from(name);
    if here.is_dir() {
        return here;
    }
    if let Ok(exe) = std::env::current_exe() {
        for up in exe.ancestors().skip(1) {
            let p = up.join(name);
            if p.is_dir() {
                return p;
            }
        }
    }
    here
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("usage: s3-studio [--port 7333] [--host 127.0.0.1] [--worlds dir] [--studio dir] [--levels dir] [--files dir]\n  --files: the folder of .s files the room opens (default: the current folder)");
        return;
    }
    let port = arg(&args, "--port").unwrap_or_else(|| "7333".into());
    let host = arg(&args, "--host").unwrap_or_else(|| "127.0.0.1".into());
    let worlds = arg(&args, "--worlds").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("worlds"));
    let studio = arg(&args, "--studio").map(PathBuf::from).unwrap_or_else(|| find_dir("studio"));
    let levels = arg(&args, "--levels").map(PathBuf::from).unwrap_or_else(|| find_dir("levels"));
    // every folder absolute: a run in the room works in its file's folder
    let _ = std::fs::create_dir_all(&worlds);
    let worlds = worlds.canonicalize().unwrap_or(worlds);
    let studio = studio.canonicalize().unwrap_or(studio);
    let levels = levels.canonicalize().unwrap_or(levels);
    let files = arg(&args, "--files").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    room::ROOT.set(files.canonicalize().unwrap_or(files.clone())).ok();
    let (tx, rx) = channel();
    CORE.set(Mutex::new(tx)).ok();
    let (w2, l2) = (worlds.clone(), levels.clone());
    std::thread::Builder::new()
        .name("s3-core".into())
        .stack_size(s3_core::rt::MAIN_STACK)
        .spawn(move || core::Core::new(w2, l2).serve(rx))
        .expect("core thread");
    let router = Router::new()
        .push(Router::with_path("api").post(api))
        .push(Router::with_path("{*path}").get(StaticDir::new([studio.clone()]).defaults("index.html")));
    let addr = format!("{}:{}", host, port);
    println!("s3 studio on http://{}  (s files in {}, worlds in {}, studio from {})", addr, files.display(), worlds.display(), studio.display());
    let acceptor = TcpListener::new(addr).bind().await;
    Server::new(acceptor).serve(router).await;
}
