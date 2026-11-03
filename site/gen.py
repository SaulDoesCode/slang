import html, json, re

FONTS = '<link rel="preconnect" href="https://fonts.googleapis.com"><link rel="preconnect" href="https://fonts.gstatic.com" crossorigin><link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:ital,wght@0,400;0,500;0,600;1,400&family=IBM+Plex+Sans:wght@400;500;600&display=swap">'
PAGES = [("index.html", "s"), ("guide.html", "Guide"), ("reference.html", "Reference"), ("std.html", "Library"), ("build.html", "Building")]

def e(s):
    return html.escape(s, quote=False)

N = [0]
def ex(code, stdin=None, args=None, out=None, lab=None, id=None):
    """An s block that runs in the page; with out, a block showing what the Rust machine printed."""
    N[0] += 1
    i = id or "ex%d" % N[0]
    a = ' id="%s"' % i
    if stdin is not None:
        a += ' data-stdin="%s"' % html.escape(stdin.replace("\n", "\\n"))
    if args is not None:
        a += ' data-args="%s"' % html.escape(args)
    if lab:
        a += ' data-lab="%s"' % html.escape(lab)
    if out is not None:
        a += ' data-static'
        o = '<pre class="out" data-lab="output, from s3 run">%s</pre>' % e(out)
    else:
        o = ''
    return '<div class="ex"%s><pre class="s">%s</pre>%s</div>' % (a, e(code.strip("\n")), o)

def c(s):
    return "<code>%s</code>" % e(s)

def header(cur):
    nav = "".join('<a href="%s"%s>%s</a>' % (f, ' aria-current="page"' if f == cur else '', t) for f, t in PAGES[1:])
    return '<header class="top gutter"><div class="in"><a class="mark" href="index.html">s<span>s3 0.3</span></a><nav aria-label="Site">%s</nav></div></header>' % nav

FOOT = '<footer class="foot gutter"><div class="in"><span>s3 0.3</span><a href="guide.html">Guide</a><a href="reference.html">Reference</a><a href="std.html">Library</a><a href="build.html">Building</a><span>The examples run on the s machine in this page.</span></div></footer>'

def page(fname, title, desc, body, full=True):
    head = '<title>%s</title>\n<meta name="claude:viewport-fit-sides" content="cover">\n<meta name="description" content="%s">\n%s\n<link rel="stylesheet" href="site.css">\n' % (title, html.escape(desc), FONTS)
    main = header(fname) + body + FOOT + '\n<script src="machine.js"></script>\n<script src="site.js"></script>\n'
    if full:
        doc = '<!doctype html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">\n' + head + '</head>\n<body>\n' + main + '</body>\n</html>\n'
    else:
        doc = head + main
    open('/tmp/site/out/' + fname, 'w').write(doc)

def docpage(fname, title, h1, lead, sections, desc):
    rail = '<nav class="rail" aria-label="On this page"><p>%s</p><ol>%s</ol></nav>' % (e(h1), "".join('<li><a href="#%s">%s</a></li>' % (sid, e(t)) for sid, t, _ in sections))
    secs = "".join('<section id="%s"><h2>%s</h2>%s</section>' % (sid, e(t), b) for sid, t, b in sections)
    body = '<main class="doc gutter">%s<div class="body"><header><h1>%s</h1><p>%s</p></header>%s</div></main>' % (rail, e(h1), lead, secs)
    page(fname, title, desc, body)

def P(*xs):
    return "".join("<p>%s</p>" % x for x in xs)

def dl(rows):
    return '<dl class="defs">%s</dl>' % "".join("<dt>%s</dt><dd>%s</dd>" % (e(a), b) for a, b in rows)

def table(head, rows, cls=None):
    cls = cls or [""] * len(head)
    th = "".join('<th%s>%s</th>' % (' class="n"' if k == "n" else "", h) for h, k in zip(head, cls))
    tr = "".join("<tr>%s</tr>" % "".join('<td%s>%s</td>' % (' class="%s"' % k if k else "", v) for v, k in zip(r, cls)) for r in rows)
    return '<div class="tw"><table><thead><tr>%s</tr></thead><tbody>%s</tbody></table></div>' % (th, tr)

# ---------------------------------------------------------------- front page

hero_prog = """{1 2 3 4 5 6 7 8} xs /l
0 s#  0 i#
{i~ 8 == <-?
 xs i~ x [l]
 x~ x~ * =~  s~ + =~ s#
 i;  sq.} sq$
sq.
{the sum of the squares is ~s~} >"""

front = '<main class="front gutter">'
front += '<div class="hero"><div><h1>s</h1><p class="lead">A postfix language written right-aligned. Words push values onto a line from left to right, and the command at the end of the line consumes it, so every command stands at the right margin.</p>'
front += '<p class="sub">The machine runs s directly. <code>s3 native</code> compiles a program to plain Rust with no runtime in it, and <code>s3 bend</code> writes it as Bend 2. Every example on this site runs on the s machine in the page.</p>'
front += '<div class="go"><a class="btn primary" href="guide.html">Read the guide</a><a class="btn" href="reference.html">Reference</a></div></div>'
front += '<div>' + ex(hero_prog, lab="playground.s", id="play", stdin="", args="") + '</div></div>'

