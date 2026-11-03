import Base

# A value: a number or a text.
type V is Data:
  N{x: F32}
  T{s: String}
  X{}

# A list, newest item first, with its length.
type SL is Data:
  SL{n: F32, xs: List<&2, V>}

def S.read_line() -> IO(String):
  import "./s_read_line.c"
  import "./s_read_line.js"

# Files, the shell and the random numbers: "1" and what came back, or "0" and the error.
def S.fx(op: String, a: String, b: String) -> IO(String):
  import "./s_fx.c"
  import "./s_fx.js"

@unsafe def S.fx_ok(s: String) -> Bool:
  match s:
    case SNil{}:
      False{}
    case SCon{+c, t}:
      U32.is_eq(Char.to_u32(c), 49)

@unsafe def S.fx_val(s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{+c, +t}:
      t

# A shell's answer: its exit code, a byte 1, its output.
@unsafe def S.sh_rc(s: String) -> F32:
  S.parse(S.before1(s, SNil{}))

@unsafe def S.before1(s: String, acc: String) -> String:
  match s:
    case SNil{}:
      String.reverse(acc)
    case SCon{+c, t}:
      S.before1.if(U32.is_eq(Char.to_u32(c), 1), c, t, acc)

@unsafe def S.before1.if(e: Bool, +c: Char, t: String, acc: String) -> String:
  match e:
    case True{}:
      String.reverse(acc)
    case False{}:
      S.before1(t, SCon{c, acc})

@unsafe def S.sh_out(s: String) -> String:
  match s:
    case SNil{}:
      ""
    case SCon{+c, t}:
      S.sh_out.if(U32.is_eq(Char.to_u32(c), 1), t)

@unsafe def S.sh_out.if(e: Bool, t: String) -> String:
  match e:
    case True{}:
      t
    case False{}:
      S.sh_out(t)

@unsafe def S.pick(c: Bool, +a: F32, +b: F32) -> F32:
  match c:
    case True{}:
      a
    case False{}:
      b

@unsafe def S.pickv(c: Bool, +a: V, +b: V) -> V:
  match c:
    case True{}:
      a
    case False{}:
      b

# A number as s writes it.
@unsafe def S.fmt(+x: F32) -> String:
  S.fmt.go(F32.is_eq(x, 0.0), F32.is_ne(x, x), x)

@unsafe def S.fmt.go(z: Bool, nan: Bool, +x: F32) -> String:
  match z:
    case True{}:
      "0"
    case False{}:
      S.fmt.nan(nan, x)

@unsafe def S.fmt.nan(nan: Bool, +x: F32) -> String:
  match nan:
    case True{}:
      "NaN"
    case False{}:
      F32.show(x)

# Text read as a number, 0 when it is none.
@unsafe def S.parse(s: String) -> F32:
  S.parse.go(F32.read(String.trim(s)))

@unsafe def S.parse.go(m: Maybe<&2, F32>) -> F32:
  match m:
    case None{}:
      0.0
    case Some{+x}:
      x

@unsafe def S.rem(+a: F32, +b: F32) -> F32:
  S.rem.go(F32.mod(a, b), b)

@unsafe def S.rem.go(+r: F32, +b: F32) -> F32:
  S.pick(F32.is_lt(r, 0.0), F32.add(r, F32.abs(b)), r)

@unsafe def V.of(+s: String) -> V:
  V.of.go(F32.read(s), s)

@unsafe def V.of.go(m: Maybe<&2, F32>, +s: String) -> V:
  match m:
    case None{}:
      T{s}
    case Some{+x}:
      V.of.chk(String.eq(S.fmt(x), s), x, s)

@unsafe def V.of.chk(ok: Bool, +x: F32, +s: String) -> V:
  match ok:
    case True{}:
      N{x}
    case False{}:
      T{s}

@unsafe def V.text(v: V) -> String:
  match v:
    case N{+x}:
      S.fmt(x)
    case T{+s}:
      s
    case X{}:
      "\u{1}"

@unsafe def V.absent(v: V) -> Bool:
  match v:
    case X{}:
      True{}
    case N{+x}:
      False{}
    case T{+s}:
      False{}

@unsafe def V.num(v: V) -> F32:
  match v:
    case N{+x}:
      x
    case T{+s}:
      S.parse(s)
    case X{}:
      0.0

@unsafe def V.eq(+a: V, +b: V) -> Bool:
  String.eq(V.text(a), V.text(b))

@unsafe def S.slen(s: String) -> F32:
  U32.to_f32(U32.from_nat(String.length(s)))

# Lists

# A list cut into runs, held in a balanced tree, for lanes mapped in parallel.
type ST is Data:
  SLeaf{n: F32, xs: List<&2, V>}
  SNode{a: ST, b: ST}

@unsafe def S.lrev(xs: List<&2, V>, acc: List<&2, V>) -> List<&2, V>:
  match xs:
    case Nil{}:
      acc
    case Con{+h, t}:
      S.lrev(t, h <> acc)

@unsafe def S.strev(xs: List<&2, ST>, acc: List<&2, ST>) -> List<&2, ST>:
  match xs:
    case Nil{}:
      acc
    case Con{+h, t}:
      S.strev(t, h <> acc)

@unsafe def S.chunks(xs: List<&2, V>) -> List<&2, ST>:
  S.chunks.go(xs, Nil{}, 0.0, Nil{})

@unsafe def S.chunks.go(xs: List<&2, V>, cur: List<&2, V>, +k: F32, acc: List<&2, ST>) -> List<&2, ST>:
  match xs:
    case Nil{}:
      S.strev(S.chunks.end(cur, k, acc), Nil{})
    case Con{+h, t}:
      S.chunks.if(F32.is_ge(k, PARRUN), h, t, cur, k, acc)

@unsafe def S.chunks.if(c: Bool, +h: V, t: List<&2, V>, cur: List<&2, V>, +k: F32, acc: List<&2, ST>) -> List<&2, ST>:
  match c:
    case True{}:
      S.chunks.go(t, h <> Nil{}, 1.0, SLeaf{k, S.lrev(cur, Nil{})} <> acc)
    case False{}:
      S.chunks.go(t, h <> cur, F32.add(k, 1.0), acc)

@unsafe def S.chunks.end(cur: List<&2, V>, +k: F32, acc: List<&2, ST>) -> List<&2, ST>:
  match cur:
    case Nil{}:
      acc
    case Con{+h, t}:
      SLeaf{k, S.lrev(h <> t, Nil{})} <> acc

@unsafe def S.tree(cs: List<&2, ST>) -> ST:
  match cs:
    case Nil{}:
      SLeaf{0.0, Nil{}}
    case Con{+h, t}:
      S.tree.t(t, h)

@unsafe def S.tree.t(t: List<&2, ST>, +h: ST) -> ST:
  match t:
    case Nil{}:
      h
    case Con{+x, r}:
      S.tree(S.pairs(h <> x <> r))

@unsafe def S.pairs(cs: List<&2, ST>) -> List<&2, ST>:
  match cs:
    case Nil{}:
      Nil{}
    case Con{+a, t}:
      S.pairs.b(t, a)

@unsafe def S.pairs.b(t: List<&2, ST>, +a: ST) -> List<&2, ST>:
  match t:
    case Nil{}:
      a <> Nil{}
    case Con{+b, r}:
      SNode{a, b} <> S.pairs(r)

@unsafe def S.lapp(xs: List<&2, V>, ys: List<&2, V>) -> List<&2, V>:
  match xs:
    case Nil{}:
      ys
    case Con{+h, t}:
      h <> S.lapp(t, ys)

@unsafe def S.clampk(+k: F32, +n: F32) -> F32:
  S.pick(F32.is_le(k, 0.0), 0.0, S.pick(F32.is_ge(k, n), n, F32.trunc(k)))

@unsafe def S.dropn(xs: List<&2, V>, +m: F32) -> List<&2, V>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, t}:
      S.dropn.at(F32.is_le(m, 0.0), h, t, m)

