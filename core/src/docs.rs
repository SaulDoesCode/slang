// What each word does, for completion, hover and the palette.
use crate::json::J;
use std::sync::OnceLock;

/// (group, word, signature, description)
pub const CMDDOC: &[(&str, &str, &str, &str)] = &[
    ("io", ">", "values… >", "Write a newline, then each value preceded by one space."),
    ("io", ">_", "values… >_", "Write each value with no separators and no newline."),
    ("io", "<_", "[dest] <_", "Read one stdin line into mem[dest] (default _). mem[eof] = 1 at end of input."),
    ("io", "<", "cmd <", "Run a shell command; mem[r] = stdout, numbers[rc] = exit code."),
    ("file", "w", "path text w", "Write a file."),
    ("file", "+w", "path text +w", "Append to a file, creating it."),
    ("file", "mv", "from to mv", "Move a file, overwriting."),
    ("file", "r", "path [dest] r", "mem[dest] = file contents; dest defaults to the path."),
    ("file", "r@", "path off len dest r@", "mem[dest] = up to len bytes from byte off."),
    ("file", "f", "path name mode [err] f", "Open a handle. Modes r w a r+ w+ a+. With err given, a failure sets mem[err] and runs err."),
    ("file", "+f", "name off key [err] +f", "Write mem[key] at byte off."),
    ("file", "@f", "name off len dest @f", "mem[dest] = up to len bytes at off; eof flag when fewer were read."),
    ("file", "f_", "name text [err] f_", "Write text and a newline at the current position."),
    ("file", "_f", "name dest _f", "mem[dest] = next line; eof flag when no line was read."),
    ("file", "?eof", "name ?eof", "mem[eof] = 1 when the handle is at end (or missing), else 0."),
    ("file", "f-", "name f-", "Close a handle."),
    ("file", "tcp", "addr name mode [err] tcp", "Network handle. Mode c connects to host:port, s listens (numbers[port] = the port), a accepts on the listener named addr (mem[peer] = the other side)."),
    ("num", "+", "a b [dest] +", "numbers[dest] = a + b (dest defaults to =)."),
    ("num", "-", "a b [dest] -", "numbers[dest] = a − b."),
    ("num", "*", "a b [dest] *", "numbers[dest] = a × b."),
    ("num", "/", "a b [dest] /", "numbers[dest] = a ÷ b."),
    ("num", "^", "a b [dest] ^", "numbers[dest] = a to the power b."),
    ("num", "%", "a b [dest] %", "numbers[dest] = Euclidean remainder of a by b."),
    ("num", "n", "value name n", "numbers[name] = value."),
    ("num", "#prng", "[seed [seed2 [dest]]] #prng", "With a seed, reseed; numbers[dest] (default rn) = next float in [0, 1)."),
    ("num", "`prng", "[seed [seed2 [dest]]] `prng", "As #prng; mem[dest] = 32 letters from a-zA-Z."),
    ("cond", "=", "a b =", "ow = same text."),
    ("cond", "!=", "a b !=", "ow = different text."),
    ("cond", "&", "a b &", "ow = ow and same text."),
    ("cond", ";", "a b ;", "ow = ow and different text."),
    ("cond", "==", "a b ==", "ow = equal as numbers."),
    ("cond", "<<", "a b <<", "ow = a < b."),
    ("cond", ">>", "a b >>", "ow = a > b."),
    ("cond", "<<=", "a b <<=", "ow = a ≤ b."),
    ("cond", ">>=", "a b >>=", "ow = a ≥ b."),
    ("cond", "!!", "!!", "ow = not ow."),
    ("list", "l", "values… list l", "Append each value to the list, in order."),
    ("list", "-l", "list -l", "mem[$] = removed last item, 404 when empty."),
    ("list", "[l]", "list i dest [l]", "mem[dest] = item i; negative i counts from the end."),
    ("list", "l=", "value list i l=", "Item i = value."),
    ("list", "l-", "list l-", "Delete the list."),
    ("list", "#l", "list [dest] #l", "numbers[dest] = length."),
    ("list", "/l", "text list [sep] /l", "Split text into list: on whitespace, into characters with {} as sep, or on sep."),
    ("list", "l/", "list dest [sep] l/", "mem[dest] = items joined with sep (default space)."),
    ("list", "#s", "text [dest] #s", "numbers[dest] = number of characters."),
    ("code", "?)", "table key [default] ?)", "table holds pairs k v; the v for key (or default) runs as code and leaves its stack."),
    ("code", "&&&", "count name &&&", "Call name count times; numbers[n] = 0 … count−1."),
    ("code", "><", "value word ><", "From now on the word runs value as code wherever it stands. A body sees definitions older than itself."),
    ("code", "//", "notes… //", "Clear the stack: a comment stands to the left of //."),
    ("code", "<-", "<-", "End the current script or unit; the stack is kept as its result."),
    ("code", "<--", "[code] <--", "End the program with an exit code."),
    ("kern", "(", "… k (", "Keep the top k values; the rest of the line waits in a frame."),
    ("kern", ")", ")", "Put the waiting values back underneath the current ones."),
    ("kern", "~", "… code ~", "Run the top value as code on the rest of the line."),
    ("kern", "##", "… ##", "Push how many values are on the line."),
    ("thread", "t", "path… t", "Start a worker per file; numbers[threads] = last index."),
    ("thread", "<t", "code i <t", "Send code to worker i; it runs it and returns mem[out] if set."),
    ("thread", ">t", "i >t", "Take worker i's next result and run it here."),
    ("thread", "-t", "i -t", "Stop worker i."),
    ("thread", "***", "path… ***", "Run each file in a fresh interpreter, then run each one's mem[out] here, in order."),
];