front += '<div class="band"><h2>A line</h2>'
front += table(["s", "what happens"], [
    [e("5 x n"), "The command " + c("n") + " takes the line " + c("5 x") + ": the name x holds the number 5."],
    [e("x~ 2 + =~ >"), c("x~") + " pushes 5. " + c("+") + " adds 5 and 2 and writes 7 to the name " + c("=") + ". " + c("=~") + " pushes 7, and " + c(">") + " prints it."],
    [e("{a b c} xs /l"), c("/l") + " splits the text at whitespace into the list xs: a, b, c."],
    [e("a b c xs l"), "Every value before the list name is appended: xs is now a, b, c, a, b, c."],
], ["code", ""])
front += '</div>'

front += '<div class="band"><h2>Four ways to run a program</h2>'
front += table(["command", "what it makes"], [
    [c("s3 run prog.s"), "Runs the program on the machine: the reference semantics, with every command."],
    [c("s3 build prog.s"), "One Rust program that carries the machine. What the program does before it first reaches outside (input, files, shell, threads, network, randomness, arguments) is done at build time."],
    [c("s3 native prog.s"), "Rust with no machine in it. The program runs at build time; what is left is only what depends on input. Numbers are f64, lists are vectors changed in place, a script calling itself last is a loop, a fork over numbers is a vector kernel."],
    [c("s3 bend prog.s"), "The same residue written as Bend 2, with F32 numbers and no vector kernels."],
])
front += '<p class="note">Measured once each on the Linux x86-64 machine these builds were made on. The list loop pushes 2,000,000 numbers onto a list, then sums them by index.</p>'
front += table(["program", "s3 run", "s3 native", "s3 bend", "Rust by hand"], [
    ["fib 32, two calls per step", "1.97 s", "0.017 s", "0.019 s", "&nbsp;"],
    ["fib 40", "&nbsp;", "0.653 s", "&nbsp;", "0.640 s"],
    ["list loop, 2,000,000 items", "0.62 s", "0.044 s", "&nbsp;", "0.019 s"],
], ["", "n", "n", "n", "n"])
front += '</div>'

front += '<div class="band"><h2>Lanes and SIMD</h2>'
front += P("A fork over a list of numbers whose lanes do arithmetic, comparisons and writes under the flag runs as one program over the column of numbers. Each way of running does it differently:")
front += table(["way", "a fork over numbers", "10,000,000 lanes of " + c("2 * 1 + 3 /")], [
    [c("s3 run"), "A vector program in the machine, in blocks of 2048 lanes.", "58 ms"],
    [c("s3 build"), "A compiled kernel for AVX-512, AVX or SSE2, chosen on the machine it runs on; NEON on ARM.", "41 ms"],
    [c("s3 native"), "A compiled kernel for AVX-512, AVX2 or the x86-64 baseline, chosen when it runs, cut across the cores. On ARM, what rustc vectorizes for the target; not tested there.", "24 ms"],
    [c("s3 bend"), "No vector kernel: Bend has no SIMD. Lanes over a list map in parallel as a tree.", "&nbsp;"],
    ["one lane at a time", "What the machine does with a fork it cannot vectorize.", "9.6 s"],
], ["", "", "n"])
front += '<p class="note">s3 build and s3 native as each reports with S_FORKS=1, s3 run and one lane at a time from the repository’s lanes benchmark, all on one Linux x86-64 machine with AVX-512. No kernel uses fused multiply-add, so each prints what the machine prints. <a href="build.html#missing">What each way does not do yet</a> is listed on the building page.</p></div>'
front += '<div class="band"><h2>In the language</h2><div class="cols">'
for t, d, h in [
    ("Names with forms", "A name holds a text, a number, a list and a script. The last character of a word says what it does with the name: " + c("x~") + " reads, " + c("x#") + " sets a number, " + c("x;") + " adds one, " + c("x$") + " defines a script.", "guide.html#names"),
    ("Scripts and tail calls", "A brace becomes a script with " + c("name$") + ". A call as the last word of a script is a tail call, so loops are scripts that call themselves.", "guide.html#scripts"),
    ("Replacements", c("{code} 'word' ><") + " makes a word run code. A script sees the replacements that existed when it was defined.", "guide.html#replacements"),
    ("Lists and text", "Push, pop, index, split and join, on lists that grow as values arrive.", "guide.html#lists"),
    ("Input, files, shell", "stdin by line, files whole or by handle, the shell's output and exit code.", "guide.html#io"),
    ("Threads and TCP", "Workers that run other files and answer with code, and sockets through the same handles as files.", "guide.html#threads"),
    ("Lanes", "A line splits into lanes that run the same words on their own copies of the names, and join, filed by index.", "guide.html#lanes"),
    ("A standard library in s", "Stack words, folds over the line, text, maths, checks and shell pipelines, written in s.", "std.html"),
]:
    front += '<div><h3><a href="%s">%s</a></h3><p>%s</p></div>' % (h, e(t), d)
front += '</div></div>'

front += '<div class="band"><h2>From the source tree</h2>'
front += '<pre class="out" data-lab="shell">cargo build --release\n./target/release/s3 run prog.s           # run on the machine\n./target/release/s3 native prog.s -o prog # build plain Rust, needs rustc\n./target/release/s3 bend prog.s -o prog   # build through Bend 2, needs bend\ncargo test --release</pre></div>'
front += '</main>'
page("index.html", "The s language", "s, a right-aligned postfix language: the guide, the reference and the library, with every example running in the page.", front, full=False)

# ---------------------------------------------------------------- guide

