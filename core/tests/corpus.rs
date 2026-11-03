// s10-corpus: every s 1.0 case, imported as a world, built and run, prints what it printed.
use s3_core::build::build;
use s3_core::ops::import_text;
use s3_core::rt::Program;
use s3_core::session::run_headless;
use std::path::Path;

static P: Program = Program::with_lib(s3_core::LIB);

#[test]
fn s10_corpus_runs_the_same_as_worlds() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
    std::env::set_current_dir(&dir).unwrap();
    let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().map_or(false, |e| e == "s") && p.with_extension("out").exists()).collect();
    names.sort();
    let mut failed = Vec::new();
    for p in &names {
        let src = std::fs::read_to_string(p).unwrap();
        let want = std::fs::read_to_string(p.with_extension("out")).unwrap();
        let w = match import_text("case", &src) {
            Ok(w) => w,
            Err(e) => {
                failed.push(format!("{}: import: {}", p.display(), e));
                continue;
            }
        };
        let spec = match build(&w) {
            Ok(s) => s,
            Err(e) => {
                failed.push(format!("{}: build: {:?}", p.display(), e));
                continue;
            }
        };
        let (got, s) = run_headless(spec, &P, 0);
        if got != want {
            failed.push(format!("{}:\n--- want\n{}\n--- got\n{}\n--- errors {:?}", p.display(), want, got, s.errors.iter().map(|e| e.dump()).collect::<Vec<_>>()));
        }
    }
    assert!(failed.is_empty(), "{} of {} failed:\n{}", failed.len(), names.len(), failed.join("\n\n"));
}
