# python3 studio/web/test/page.py: the page words in a browser. Serves studio/web, runs
# programs into a blank page, clicks, types and reads what the page shows.
import functools, http.server, threading, sys, os, json
from playwright.sync_api import sync_playwright

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
srv = http.server.ThreadingHTTPServer(('127.0.0.1', 0), functools.partial(http.server.SimpleHTTPRequestHandler, directory=ROOT))
srv.RequestHandlerClass.log_message = lambda *a: None
threading.Thread(target=srv.serve_forever, daemon=True).start()
BASE = f'http://127.0.0.1:{srv.server_port}/'
ok = bad = 0
def eq(name, got, want):
    global ok, bad
    if got == want: ok += 1
    else:
        bad += 1
        print(f'--- {name}\n  want: {want!r}\n  got:  {got!r}')

with sync_playwright() as pw:
    b = pw.chromium.launch()
    pg = b.new_page()
    errs = []
    pg.on('pageerror', lambda e: errs.append(str(e)))
    def load(src):
        pg.goto(BASE + 'test/blank.html')
        pg.evaluate('''(src) => { window.out = []; window.errs = [];
          window.cur = S.runS(src, { write: (s) => out.push(s), onError: (e) => errs.push(e.msg), storage: null }) }''', src)
    body = lambda: pg.evaluate('document.body.innerText').strip()
    serr = lambda: pg.evaluate('errs')

    load(open(os.path.join(ROOT, 'examples/counter.s')).read())
    eq('counter start', pg.inner_text('h1'), '0')
    pg.click('text=more'); pg.click('text=more')
    eq('counter more', pg.inner_text('h1'), '2')
    pg.click('text=less')
    eq('counter less', pg.inner_text('h1'), '1')
    eq('counter: only the h1 is made again', pg.evaluate('cur.dom.D.regions.map(r => r.el.tagName).join()'), 'H1')

    load(open(os.path.join(ROOT, 'examples/site.s')).read())
    eq('site home', pg.inner_text('h1'), 'Pages described in s')
    eq('site cards', pg.evaluate("document.querySelectorAll('article.card h3').length"), 3)
    pg.click('nav >> text=Counter')
    pg.wait_for_function("document.querySelector('h1').textContent == 'Counter'")
    pg.click('text=+1'); pg.click('text=+1'); pg.click('text=+1')
    eq('site counter', pg.inner_text('p.big'), '3')
    pg.click('text=reset')
    eq('site reset', pg.inner_text('p.big'), '0')
    pg.click('nav >> text=List')
    pg.wait_for_function("document.querySelector('h1').textContent == 'A list that stays'")
    pg.fill('input', 'bread')
    pg.click('text=Add')
    pg.fill('input', 'milk')
    pg.keyboard.press('Tab')
    pg.click('text=Add')
    eq('site list', pg.evaluate("[...document.querySelectorAll('li')].map(l => l.textContent)"), ['bread', 'milk'])
    eq('site input emptied', pg.input_value('input'), '')
    pg.click('text=Clear')
    eq('site cleared', pg.evaluate("document.querySelectorAll('li').length"), 0)

    load('{Hello} p.lead  {x} div#main  {{inner} span} section  {link} a  {#/a} href= {go} a')
    eq('bare tags', pg.evaluate("document.body.innerHTML.replace(/<script.*?<\\/script>/s,'').trim()"),
       '<p class="lead">Hello</p> <div id="main">x</div> <section><span>inner</span></section> <a>link</a> <a href="#/a">go</a>')
    load('std/all.s. {inside} div {another} div')
    eq('std div: page code gets the element', pg.evaluate("document.querySelectorAll('div').length"), 2)
    eq('std div: std bodies keep theirs', serr(), [])
    load('std/all.s. 10 4 \\- =~ 2 lcm >  {after} p')
    eq('std words using div still divide', pg.evaluate('out.join("")'), '\n 6')
    load('{3} \\p n  {~p~} p')
    eq('backslash names a name', pg.inner_text('p'), '3')

    # two-way and attribute binding
    load('{} who` {~who~} value= input  {Hello ~who~} p  {~who~} title= {box} div')
    pg.fill('input', 'Ann')
    eq('two-way text', pg.inner_text('p'), 'Hello Ann')
    eq('attribute follows', pg.get_attribute('div', 'title'), 'Ann')
    load('0 on#  {~on~} checked= {checkbox} type= input  {on is ~on~} p')
    pg.check('input')
    eq('two-way checkbox', pg.inner_text('p'), 'on is 1')
    # regions: nested, each, refresh
    load('0 a# 0 b#  {{~a~} span.a {~b~} span.b} div  {a;} onclick {A} button  {b;} onclick {B} button')
    pg.click('text=B')
    eq('only the inner region runs', pg.evaluate("[document.querySelector('.a').textContent, document.querySelector('.b').textContent]"), ['0', '1'])
    load('{x} xs l  {xs {{~it~} li} each} ul  {{~xs~ y} xs l} onclick {add} button')
    pg.click('text=add')
    eq('each follows its list', pg.evaluate("document.querySelectorAll('li').length"), 2)
    # lanes and try in a page
    load('{hi} msg` ⟪{~msg~} h1¦{~msg~} h2¦{~msg~} p⟫ ⨝')
    eq('lanes make elements in lane order', pg.evaluate("[...document.body.children].filter(e=>e.tagName!='SCRIPT').map(e => e.tagName).join()"), 'H1,H2,P')
    load('{{nope.} {{failed: ~err~} p.err} try} section')
    eq('try in a page', pg.inner_text('p.err'), "failed: nope.: no script or file named 'nope'")
    load('{Fill in the form.} p {see main.} p')
    eq('prose ending with a period stays prose', pg.evaluate("document.querySelectorAll('p').length"), 2)
    eq('no page errors', errs, [])
    b.close()
print(f'{ok} pass, {bad} fail')
sys.exit(1 if bad else 0)