G = []
G.append(("run", "Running a program", P(
    "A program is a text file of s. " + c("s3 run prog.s") + " runs it on the machine; " + c("s3 prog.s a b") + " runs it with the arguments a and b, which the program finds in the list " + c("args") + " and the number " + c("argc") + ".",
    "Each example on this page is an editor. Change it and press Run or Ctrl+Enter; Reset brings the example back.")
    + ex("{hello, world} >")
    + P(c(">") + " prints a newline, then each value on the line after a space. " + c(">_") + " prints the values with nothing added.")))

G.append(("line", "The line", P(
    "Words are separated by whitespace, newlines included, and taken from left to right. A word pushes a value onto the line, reads or writes a name, or is a command. A command consumes every value on the line as its arguments, in order, and the line is empty after it.",
    "Values a line does not consume stay for the next command, wherever that command stands in the text. A program is written right-aligned so the commands line up at the right margin.")
    + ex("5 x n\nx~ 2 + =~ >\n3 4 total *  total~ >")
    + P("The arithmetic commands write their result to the name " + c("=") + ", or to the name given as a third value. " + c("=~") + " puts the result back on the line.")))

G.append(("values", "Values", dl([
    ("12  -0.5", "A word that reads as a number written the way s writes numbers is a number. Numbers are 64-bit floating point. They print in the shortest form that reads back the same, and negative zero prints as 0."),
    ("word", "Any other word is pushed as itself."),
    ("{a text}", "Braces hold text with spaces, newlines and nested braces. Inside the outer braces, " + c("~name~") + " is replaced by the name's text when the brace is pushed."),
    ("'a text'", "Quotes push the text as written, with no replacing."),
    ("{r}", "A command word runs wherever it appears. Written in braces it is a text: " + c("5 {r} n") + " sets the name r, where " + c("5 r n") + " would run " + c("r") + " first."),
]) + ex("world w`\n{hello ~w~} >\n'~w~ stays as written' >")))

G.append(("names", "Names", P(
    "A name holds a text, a number, a list and a script, each apart from the others. The last character of a word, after at least one other character, chooses what the word does with the name.")
    + dl([
        ("x~", "Pushes x: its text when it has one, else its number, else 404. When that value is one word that runs (a command, a form, a replacement), it runs."),
        ("{~x~}", "Pushes x's text inside a brace. It never runs."),
        ("x`", "x's text becomes the line: the one value, or all of them joined with spaces. The line is emptied."),
        ("x#", "x's number becomes the first value on the line, or 0. The line is emptied."),
        ("x;  x:", "Adds to x's number, or subtracts from it, the first value on the line, or 1."),
        ("x@", "Takes the last value off the line into x's text."),
        ("x$", "The line's text becomes the script x. A " + c("|") + " in it is read as " + c("~") + "."),
        ("x.  x..", "Calls the script x. With exactly one value on the line, " + c("x.") + " runs x that many times; " + c("x..") + " always calls once and leaves the line for the script."),
        (":x", "A name that starts with a colon belongs to the call it was written in. Each call of a script has its own."),
    ])
    + ex("0 i#  i;  i;  5 i;  i~ >\na b c x@  x~ >")
    + P("The first form reads the value back after " + c("x@") + " took it off the line, so the line prints a b c again.",
        "A few names belong to the machine: " + c("=") + " holds results, " + c("r") + " and " + c("rc") + " the shell's output and exit code, " + c("err") + " the last error, " + c("eof") + " whether input has ended. The <a href=\"reference.html#names\">reference</a> lists them all.")
    + ex("{>} p`\nhello p~\nhello {~p~} >")
    + P(c("p") + " holds the command " + c(">") + ", so " + c("p~") + " runs it on the line " + c("hello") + ". The brace reads p as text and pushes it.")))

G.append(("conditions", "Conditions", P(
    "A comparison sets the flag " + c("ow") + " and consumes the line. A word that ends in " + c("?") + " runs only when ow is set; one that ends in " + c("!") + " runs only when it is clear. The suffix goes on any word or form: " + c(">?") + ", " + c("r#?") + ", " + c("loop.?") + ", " + c("<-?") + ".")
    + dl([
        ("=  !=", "The same value, or a different one: numbers by value, everything else by text."),
        ("==", "Equal as numbers."),
        ("<<  >>  <<=  >>=", "Less, greater, at most, at least, as numbers."),
        ("&  ;", "ow stays set only when it was set and the values are the same (" + c("&") + ") or different (" + c(";") + ")."),
        ("!!", "Flips ow."),
    ])
    + ex("4 x n\nx~ 3 >>  {four is more than three} >?\nx~ 3 <<  {not less} >!")))

G.append(("scripts", "Scripts", P(
    "A brace followed by " + c("name$") + " is a script. " + c("name.") + " calls it. " + c("<-") + " returns from the script running; " + c("<-?") + " returns when ow is set. A call that is the last word of a script is a tail call: the call replaces the running one, so a loop written as a script that calls itself runs in constant space.")
    + ex("{{hello again} >} hi$\n3 hi.")
    + P("With one value on the line, a call runs the script that many times. " + c("&&&") + " does the same and sets " + c("n") + " to the round, from 0.")
    + ex("{n~ >} round$\n3 round &&&")
    + P("A script that takes values from the line is called with " + c("..") + ". Each call of fib below has its own " + c(":n") + " and " + c(":a") + "; it leaves its answer in r.")
    + ex("{:n#  :n~ 2 << :n~ r#? <-?  //  :n~ 1 - =~ fib..  r~ :a#  :n~ 2 - =~ fib..  :a~ r~ + =~ r#} fib$\n20 fib..  r~ >")
    + P("A script that calls itself last is a loop. This one reads stdin until it ends, then prints the count and the sum.")
    + ex("0 n#  0 s#\n{x <_  eof~ 1 == <-?\n n;  s~ x~ + =~ s#\n loop.} loop$\nloop.\nn~ s~ >", stdin="4\n5\n6")))