@unsafe def S.dropn.at(c: Bool, +h: V, t: List<&2, V>, +m: F32) -> List<&2, V>:
  match c:
    case True{}:
      h <> t
    case False{}:
      S.dropn(t, F32.sub(m, 1.0))

@unsafe def S.taken(xs: List<&2, V>, +m: F32) -> List<&2, V>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, t}:
      S.taken.at(F32.is_le(m, 0.0), h, t, m)

@unsafe def S.taken.at(c: Bool, +h: V, t: List<&2, V>, +m: F32) -> List<&2, V>:
  match c:
    case True{}:
      Nil{}
    case False{}:
      h <> S.taken(t, F32.sub(m, 1.0))

# the first k items: the newest n-k dropped
@unsafe def S.ltake(l: SL, +k: F32) -> SL:
  match l:
    case SL{+n, xs}:
      S.ltake.go(xs, n, S.clampk(k, n))

@unsafe def S.ltake.go(xs: List<&2, V>, +n: F32, +k: F32) -> SL:
  SL{k, S.dropn(xs, F32.sub(n, k))}

# all but the first k: the newest n-k kept
@unsafe def S.ldrop(l: SL, +k: F32) -> SL:
  match l:
    case SL{+n, xs}:
      S.ldrop.go(xs, n, S.clampk(k, n))

