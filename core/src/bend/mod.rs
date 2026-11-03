// s → Bend 2: the residue of the machine's build-time run, written as Bend.
pub mod emit;
pub mod emit_rs;
pub mod ir;
pub mod opt;
pub mod pe;

/// The Bend program for an s program, and the effect files it imports, as (name, text).
pub fn build(src: &str, title: &str) -> Result<(String, Vec<(&'static str, &'static str)>), String> {
    let src = src.to_string();
    let title = title.to_string();
    let h = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || -> Result<String, String> {
            let lib: Vec<(String, String)> = crate::LIB.iter().map(|(p, s)| (p.to_string(), s.to_string())).collect();
            let mut pe = pe::Pe::new(lib, &src);
            let main = pe.run_main(&src)?;
            let mut main = main;
            let mut owned: Vec<Option<ir::Fun>> = pe.funs.iter_mut().map(|f| f.fun.take()).collect();
            let mut tys = pe.tys.clone();
            if std::env::var("S3_NO_FOLD").is_err() {
                opt::run(&mut owned, &mut main, &mut tys);
            }
            // Bend runs IO on one thread: each worker's block is written where it was sent
            for f in owned.iter_mut().flatten() {
                f.body.stmts = ir::inline_spawns(&f.body.stmts);
            }
            main.stmts = ir::inline_spawns(&main.stmts);
            let funs: Vec<Option<&ir::Fun>> = owned.iter().map(|f| f.as_ref()).collect();
            let mut e = emit::Emit::new(&funs, &tys, &pe.keysets);
            e.lmaps = pe.lmaps.iter().map(|m| m.1.clone()).collect();
            Ok(e.program(&main, &title))
        })
        .map_err(|e| e.to_string())?;
    let text = h.join().map_err(|_| "s3 bend: the build-time run failed".to_string())??;
    Ok((text, vec![("s_read_line.c", emit::READ_LINE_C), ("s_read_line.js", emit::READ_LINE_JS), ("s_fx.c", emit::FX_C), ("s_fx.js", emit::FX_JS)]))
}

/// The Rust program for an s program: the same residue, with no machine in it.
pub fn build_rs(src: &str, title: &str) -> Result<String, String> {
    let src = src.to_string();
    let title = title.to_string();
    let h = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || -> Result<String, String> {
            let lib: Vec<(String, String)> = crate::LIB.iter().map(|(p, s)| (p.to_string(), s.to_string())).collect();
            let mut pe = pe::Pe::new(lib, &src);
            let main = pe.run_main(&src)?;
            let mut main = main;
            let mut owned: Vec<Option<ir::Fun>> = pe.funs.iter_mut().map(|f| f.fun.take()).collect();
            let mut tys = pe.tys.clone();
            if std::env::var("S3_NO_FOLD").is_err() {
                opt::run(&mut owned, &mut main, &mut tys);
            }
            let funs: Vec<Option<&ir::Fun>> = owned.iter().map(|f| f.as_ref()).collect();
            let mut e = emit_rs::EmitRs::new(&funs, &tys, &pe.keysets);
            e.lmaps = pe.lmaps.iter().map(|m| m.1.clone()).collect();
            Ok(e.program(&main, &title))
        })
        .map_err(|e| e.to_string())?;
    h.join().map_err(|_| "s3 native: the build-time run failed".to_string())?
}
