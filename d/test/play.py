# python3 d/test/play.py: the d playground — running, counts, errors, export.
import functools, http.server, threading, sys, os
from playwright.sync_api import sync_playwright
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
srv = http.server.ThreadingHTTPServer(('127.0.0.1', 0), functools.partial(http.server.SimpleHTTPRequestHandler, directory=ROOT))
srv.RequestHandlerClass.log_message = lambda *a: None
threading.Thread(target=srv.serve_forever, daemon=True).start()
ok = bad = 0
def eq(name, got, want):
    global ok, bad
    if got == want: ok += 1
    else:
        bad += 1; print(f'--- {name}\n  want: {want!r}\n  got:  {got!r}')
with sync_playwright() as pw:
    b = pw.chromium.launch()
    ctx = b.new_context(accept_downloads=True, viewport={'width': 1300, 'height': 800})
    pg = ctx.new_page()
    errs = []
    pg.on('pageerror', lambda e: errs.append(str(e)))
    pg.goto(f'http://127.0.0.1:{srv.server_port}/index.html')
    pg.wait_for_function('window.src')
    code = "n = $(0)\n$`div>h1${n}+button{more}${() => n(x => x + 1)}`"
    pg.fill('#src', code)
    fr = pg.frame_locator('#view')
    fr.locator('h1').wait_for()
    fr.locator('button').click(); fr.locator('button').click()
    eq('runs and lives', fr.locator('h1').inner_text(), '2')
    pg.wait_for_timeout(200)
    c = pg.inner_text('#count')
    eq('counts', c.split(' · ')[:3], [f'{len(code)} characters', '45 in $`…`', 'page: 3 elements, 42 characters of HTML'])
    pg.fill('#src', "x = 1\n$`div{`")
    pg.wait_for_function("document.querySelector('#status').textContent.includes('a { without }')")
    eq('errors show', 'a { without }' in pg.inner_text('#status'), True)
    pg.fill('#src', "$`ul>li*3{$}`")
    fr.locator('li').first.wait_for()
    with pg.expect_download() as d: pg.click('#export')
    html = open(d.value.path()).read()
    p2 = ctx.new_page(); p2.set_content(html)
    eq('export runs on its own', p2.evaluate("document.querySelectorAll('li').length"), 3)
    eq('no page errors', errs, [])
    b.close()
print(f'{ok} pass, {bad} fail'); sys.exit(1 if bad else 0)