G.append(("replacements", "Replacements", P(
    c("{code} 'word' ><") + " makes the word run the code wherever it appears afterwards, command words included. " + c("\\word") + " is the command or the plain word, past any replacement.")
    + ex("{2 *} 'double' ><\n5 double =~ >\n{ {small} > } 'big' ><\nbig  \\big >")
    + P("Every definition has a generation. A script sees the replacements made before the script was defined, so a later definition of the same word changes the word everywhere after it and leaves older scripts as they were.")
    + ex("{ {old} > } 'w' ><\n{w} f$\n{ {new} > } 'w' ><\nf.  w")))

G.append(("frames", "Frames", P(
    c("k (") + " keeps the last k values on the line and puts the rest aside; " + c(")") + " puts them back in front of what the line holds then. " + c("##") + " pushes how many values the line holds. The standard library's words use frames to act on the top of the line and leave the rest.")
    + ex("1 2 3  1 (  10 + =~  )  >")))

G.append(("lists", "Lists", dl([
    ("a b xs l", "Appends every value before the name to the list xs."),
    ("xs -l", "Takes the last item off xs into the name $, or 404 when xs is empty."),
    ("xs i v [l]", "Item i of xs into v. A negative i counts from the end. Outside the list, v keeps what it held."),
    ("v xs i l=", "Sets item i of xs to v."),
    ("xs #l", "The length of xs into =, or into the name given second."),
    ("xs l-", "Deletes xs."),
]) + ex("a b c xs l\nxs 1 v [l]  v~ >\nZ xs 0 l=  xs o l/ o~ >\nxs -l  $~ >")))

G.append(("text", "Text", dl([
    ("t xs /l", "Splits the text t into the list xs at whitespace; with a third value, at that separator; at " + c("{}") + ", into characters."),
    ("xs o l/", "Joins xs into the name o with single spaces, or with a separator given third."),
    ("t #s", "The number of characters of t into =, or into the name given second."),
]) + ex("{a b c} xs /l\nxs #l =~ >\nxs o {-} l/ o~ >\n{héllo} #s =~ >")))

G.append(("code", "Code as values", dl([
    ("text ~", "Runs the text as s."),
    ("table key ?)", "The table is pairs of words, key then code. Runs the code after the key, or a default given third."),
    ("n name &&&", "Runs the script n times with n set to the round."),
]) + ex("{3 4 * =~ >} ~\n{{one} >} one$  {{two} >} two$\n{a one. b two.} b {{neither} >} ?)\n{a one. b two.} z {{neither} >} ?)")))

G.append(("io", "Input and output", dl([
    ("name <_", "Reads a line from stdin into name, or into _. eof holds 1 once input has ended, else 0."),
    ("path text w", "Writes the text to the file; " + c("+w") + " appends; " + c("a b mv") + " moves a file."),
    ("path name r", "Reads the whole file into name. Without a name, into the name spelled as the path."),
    ("cmd <", "Runs the command in sh: its output in r, its exit code in rc."),
    ("args  argc", "The command line as a list, and its length."),
]) + ex("name <_  {hello ~name~} >", stdin="world")
    + ex("notes.txt {hello} w\nnotes.txt text r  text~ >\nnotes.txt { again} +w\nnotes.txt r  notes.txt~ >")
    + P("Files in the page live in memory for the length of one run. The shell is not part of a browser, so the next example shows what s3 run printed.")
    + ex("{echo one; echo two} <  r~ >_  rc~ >", out="one\ntwo\n\n 0")
    + ex("args 0 a [l]  argc~ a~ >", args="x y")
    + P("Handles read and write a file in parts: " + c("path h mode f") + " opens (r, w, a, r+, w+, a+), " + c("h line f_") + " writes a line, " + c("h v _f") + " reads one, " + c("h ?eof") + " sets eof, " + c("h f-") + " closes. The <a href=\"reference.html#commands\">reference</a> has the rest.")))

G.append(("errors", "Errors", P(
    "An error stops the program with " + c("s:") + " and the message on stderr, and exit code 1. " + c("{body} {handler} try") + " runs the body; when it fails, the handler runs with the message in err. " + c("f") + " and " + c("tcp") + " take a script as their last value, which runs on failure the same way.", c("code <--") + " ends the program with that exit code.")
    + ex("{{missing.txt} r} {{caught: ~err~} >} try")
    + ex("{before} >  3 <--  {after} >")))