/// (word, signature, description) for the words a graph runtime adds.
pub const HOSTDOC: &[(&str, &str, &str)] = &[
    ("move", "dx dy dz move", "Move the running sphere, then refresh strikes."),
    ("aim", "x y z aim", "Set the running sphere's aim, then refresh strikes."),
    ("at", "name at", "Aim at the named sphere's center."),
    ("pos", "pos", "Push the running sphere's x y z."),
    ("hit", "hit", "Push the name of the sphere the ray strikes, or 404."),
    ("reach", "n reach", "Set the reach of the running sphere's ray."),
    ("emit", "emit", "Send the line along out now; the line stays."),
    ("self", "self", "Push the running sphere's name."),
    ("spawn", "name spawn", "Start a zone now, taking its captures."),
    ("join", "name join", "Wait until the zone is quiet, end it, release its captures."),
    ("give", "name give", "In a gate body: move the name into the crossing."),
    ("zone", "zone", "Push the current zone's name."),
    ("host", "host", "Push the current zone's host:port, or local."),
    ("inbox", "inbox", "Push the number of waiting messages."),
    ("schema", "name schema", "Push the names of the datum's transform parts that differ from standard."),
];

pub const CAPDOC: &[(&str, &str, &str)] = &[
    ("~", "look", "Look up the name and run its value as a word."),
    ("`", "set", "mem[name] = the line."),
    ("#", "num", "numbers[name] = first value."),
    (";", "inc", "numbers[name] += first value or 1."),
    (":", "dec", "numbers[name] −= first value or 1."),
    ("$", "def", "Define script name from the line."),
    (".", "call", "Call script name (one value on the line = repeat count)."),
    ("..", "pass", "Call script name once on the current line."),
    ("@", "pop", "Move the top value into mem[name]."),
];

pub struct LibWord {
    pub word: String,
    pub file: &'static str,
    pub doc: String,
}

/// Every public word the standard library defines, with its doc comment.
pub fn lib_words() -> &'static [LibWord] {
    static W: OnceLock<Vec<LibWord>> = OnceLock::new();
    W.get_or_init(|| {
        let mut out: Vec<LibWord> = Vec::new();
        for &(file, src) in crate::LIB {
            for line in src.lines() {
                let Some(p) = line.find("' ><") else { continue };
                let Some(q) = line[..p].rfind('\'') else { continue };
                let word = &line[q + 1..p];
                if word.starts_with('_') || word.is_empty() {
                    continue;
                }
                let rest = line[p + 4..].trim();
                let doc = rest.strip_prefix('{').and_then(|r| r.rfind("} //").map(|i| r[..i].to_string())).unwrap_or_default();
                if !out.iter().any(|w| w.word == word && w.file == file) {
                    out.push(LibWord { word: word.to_string(), file, doc });
                }
            }
        }
        out
    })
}

const MODES: &[&str] = &["std/sh.s"];

/// A word the general library (not a mode) defines.
pub fn lib_doc(w: &str) -> Option<&'static LibWord> {
    lib_words().iter().find(|x| x.word == w && !MODES.contains(&x.file))
}

pub fn cmd_doc(w: &str) -> Option<&'static (&'static str, &'static str, &'static str, &'static str)> {
    CMDDOC.iter().find(|c| c.1 == w)
}

/// The palette: commands by group, library words by file, graph words, caps.
pub fn palette() -> J {
    let mut groups: Vec<(String, Vec<J>)> = Vec::new();
    let mut add = |g: &str, item: J| match groups.iter_mut().find(|x| x.0 == g) {
        Some(x) => x.1.push(item),
        None => groups.push((g.to_string(), vec![item])),
    };
    for (g, w, sig, d) in CMDDOC {
        add(g, J::obj().with("w", (*w).into()).with("kind", "cmd".into()).with("sig", (*sig).into()).with("doc", (*d).into()));
    }
    for (w, sig, d) in HOSTDOC {
        add("graph", J::obj().with("w", (*w).into()).with("kind", "geo".into()).with("sig", (*sig).into()).with("doc", (*d).into()));
    }
    for lw in lib_words() {
        add(lw.file, J::obj().with("w", lw.word.clone().into()).with("kind", "lib".into()).with("doc", lw.doc.clone().into()));
    }
    let caps = J::Arr(CAPDOC.iter().map(|(s, n, d)| J::obj().with("sigil", (*s).into()).with("name", (*n).into()).with("doc", (*d).into())).collect());
    J::obj()
        .with("groups", J::Arr(groups.into_iter().map(|(g, v)| J::obj().with("name", g.into()).with("items", J::Arr(v))).collect()))
        .with("caps", caps)
}