@unsafe def S.ldrop.go(xs: List<&2, V>, +n: F32, +k: F32) -> SL:
  SL{F32.sub(n, k), S.taken(xs, F32.sub(n, k))}

@unsafe def S.lcat(a: SL, b: SL) -> SL:
  match a:
    case SL{+na, xa}:
      S.lcat.b(b, na, xa)

@unsafe def S.lcat.b(b: SL, +na: F32, xa: List<&2, V>) -> SL:
  match b:
    case SL{+nb, xb}:
      SL{F32.add(na, nb), S.lapp(xa, xb)}

@unsafe def S.mget(m: Map<&2, SL>, +k: String) -> SL:
  S.mget.go(Map.get(SL, S.lnew(), m, k))

@unsafe def S.mget.go(p: Map<&2, SL> & SL) -> SL:
  (m, +v) = p
  v

@unsafe def S.items(l: SL) -> List<&2, V>:
  match l:
    case SL{+n, xs}:
      xs

@unsafe def S.lnew() -> SL:
  SL{0.0, Nil{}}

@unsafe def S.lpush(l: SL, +v: V) -> SL:
  match l:
    case SL{+n, xs}:
      SL{F32.add(n, 1.0), v <> xs}

@unsafe def S.lpop(l: SL) -> SL:
  match l:
    case SL{+n, xs}:
      S.lpop.go(xs, n)

@unsafe def S.lpop.go(xs: List<&2, V>, +n: F32) -> SL:
  match xs:
    case Nil{}:
      SL{n, Nil{}}
    case Con{h, t}:
      SL{F32.sub(n, 1.0), t}

@unsafe def S.head(xs: List<&2, V>) -> V:
  match xs:
    case Nil{}:
      T{"404"}
    case Con{+h, t}:
      h

@unsafe def S.llast(l: SL) -> V:
  match l:
    case SL{+n, xs}:
      S.head(xs)

@unsafe def S.llen(l: SL) -> F32:
  match l:
    case SL{+n, xs}:
      n

# Where index i (from the oldest, or from the end when negative) stands counted from the
# newest item; -1 outside the list.
@unsafe def S.at(+n: F32, +i: F32) -> F32:
  S.at.go(n, S.pick(F32.is_lt(F32.trunc(i), 0.0), F32.add(F32.trunc(i), n), F32.trunc(i)))

@unsafe def S.at.go(+n: F32, +j: F32) -> F32:
  S.pick(Bool.and(F32.is_ge(j, 0.0), F32.is_lt(j, n)), F32.sub(F32.sub(n, 1.0), j), F32.neg(1.0))

@unsafe def S.lhas(l: SL, +i: F32) -> Bool:
  match l:
    case SL{+n, xs}:
      F32.is_ge(S.at(n, i), 0.0)

@unsafe def S.lget(l: SL, +i: F32) -> V:
  match l:
    case SL{+n, xs}:
      S.lget.go(xs, S.at(n, i))

@unsafe def S.lget.go(xs: List<&2, V>, +k: F32) -> V:
  match xs:
    case Nil{}:
      T{"404"}
    case Con{+h, t}:
      S.lget.at(F32.is_eq(k, 0.0), h, t, k)

@unsafe def S.lget.at(c: Bool, +h: V, t: List<&2, V>, +k: F32) -> V:
  match c:
    case True{}:
      h
    case False{}:
      S.lget.go(t, F32.sub(k, 1.0))

@unsafe def S.lset(l: SL, +i: F32, +v: V) -> SL:
  match l:
    case SL{+n, xs}:
      SL{n, S.lset.go(xs, S.at(n, i), v)}

@unsafe def S.lset.go(xs: List<&2, V>, +k: F32, +v: V) -> List<&2, V>:
  match xs:
    case Nil{}:
      Nil{}
    case Con{+h, t}:
      S.lset.at(F32.is_eq(k, 0.0), h, t, k, v)

@unsafe def S.lset.at(c: Bool, +h: V, t: List<&2, V>, +k: F32, +v: V) -> List<&2, V>:
  match c:
    case True{}:
      v <> t
    case False{}:
      h <> S.lset.go(t, F32.sub(k, 1.0), v)

@unsafe def S.texts(xs: List<&2, V>, acc: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}:
      acc
    case Con{+h, t}:
      S.texts(t, V.text(h) <> acc)

@unsafe def S.join(l: SL, +sep: String) -> String:
  match l:
    case SL{+n, xs}:
      String.join(S.texts(xs, Nil{}), sep)

@unsafe def S.is_ws(c: Char) -> Bool:
  S.is_ws.go(Char.to_u32(c))

@unsafe def S.is_ws.go(+u: U32) -> Bool:
  Bool.or(Bool.or(U32.is_eq(u, 32), U32.is_eq(u, 10)), Bool.or(U32.is_eq(u, 9), U32.is_eq(u, 13)))