G.append(("threads", "Threads and the network", P(
    c("file t") + " starts a worker: its own machine, running the file. " + c("text i <t") + " sends worker i a text, which it runs as code. When a message leaves code in the worker's name out, that is its reply; " + c("i >t") + " waits for the reply and runs it in the sender. " + c("i -t") + " closes the worker. " + c("files… ***") + " runs files side by side and then runs each one's out in order.",
    "A browser has no threads for the machine, so these examples show what s3 run printed. The worker file, threads_worker.s:")
    + '<pre class="s">0 hits#\n{hits;  {\'got ~msg~ hits ~hits~\' >} out`} handle$</pre>'
    + ex("threads_worker.s t\n{ping msg` handle.} 0 <t\n0 >t\n{pong msg` handle.} 0 <t\n0 >t\n0 -t\n{x} 0 <t  err~ >", out="\n got ping hits 1\n got pong hits 2\n Invalid thread index")
    + P(c("addr h mode tcp") + " opens a connection: mode c connects, s listens and sets port, a accepts on the listener named first and sets peer. A connection takes " + c("f_") + ", " + c("_f") + ", " + c("?eof") + " and " + c("f-") + " like a file. Here the client is a worker, so one program is both ends.")
    + ex("127.0.0.1:0 srv s tcp\nrs_tcp_client.s t\n{127.0.0.1:~port~ k c tcp  k {hello from the client} f_  k reply _f  {'client got: ~reply~' >} out`  k f-} 0 <t\nsrv conn a tcp\nconn line _f  {server got: ~line~} >\nconn {echo ~line~} f_\n0 >t\nconn line _f  conn ?eof  {after close, eof is ~eof~} >\nconn f-  srv f-\n127.0.0.1:1 x c {{connection refused, as expected} >} tcp", out="\n server got: hello from the client\n client got: echo hello from the client\n after close, eof is 1\n connection refused, as expected")))

G.append(("lanes", "Lanes", P(
    "A fork splits the line into lanes. " + c("⟪a¦b¦c⟫ words ⨝") + " has three lanes; each lane's line is the line at the fork, its own values, then the words after " + c("⟫") + ". " + c("⟪xs…⟫") + " has one lane per item of the list xs.",
    "Each lane writes into its own copy of the names as they were at the fork. At the join " + c("⨝") + " the names are as they were at the fork, and the lanes are held, lane 0 first.")
    + dl([
        ("collect", "Puts every lane's line on the line, lane after lane."),
        ("xs into", "Makes the list xs from the lanes' lines."),
        ("i lane", "Brings back lane i's line, and reads its writes first until " + c("endlane") + "."),
        ("{code} reduce", "Folds the lanes' lines with the code, from lane 0."),
        ("{code} lanewise", "Runs the code once in each lane."),
        ("lanes", "Pushes how many lanes there are."),
    ])
    + ex("⟪1¦2¦3⟫ 10 * =~ ⨝ collect >\n{1 2 3 4} xs /l\n⟪xs…⟫ 2 * =~ 1 + =~ ⨝ ys into  ys o l/ o~ >\n⟪1¦2¦3¦4⟫ ⨝ {+ =~} reduce >")
    + P("The machine runs a fork over numbers with arithmetic and comparisons as one vector program over the column, a fork whose lanes hold code on threads, and anything else one lane at a time. s3 build and s3 native compile a fork over numbers to a SIMD kernel; s3 bend does not. The output is the same every way. With S_FORKS=1 a run, or a built program, reports on stderr how each fork ran and how long it took.")))

G.append(("loading", "Loading files", P(
    "A call of a name with no script runs the file of that name: " + c("std/line.s.") + " loads the line library, " + c("helpers.s.") + " runs helpers.s from the program's folder. Each library file loads once however often it is called. " + c("std/all.s.") + " loads core, line, text, math and check.")
    + ex("std/line.s.\n1 2 3 4 sum >\n1 2 3 4 5 {2 mul} map >\n1 2 3 4 5 6 {2 mod 0 eq} filter >")))

docpage("guide.html", "s Guide", "Guide", "The language from the first line to threads and lanes. Every example runs here; change it and run it again.", G, "The s guide: the line, names, scripts, replacements, lists, input and output, threads and lanes.")

# ---------------------------------------------------------------- reference

