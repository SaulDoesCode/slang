pub mod bend;
pub mod build;
pub mod compile;
pub mod docs;
pub mod graph;
pub mod json;
pub mod lanes;
pub mod machine;
pub mod ops;
pub mod part;
pub mod query;
pub mod rt;
pub mod session;
pub mod strike;
pub mod textform;
pub mod wire;
pub mod world;
pub mod zone;

/// The standard library, available to every program by path (`std/core.s.`).
/// A file on disk at the same path takes precedence.
pub static LIB: &[(&str, &str)] = &[
    ("std/core.s", include_str!("../std/core.s")),
    ("std/line.s", include_str!("../std/line.s")),
    ("std/text.s", include_str!("../std/text.s")),
    ("std/gem.s", include_str!("../std/gem.s")),
    ("std/sh.s", include_str!("../std/sh.s")),
    ("std/math.s", include_str!("../std/math.s")),
    ("std/check.s", include_str!("../std/check.s")),
    ("std/all.s", include_str!("../std/all.s")),
    ("std/logic.s", include_str!("../std/logic.s")),
    ("std/datum.s", include_str!("../std/datum.s")),
];
