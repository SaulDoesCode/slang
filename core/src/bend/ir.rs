// The residue of the build-time run: functions of statements over variables, before they are
// written as Bend.
use std::rc::Rc;

/// The Bend type a variable holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Ty {
    /// F32
    F,
    /// String
    S,
    /// Bool
    B,
    /// V: a number or a text
    V,
    /// SL: a list of V, newest first, with its length
    L,
    /// a list whose items are all numbers (the Rust writer's, from its own analysis)
    N,
}

impl Ty {
    pub fn bend(self) -> &'static str {
        match self {
            Ty::F => "F32",
            Ty::S => "String",
            Ty::B => "Bool",
            Ty::V => "V",
            Ty::L | Ty::N => "SL",
        }
    }
}

/// An operation on values with no effect.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum P {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    /// rem_euclid
    Mod,
    Lt,
    Gt,
    Le,
    Ge,
    /// numbers equal
    NEq,
    Not,
    And,
    /// V equal as s compares values
    VEq,
    /// text equal
    SEq,
    /// a number as s writes it
    Fmt,
    /// text read as a number, 0 when it is none
    Parse,
    /// V from a number
    VN,
    /// V from text, a number when the text is one written as s writes it
    VOf,
    /// V from text that is not a number
    VT,
    /// V a path did not set
    VX,
    /// whether a V is one a path did not set
    IsX,
    /// a V's text
    VText,
    /// a V's number
    VNum,
    /// text joined
    Cat,
    /// characters in a text
    SLen,
    /// the empty list
    LNew,
    /// list, value: the value appended
    LPush,
    /// list: without its last item
    LPop,
    /// list: its last item, or the text 404
    LLast,
    /// list, index: the item, the index counted from the end when negative, 404 outside
    LGet,
    /// list, index: whether the index is inside
    LHas,
    /// list, index, value: the item replaced; outside, the list as it was
    LSet,
    /// list: its length
    LLen,
    /// two lists, one after the other
    LCat,
    /// list, n: its first n items (n held to 0..len)
    LTake,
    /// list, n: all but its first n items
    LDrop,
    /// text: the words, split at whitespace
    SplitWs,
    /// text: the characters
    SplitChars,
    /// text, separator: the pieces
    SplitBy,
    /// list, separator: the texts joined
    Join,
    /// text is one word that the machine would run: a command, a form with a suffix, an
    /// escape, or (second argument, a list of texts) a replacement the code can see
    Runs,
    /// the list named by a text: the index of a map of known lists, the text, then name and
    /// list of each list held in a variable
    LNamed,
    /// an effect's answer was given
    FxOk,
    /// what an effect gave back
    FxVal,
    /// a shell's exit code
    ShRc,
    /// a shell's output
    ShOut,
    /// pick: condition, a, b
    If,
}

#[derive(Clone, Debug)]
pub enum Ex {
    Var(u32),
    Num(f64),
    Str(Rc<str>),
    Bool(bool),
    /// a list of texts known at build time, as V
    Strs(Rc<Vec<String>>),
    App(P, Vec<Ex>),
}

/// A function: its parameters, what it returns, its body.
pub struct Fun {
    pub name: String,
    pub params: Vec<(u32, Ty)>,
    pub outs: Vec<Ty>,
    pub body: Block,
    /// the s it stands for, for the comment above it
    pub what: String,
}

#[derive(Clone, Debug, Default)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub end: End,
}

#[derive(Clone, Debug)]
pub enum End {
    /// the values the block gives back, one per output
    Yield(Vec<Ex>),
    /// the program ends here: exit code
    Exit(Ex),
    /// the program ends with an error: the message printed after `s: `
    Fail(Ex),
    /// the block's last statement is a branch whose paths all end the program
    Never,
}