forms = [
    ("x~", "Pushes x (text, else number, else 404); a one-word value that runs, runs."),
    ("{~x~}", "Inside a brace: x's text, never run."),
    ("x`", "Sets x's text from the line (one value, or all joined with spaces)."),
    ("x#", "Sets x's number from the first value, or 0."),
    ("x;", "Adds the first value, or 1, to x's number."),
    ("x:", "Subtracts the first value, or 1, from x's number."),
    ("x@", "Takes the last value off the line into x."),
    ("x$", "Defines the script x from the line's text; | reads as ~."),
    ("x.", "Calls x; with one value on the line, runs x that many times; last in a script, a tail call."),
    ("x..", "Calls x once, leaving the line to it."),
    ("form?", "Runs the form when ow is set."),
    ("form!", "Runs the form when ow is clear."),
    ("\\word", "The command itself past any replacement; for any other word, the word as text."),
    ("⟨word⟩~", "A name that ends in a suffix character, used with any form."),
    (":x", "A name that belongs to the running call."),
]
names = [
    ("=", "Results of arithmetic, #l and #s, when no other name is given."),
    ("n", "The round of &&&."),
    ("r  rc", "The output and exit code of <."),
    ("rn", "The result of #prng and `prng."),
    ("$", "The item -l took."),
    ("err", "The message of the last error a try, f or tcp caught, and of a thread command that failed."),
    ("eof", "1 once <_, _f or ?eof found the end, else 0."),
    ("out", "A worker's reply; a *** file's code to run."),
    ("threads", "The index of the last worker t started."),
    ("_", "The line <_ read, when no name is given."),
    ("args  argc", "The command line and its length."),
    ("port  peer", "Set by tcp: the port a listener got, and the address it accepted."),
    ("?", "A new random 64-bit number on every read."),
]
groups = [
    ("Output", [(">", "v… >", "Prints a newline, then each value after a space."), (">_", "v… >_", "Prints the values with nothing added.")]),
    ("Input, files, shell", [
        ("<_", "[name] <_", "Reads a line of stdin into name, or _; sets eof."),
        ("w", "path text w", "Writes the file."),
        ("+w", "path text +w", "Appends to the file."),
        ("mv", "a b mv", "Moves a to b."),
        ("r", "path [name] r", "Reads the file into name, or into the name spelled as the path."),
        ("r@", "path off len name r@", "Reads len bytes from offset off into name."),
        ("<", "cmd <", "Runs cmd in sh: output in r, exit code in rc."),
        ("f", "path h mode [script] f", "Opens a handle: r, w, a, r+, w+ or a+."),
        ("f_", "h text [script] f_", "Writes the text and a newline."),
        ("_f", "h name _f", "Reads a line into name; sets eof at the end."),
        ("+f", "h off name [script] +f", "Writes name's text at offset off."),
        ("@f", "h off len name @f", "Reads len bytes at offset off into name."),
        ("?eof", "h ?eof", "Sets eof for the handle."),
        ("f-", "h f-", "Closes the handle."),
    ]),
    ("Names and numbers", [
        ("n", "x name n", "Sets name's number."),
        ("+ - * / ^", "a b [name] +", "Adds, subtracts, multiplies, divides, raises; into =, or name."),
        ("%", "a b [name] %", "The remainder of a by b, never negative for b > 0."),
        ("#prng", "[a [b [name]]] #prng", "A random number from 0 up to 1 into rn, or name; a and b set the seed first."),
        ("`prng", "[a [b [name]]] `prng", "Thirty-two random letters, the same way."),
    ]),
    ("Comparisons", [
        ("=  !=", "a b =", "ow: same value, different value."),
        ("==", "a b ==", "ow: equal as numbers."),
        ("<<  >>  <<=  >>=", "a b <<", "ow: less, greater, at most, at least."),
        ("&  ;", "a b &", "ow: still set and the same; still set and different."),
        ("!!", "!!", "Flips ow."),
    ]),
    ("Lists and text", [
        ("l", "v… xs l", "Appends each value to xs."),
        ("-l", "xs -l", "Takes the last item into $."),
        ("[l]", "xs i name [l]", "Item i into name; negative i from the end."),
        ("l=", "v xs i l=", "Sets item i."),
        ("l-", "xs l-", "Deletes the list."),
        ("#l", "xs [name] #l", "The length into =, or name."),
        ("/l", "text xs [sep] /l", "Splits at whitespace, at sep, or at {} into characters."),
        ("l/", "xs name [sep] l/", "Joins with spaces, or sep."),
        ("#s", "text [name] #s", "The number of characters."),
    ]),
    ("Code", [
        ("><", "value 'word' ><", "The word runs the value as code from now on."),
        ("~", "text ~", "Runs the text."),
        ("?)", "table key [default] ?)", "Runs the code after key in the table of pairs, or default."),
        ("&&&", "count name &&&", "Runs the script count times, n holding the round."),
        ("//", "… //", "Consumes the line: a brace before it is a comment."),
        ("<-", "<-", "Returns from the running script."),
        ("<--", "[code] <--", "Ends the program with the exit code, 0 without one."),
        ("(", "… k (", "Keeps the last k values, puts the rest aside."),
        (")", ")", "Puts the values set aside back in front."),
        ("##", "##", "Pushes the number of values on the line."),
    ]),
    ("Threads and the network", [
        ("t", "file… t", "Starts a worker for each file."),
        ("<t", "text i <t", "Sends worker i a text to run."),
        (">t", "i >t", "Waits for worker i's reply and runs it."),
        ("-t", "i -t", "Closes worker i."),
        ("***", "file… ***", "Runs the files side by side, then each file's out in order."),
        ("tcp", "addr h mode [script] tcp", "c connects, s listens (sets port), a accepts on the listener named first (sets peer)."),
    ]),
]
R = []
R.append(("forms", "Forms", P("The last character of a word, after at least one other, chooses its form. A word that is a command is the command, before any form.") + table(["form", "does"], [[e(a), e(b)] for a, b in forms], ["w", ""])))
R.append(("names", "Names the machine sets", table(["name", "holds"], [[e(a), e(b)] for a, b in names], ["w", ""])))
R.append(("commands", "Commands", P("A command takes the values on the line in order, shown left to right; values in brackets may be left out. After a command the line is empty, except after (, ), ~, ## and <-, which work on the line itself.") + "".join("<h3>%s</h3>" % e(g) + table(["word", "line", "does"], [[e(w), e(l), e(d)] for w, l, d in rows], ["w", "code", ""]) for g, rows in groups)))
R.append(("lanes", "Lane words", table(["word", "does"], [[e(a), e(b)] for a, b in [
    ("⟪a¦b⟫ … ⨝", "A fork of lanes a and b, joined at ⨝."),
    ("⟪xs…⟫", "One lane per item of xs, each starting with its item."),
    ("[[ || ]] ||>", "The same marks in ASCII."),
    ("collect", "Every lane's line onto the line."),
    ("xs into", "The lanes' lines as the list xs."),
    ("i lane  endlane", "Lane i's line, with its writes read first until endlane."),
    ("{code} reduce", "Folds the lanes' lines with code."),
    ("{code} lanewise", "Runs code in each lane."),
    ("lanes", "The number of lanes."),
    ("{body} {handler} try", "Runs body; on an error, handler with the message in err."),
]], ["w", ""])))
docpage("reference.html", "s Reference", "Reference", "Every form, every name the machine sets, every command and the line each one takes.", R, "The s reference: forms, the names the machine sets, and every command with the line it takes.")