@unsafe def S.flush(cur: String, acc: SL) -> SL:
  match cur:
    case SNil{}:
      acc
    case SCon{+h, t}:
      S.lpush(acc, V.of(String.reverse(SCon{h, t})))

@unsafe def S.splitws.go(s: String, cur: String, acc: SL) -> SL:
  match s:
    case SNil{}:
      S.flush(cur, acc)
    case SCon{+c, t}:
      S.splitws.ch(S.is_ws(c), c, t, cur, acc)

@unsafe def S.splitws.ch(w: Bool, +c: Char, t: String, cur: String, acc: SL) -> SL:
  match w:
    case True{}:
      S.splitws.go(t, SNil{}, S.flush(cur, acc))
    case False{}:
      S.splitws.go(t, SCon{c, cur}, acc)

@unsafe def S.splitws(s: String) -> SL:
  S.splitws.go(s, SNil{}, S.lnew())

@unsafe def S.splitchars.go(s: String, acc: SL) -> SL:
  match s:
    case SNil{}:
      acc
    case SCon{+c, t}:
      S.splitchars.go(t, S.lpush(acc, V.of(SCon{c, SNil{}})))

@unsafe def S.splitchars(s: String) -> SL:
  S.splitchars.go(s, S.lnew())

@unsafe def S.splitby.go(s: String, +sep: String, cur: String, acc: SL) -> SL:
  match s:
    case SNil{}:
      S.lpush(acc, V.of(String.reverse(cur)))
    case SCon{+c, +t}:
      S.splitby.at(String.starts_with(SCon{c, t}, sep), c, t, sep, cur, acc)

@unsafe def S.splitby.at(hit: Bool, +c: Char, +t: String, +sep: String, cur: String, acc: SL) -> SL:
  match hit:
    case True{}:
      S.splitby.go(String.drop(SCon{c, t}, String.length(sep)), sep, SNil{}, S.lpush(acc, V.of(String.reverse(cur))))
    case False{}:
      S.splitby.go(t, sep, SCon{c, cur}, acc)

@unsafe def S.splitby(s: String, +sep: String) -> SL:
  S.splitby.if(String.is_empty(sep), s, sep)

@unsafe def S.splitby.if(e: Bool, s: String, +sep: String) -> SL:
  match e:
    case True{}:
      S.splitchars(s)
    case False{}:
      S.splitby.go(s, sep, SNil{}, S.lnew())

@unsafe def S.args.go(t: List<String>, acc: SL) -> SL:
  match t:
    case Nil{}:
      acc
    case Con{h, t2}:
      S.args.go(t2, S.lpush(acc, V.of(h)))

@unsafe def S.args(a: List<String>) -> SL:
  match a:
    case Nil{}:
      S.lnew()
    case Con{h, t}:
      S.args.go(t, S.lnew())

# A line from stdin: "1" and the line, or "0" at the end.
@unsafe def S.rl_eof(s: String) -> F32:
  match s:
    case SNil{}:
      1.0
    case SCon{+c, t}:
      S.pick(U32.is_eq(Char.to_u32(c), 48), 1.0, 0.0)

@unsafe def S.rl_val(s: String) -> V:
  match s:
    case SNil{}:
      T{""}
    case SCon{+c, +t}:
      V.of(t)

# Whether text is one word the machine runs: a command, a form with a suffix, an escape, or a
# replacement in ks.
@unsafe def S.member(+t: String, ks: List<&2, String>) -> Bool:
  match ks:
    case Nil{}:
      False{}
    case Con{+k, rest}:
      Bool.or(String.eq(t, k), S.member(t, rest))


@unsafe def S.ws_in(s: String) -> Bool:
  match s:
    case SNil{}:
      False{}
    case SCon{+c, t}:
      Bool.or(S.is_ws(c), S.ws_in(t))

@unsafe def S.long(s: String) -> Bool:
  match s:
    case SNil{}:
      False{}
    case SCon{+c, t}:
      Bool.not(String.is_empty(t))

@unsafe def S.active(+t: String) -> Bool:
  Bool.or(String.starts_with(t, "⟨"), Bool.and(S.long(t), Bool.or(String.starts_with(t, "\\"), S.member(String.drop(t, Nat.sub(String.length(t), 1n)), ["?", "!", "~", "$", "`", "#", ";", ":", ".", "@"]))))

@unsafe def S.oneword(+t: String) -> Bool:
  Bool.and(Bool.not(String.is_empty(t)), Bool.not(S.ws_in(t)))

@unsafe def S.pickl(c: Bool, +a: SL, +b: SL) -> SL:
  match c:
    case True{}:
      a
    case False{}:
      b
