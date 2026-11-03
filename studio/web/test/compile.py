# python3 studio/web/test/compile.py: compiles programs to pages and uses the pages: the
# compiled page has no reader and no library text, and does what the program says.
import functools, http.server, threading, sys, os, re, gzip
from playwright.sync_api import sync_playwright

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
srv = http.server.ThreadingHTTPServer(('127.0.0.1', 0), functools.partial(http.server.SimpleHTTPRequestHandler, directory=ROOT))
srv.RequestHandlerClass.log_message = lambda *a: None
threading.Thread(target=srv.serve_forever, daemon=True).start()
BASE = f'http://127.0.0.1:{srv.server_port}/'
OUT = os.path.join(ROOT, 'test', 'out'); os.makedirs(OUT, exist_ok=True)
ok = bad = 0
def eq(name, got, want):
    global ok, bad
    if got == want: ok += 1
    else:
        bad += 1
        print(f'--- {name}\n  want: {want!r}\n  got:  {got!r}')
ex = lambda n: open(os.path.join(ROOT, 'examples', n + '.s')).read()

with sync_playwright() as pw:
    b = pw.chromium.launch()
    cp = b.new_page()
    cp.goto(BASE + 'test/compile.html')
    cp.wait_for_function('window.ready')
    def build(name, src):
        r = cp.evaluate('(src) => compile(src)', src)
        open(os.path.join(OUT, name + '.html'), 'w').write(r['html'])
        size = len(r['html'].encode()); gz = len(gzip.compress(r['html'].encode(), 9))
        print(f'{name}: {size} bytes, {gz} gzipped, script: {"yes" if r["dynamic"] else "none"}' + (f', {r["units"]} units, {r["words"]} words' if r['dynamic'] else ''))
        if r['errors']: print('  build errors:', r['errors'])
        return r
    pg = b.new_page()
    errs = []
    pg.on('pageerror', lambda e: errs.append(str(e)))
    pg.on('console', lambda m: errs.append(m.text) if m.type == 'error' else None)
    def open_page(name):
        pg.goto(BASE + 'test/out/' + name + '.html')

    r = build('first', ex('first'))
    eq('first is plain html', r['dynamic'], False)
    eq('first has no script', '<script' in r['html'], False)
    open_page('first')
    eq('first h1', pg.inner_text('h1'), 'Hello from s')
    eq('first css', pg.evaluate("getComputedStyle(document.querySelector('.lead')).color"), 'rgb(122, 85, 50)')

    r = build('table', ex('table'))
    eq('table is plain html', r['dynamic'], False)
    open_page('table')
    eq('table rows', pg.evaluate("document.querySelectorAll('tr').length"), 1000)
    eq('table last', pg.evaluate("document.querySelector('tr:last-child').textContent.trim()"), '1000 1000000 1000000000')

    r = build('counter', ex('counter'))
    eq('counter has no reader', 'function brace(' in r['html'] or 'function parse(src, it)' in r['html'], False)
    eq('counter has no library text', 'std/core.s' in r['html'], False)
    script = re.search(r'<script>\n(.*)\n</script>', r['html'], re.S).group(1)
    print(f'  counter script: {len(script)} bytes, {len(gzip.compress(script.encode(), 9))} gzipped, {r["parts"][0]} of {r["parts"][1]} parts')
    eq('counter script under 5 KB gzipped', len(gzip.compress(script.encode(), 9)) < 5000, True)
    eq('counter keeps no command it does not reach', 'CMD["+"]' in script or 'class Rng' in script or 'lanesOp' in script, False)
    r2 = build('dice', '0 n#\n{ {~n~} p {\\#prng rn~ 6 \\* =~ n#} onclick {roll} button } div')
    eq('a page that rolls keeps the generator', 'class Rng' in r2['js'] and 'CMD["#prng"]' in r2['js'], True)
    open_page('dice'); pg.click('button')
    eq('the dice roll', 0 <= float(pg.inner_text('p')) < 6, True)
    r3 = build('spoken', "{>} cmd`\n{ {cmd~} onclick {say} button } div")
    eq('a text that names a command reaches it', 'CMD[">"]' in r3['js'], True)
    # a text the page builds as it runs is read on the page
    r4 = build('built', "0 n# 0 m#\n{ {~m~} p {n; {~n~ 10 \\* =~ m#} ~} onclick {go} button {{⟪1¦2⟫ ~n~ \\+ =~ ⨝ ys into ys zs l/ zs~ m`} ~} onclick {fork} button } div")
    eq('a page that runs built text carries the reader', 'function parse(' in r4['js'], True)
    open_page('built'); pg.click('text=go'); pg.click('text=go')
    eq('the built text ran', pg.inner_text('p'), '20')
    pg.click('text=fork')
    eq('a built text with a fork ran', pg.inner_text('p'), '3 4')
    eq('the counter carries no reader', 'function parse(' in script, False)
    open_page('counter')
    eq('counter shown before any script runs', pg.inner_text('h1'), '0')
    pg.click('text=more'); pg.click('text=more'); pg.click('text=less')
    eq('counter works', pg.inner_text('h1'), '1')

    build('clock', ex('clock'))
    open_page('clock')
    t0 = pg.inner_text('h1')
    pg.wait_for_timeout(1300)
    eq('clock ticks', pg.inner_text('h1') != t0, True)

    build('site', ex('site'))
    open_page('site')
    eq('site home', pg.inner_text('h1'), 'Pages described in s')
    pg.click('nav >> text=Counter')
    pg.wait_for_function("document.querySelector('h1').textContent == 'Counter'")
    pg.click('text=+1'); pg.click('text=+1')
    eq('site counter', pg.inner_text('p.big'), '2')
    pg.click('nav >> text=List')
    pg.wait_for_function("document.querySelector('h1').textContent == 'A list that stays'")
    pg.fill('input', 'bread'); pg.click('text=Add')
    eq('site list', pg.evaluate("[...document.querySelectorAll('li')].map(l => l.textContent)"), ['bread'])
    pg.reload()
    pg.wait_for_function("document.querySelector('h1').textContent == 'A list that stays'")
    eq('site list kept across a reload', pg.evaluate("[...document.querySelectorAll('li')].map(l => l.textContent)"), ['bread'])
    pg.click('text=Clear')

    # std words reached from a handler come along; others stay behind
    r = build('std', 'std/all.s.  3 x#  {~x~} p.v  {x~ 2 pow x@} onclick {square} button')
    open_page('std')
    pg.click('text=square')
    eq('std word in a handler', pg.inner_text('p.v'), '9')
    eq('only reached words', r['words'] < 20, True)
    # lanes and try compiled
    build('lanes', '0 t#  {~t~} p.t  {⟪1¦2¦3⟫ 10 \\* =~ ⨝ {\\+ =~} reduce t@} onclick {sum} button  {{nope.} {{caught} e@} try} onclick {fail} button  {~e~} p.e')
    open_page('lanes')
    pg.click('text=sum')
    eq('lanes in a handler', pg.inner_text('p.t'), '60')
    pg.click('text=fail')
    eq('try in a handler', pg.inner_text('p.e'), 'caught')
    # text made while the page runs is read on the page
    build('reader', '0 y` {1} x` {{~x~ y`} ~} onclick {go} button {~y~} p')
    open_page('reader')
    errs.clear()
    pg.click('text=go')
    eq('made text runs', pg.inner_text('p'), '1')
    eq('no other page errors', errs, [])
    b.close()
print(f'{ok} pass, {bad} fail')
sys.exit(1 if bad else 0)