# ---------------------------------------------------------------- library

std = json.load(open('/tmp/site/std.json'))
L = []
keep = ['core', 'line', 'text', 'math', 'check', 'sh', 'gem']
ex_for = {
    'core': "std/core.s.\n3 4 swap >\n2 3 add 4 mul >",
    'line': "std/line.s.\n5 iota {dup mul} map >\n3 1 2 sort >",
    'text': "std/text.s.\n{one two three} words count >\n{a,b,c} {,} split rev glue >\n{shout} upper >",
    'math': "std/math.s.\n12 18 gcd >\n10 fact >\n2 sqrt 3 places >",
    'check': "std/check.s.\n2 2 add 4 expectn\n{ab} {a} expect\nchecked",
    'gem': "std/gem.s.\n{abc} ord >",
}
for m in std:
    if m['mod'] not in keep:
        continue
    rows = []
    sec = None
    body = P(e(m['about'][0].upper() + m['about'][1:]) + ". Load it with " + c("std/%s.s." % m['mod']) + ".")
    groups_ = []
    for s_, w, d in m['words']:
        if not groups_ or groups_[-1][0] != s_:
            groups_.append((s_, []))
        groups_[-1][1].append((w, d))
    for s_, ws_ in groups_:
        if s_:
            body += "<h3>%s</h3>" % e(s_)
        body += table(["word", "line before → after"], [[e(w), e(d)] for w, d in ws_], ["w", ""])
    if m['mod'] in ex_for:
        body += ex(ex_for[m['mod']])
    if m['mod'] == 'sh':
        body += ex("std/sh.s.\n{echo one; echo two; echo one} sh |> uniq |> nl |> put", out="1  one\n2  two\n3  one\n")
    L.append((m['mod'], m['mod'], body))
docpage("std.html", "s Library", "Standard library", "The library is written in s. Each word shows the line it takes and what it leaves, left to right, the top of the line last. " + c("std/all.s.") + " loads core, line, text, math and check.", L, "The s standard library: core, line, text, math, check, sh and gem, each word with the line it takes and leaves.")

# ---------------------------------------------------------------- building

B = []
B.append(("commands", "The commands", table(["command", "what it makes"], [
    [c("s3 run prog.s [args…]"), "Runs the program on the machine."],
    [c("s3 rs prog.s [-o out.rs]"), "One Rust program carrying the machine, with what the program does before it first reaches outside done at build time. " + c("--no-fold") + " builds it as written."],
    [c("s3 build prog.s [-o bin]"), "The same, compiled with rustc -O."],
    [c("s3 native prog.s [-o bin]"), "Plain Rust with no machine in it, compiled with rustc; with " + c("-o prog.rs") + ", the Rust only."],
    [c("s3 bend prog.s [-o bin]"), "Bend 2, run through bend; with " + c("-o prog.bend") + ", the Bend only, beside its effect files."],
])))
B.append(("fold", "Folding", P(
    "A build runs the program at build time up to the first word that reaches outside the machine: stdin, files, the shell, threads, the network, random numbers or the arguments. The binary starts there, with what the program printed already written and its names, lists, scripts, replacements and line in place. A program that never reaches outside builds to its output.")))
B.append(("native", "s3 native", P(
    "The program runs at build time on an abstract machine that holds every value it can know, and writes out as code only what depends on input. A loop whose rounds differ only in such values becomes a function, and a choice on such a value becomes a branch. What is left is written as Rust:")
    + "<ul><li>Numbers are f64 and print exactly as the machine prints them. Texts are shared strings.</li>"
    "<li>A list is a vector shared by reference count, moved where it is read the last time and changed in place. A list that only ever holds numbers is a vector of f64.</li>"
    "<li>A script that calls itself last is a <code>loop</code>. Scripts that call each other last fold into one loop at build time.</li>"
    "<li>Before the Rust is written, one pass keeps only the parameters a function reads, the results a caller reads and the statements whose values are read, and passes a value read only as a number as a number.</li>"
    "<li>Files, the shell, sockets and random numbers are calls into std. A worker message that does any of these runs on its own thread from <code>&lt;t</code> to <code>&gt;t</code>; one that does none folds away.</li>"
    "<li>A fork over a list of numbers whose lanes give back numbers is one loop over the column, compiled for AVX-512, AVX2 and the baseline and chosen on the machine it runs on, and cut across its cores for long lists. It uses no fused multiply-add, so it prints what the machine prints. A remainder in a lane runs in vectors in a form exact for whole numbers below 2^53; when any input is not one, the column is taken again with the exact remainder.</li>"
    "<li>A fork over a list of other values runs its lanes on threads in chunks. A fork with a known number of lanes that do nothing outside runs each lane as a function, the lanes split in halves over the spare cores.</li></ul>"
    + P("fib as s3 native writes it, from the fib in the <a href=\"guide.html#scripts\">guide</a>:")
    + '<pre class="out" data-lab="rust">%s</pre>' % e("fn f1(mut v31: f64) -> f64 {\nlet mut v84: f64 = v31;\nlet mut v85: bool = (v84 < (2.0f64));\nlet mut v105: f64 = if v85 {\nv84\n} else {\nlet mut v86: f64 = (v84 - (1.0f64));\nlet mut v89: f64 = f1(v86);\nlet mut v94: f64 = (v84 - (2.0f64));\nlet mut v97: f64 = f1(v94);\nlet mut v102: f64 = (v89 + v97);\nv102\n};\nreturn v105;\n}")
    + P("All 30 programs of the corpus, the input programs and 300 walks of the cube example give what the machine gives.")))