impl Default for End {
    fn default() -> End {
        End::Yield(Vec::new())
    }
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Let(u32, Ty, Ex),
    /// write text to stdout
    Write(Ex),
    /// a line from stdin: the text var and the eof var (1 at the end, else 0)
    ReadLine(u32, u32),
    /// the command line, as a list (args) and its length (argc)
    Args(u32, u32),
    /// a file, the shell or the random numbers: the op, two texts, the text that comes back
    Fx(&'static str, Ex, Ex, u32),
    /// a call: the outputs bound to vars
    Call { f: usize, args: Vec<Ex>, outs: Vec<(u32, Ty)> },
    /// a choice on a Bool: both blocks yield the outputs
    Branch { c: Ex, t: Block, e: Block, outs: Vec<(u32, Ty)> },
    /// a lane function mapped over a list: the lane's own values first, its item last; one
    /// list out per value the lane gives back
    Map { f: usize, xs: Ex, args: Vec<Ex>, outs: Vec<(u32, Ty)> },
    /// lanes run as parallel calls, each binding its own outputs
    Par { calls: Vec<(usize, Vec<Ex>, Vec<(u32, Ty)>)> },
    /// a worker's message run on its own thread: the block yields the values Join binds
    Spawn { id: u32, body: Block },
    /// the worker's thread waited for: what its block yields, bound to these vars
    Join { id: u32, outs: Vec<(u32, Ty)> },
}

/// The vars a block's top level binds, in order.
pub fn top_defs(stmts: &[Stmt]) -> Vec<u32> {
    let mut out = Vec::new();
    for s in stmts {
        match s {
            Stmt::Let(v, _, _) => out.push(*v),
            Stmt::ReadLine(a, b) | Stmt::Args(a, b) => {
                out.push(*a);
                out.push(*b)
            }
            Stmt::Fx(_, _, _, o) => out.push(*o),
            Stmt::Call { outs, .. } | Stmt::Branch { outs, .. } | Stmt::Map { outs, .. } | Stmt::Join { outs, .. } => out.extend(outs.iter().map(|o| o.0)),
            Stmt::Par { calls } => {
                for c in calls {
                    out.extend(c.2.iter().map(|o| o.0))
                }
            }
            Stmt::Spawn { .. } | Stmt::Write(_) => {}
        }
    }
    out
}

/// Statements with each worker's block written in place and its join gone: for a writer
/// whose program runs on one thread.
pub fn inline_spawns(stmts: &[Stmt]) -> Vec<Stmt> {
    let mut out = Vec::new();
    for s in stmts {
        match s {
            Stmt::Spawn { body, .. } => out.extend(inline_spawns(&body.stmts)),
            Stmt::Join { .. } => {}
            // one thread: the calls in order
            Stmt::Par { calls } => {
                for (f, args, outs) in calls {
                    out.push(Stmt::Call { f: *f, args: args.clone(), outs: outs.clone() })
                }
            }
            Stmt::Branch { c, t, e, outs } => out.push(Stmt::Branch {
                c: c.clone(),
                t: Block { stmts: inline_spawns(&t.stmts), end: t.end.clone() },
                e: Block { stmts: inline_spawns(&e.stmts), end: e.end.clone() },
                outs: outs.clone(),
            }),
            s => out.push(s.clone()),
        }
    }
    out
}

impl Ex {
    pub fn vars(&self, out: &mut Vec<u32>) {
        match self {
            Ex::Var(v) => out.push(*v),
            Ex::App(_, xs) => {
                for x in xs {
                    x.vars(out)
                }
            }
            _ => {}
        }
    }
}

/// The vars a block reads that it does not bind, in the order first read.
pub fn free_vars(stmts: &[Stmt], end: &End) -> Vec<u32> {
    fn ex(e: &Ex, def: &std::collections::BTreeSet<u32>, out: &mut Vec<u32>) {
        let mut v = Vec::new();
        e.vars(&mut v);
        for x in v {
            if !def.contains(&x) && !out.contains(&x) {
                out.push(x)
            }
        }
    }
    fn block(stmts: &[Stmt], end: &End, def: &mut std::collections::BTreeSet<u32>, out: &mut Vec<u32>) {
        for s in stmts {
            match s {
                Stmt::Let(v, _, e) => {
                    ex(e, def, out);
                    def.insert(*v);
                }
                Stmt::Write(e) => ex(e, def, out),
                Stmt::ReadLine(a, b) | Stmt::Args(a, b) => {
                    def.insert(*a);
                    def.insert(*b);
                }
                Stmt::Fx(_, a, b, o) => {
                    ex(a, def, out);
                    ex(b, def, out);
                    def.insert(*o);
                }
                Stmt::Call { args, outs, .. } => {
                    for a in args {
                        ex(a, def, out)
                    }
                    def.extend(outs.iter().map(|o| o.0));
                }
                Stmt::Map { xs, args, outs, .. } => {
                    ex(xs, def, out);
                    for a in args {
                        ex(a, def, out)
                    }
                    def.extend(outs.iter().map(|o| o.0));
                }
                Stmt::Par { calls } => {
                    for (_, args, outs) in calls {
                        for a in args {
                            ex(a, def, out)
                        }
                        def.extend(outs.iter().map(|o| o.0));
                    }
                }
                Stmt::Branch { c, t, e, outs } => {
                    ex(c, def, out);
                    let mut d1 = def.clone();
                    block(&t.stmts, &t.end, &mut d1, out);
                    let mut d2 = def.clone();
                    block(&e.stmts, &e.end, &mut d2, out);
                    def.extend(outs.iter().map(|o| o.0));
                }
                Stmt::Spawn { body, .. } => {
                    let mut d = def.clone();
                    block(&body.stmts, &body.end, &mut d, out);
                }
                Stmt::Join { outs, .. } => def.extend(outs.iter().map(|o| o.0)),
            }
        }
        match end {
            End::Yield(xs) => {
                for x in xs {
                    ex(x, def, out)
                }
            }
            End::Exit(x) | End::Fail(x) => ex(x, def, out),
            End::Never => {}
        }
    }
    let mut def = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    block(stmts, end, &mut def, &mut out);
    out
}