B.append(("bend", "s3 bend", P(
    "s3 bend writes the same residue, after the same pass, as Bend 2. Numbers are F32, so a result past 24 bits of precision differs from the machine's. Bend runs effects on one thread, so a program whose worker waits on the program itself, as the TCP example does, does not finish there. 27 of the 30 corpus programs give what the machine gives; the three that differ are those cases.")))
B.append(("missing", "What each way does not do yet", P("s3 run and s3 build carry the machine and do everything the language has. s3 native builds every program: what it cannot write without the machine (words written in Rust with <code>-n</code>, worlds, and the cases under <a href=\"#limits\">What needs the machine</a>) it builds with the machine in it, as s3 build does, and says so on stderr. These are the things s3 native and s3 bend do not do yet:")
    + "<h3>s3 native</h3><ul>"
    "<li>A word that comes from input and would run as code, such as an argument that is a command word read with <code>x~</code>, stops the program with a message; the machine runs it. A program cannot know such words before it runs.</li>"
    "<li>Worker files given to <code>t</code> and <code>***</code> are built in. When the program starts it checks them, and a file changed since the build stops it with a message; s3 build runs the changed file.</li>"
    "</ul>"
    "<h3>s3 bend</h3><ul>"
    "<li>Everything listed for s3 native, and the cases under <a href=\"#limits\">What needs the machine</a>, which s3 bend refuses.</li>"
    "<li>No vector kernels: Bend has no SIMD.</li>"
    "<li>F32 numbers: a result past 24 bits of precision prints differently from the machine.</li>"
    "<li>Effects on one thread and no sockets: <code>tcp</code>, and a worker that waits on the program itself, do not work.</li></ul>"))
B.append(("limits", "What needs the machine", P(
    "These need the machine at run time. s3 native builds a program that has one with the machine in it and names the word and the line on stderr; s3 bend stops with that message:")
    + "<ul><li>Text that comes from input run as code: <code>~</code>, a script or a replacement defined from it, or a replacement's value built from it.</li>"
    "<li>A <code>?)</code> table that comes from input.</li>"
    "<li>A name or a list index that comes from input; a number from input where numbers have replacements.</li>"
    "<li>A fork over a list from input whose lanes write a list or a script or end the program; <code>into</code> over such lanes when a lane's line holds more than one value; an error inside lanes inside a <code>try</code>.</li>"
    "<li>A second message to a worker before <code>&gt;t</code> takes the reply to the first; a thread index from input; a worker's error that depends on input.</li>"
    "<li>A recursion whose end does not settle at build time; two paths that end differently and cannot be joined.</li>"
    "<li>Lanes held from before a loop that the loop reads before it forks again.</li>"
    "<li>A message to a worker, or a reply, built from input where the value lands outside a quoted text or a number.</li>"
    "<li>A loop whose rounds set different names or open different frames in a way no number of rounds settles.</li></ul>"
    + P("s3 build carries the machine, so it builds every program s3 run runs.")))

B.append(("numbers", "Measurements", P("One run each on the Linux x86-64 machine the builds were made on.")
    + table(["program", "s3 run", "s3 native", "s3 bend", "Rust by hand"], [
        ["fib 32", "1.97 s", "0.017 s", "0.019 s", "&nbsp;"],
        ["fib 40", "&nbsp;", "0.653 s", "&nbsp;", "0.640 s"],
        ["2,000,000 numbers pushed, then summed by index", "0.62 s", "0.044 s", "&nbsp;", "0.019 s"],
    ], ["", "n", "n", "n", "n"])
    + P("A fork over 10,000,000 numbers from input, " + c("⟪xs…⟫ 2 \\* =~ 1 \\+ =~ 3 \\/ =~ ⨝ ys into") + ", on a machine with AVX-512:")
    + table(["build", "the fork"], [
        ["s3 build, its compiled kernel, as it reports", "41 ms"],
        ["s3 native, as it reports with S_FORKS=1", "24 ms"],
        ["the same with " + c("x 7 %") + " in the lane: s3 build's kernel", "297 ms"],
        ["the same with " + c("x 7 %") + ": s3 native", "34 ms"],
    ], ["", "n"])
    + P("A built program reports each fork with S_FORKS=1. s3 native writes a line like <code>s-fork 0 f64×8 avx-512 · 10000000 lanes · native kernel · 24.0 ms</code>.")
    + P("The list loop's gap to the hand-written loop is the rule of " + c("[l]") + ": an index outside the list leaves the name as it was, so each read checks the index and keeps the old value.")))
docpage("build.html", "Building s", "Building", "Run on the machine, build with the machine inside, or compile to Rust or Bend with no machine at all.", B, "Building s programs: folding, s3 native to plain Rust, s3 bend to Bend 2, and what each does not build.")
print("ok", N[0], "examples")
