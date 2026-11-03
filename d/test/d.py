# python3 d/test/d.py: d.js in a browser — the HTML each abbreviation builds, and the live parts.
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
        bad += 1
        print(f'--- {name}\n  want: {want!r}\n  got:  {got!r}')

HTML = [
    ('repeat with a count', "$`ul>li.item$*3{Item $}`", '<ul><li class="item1">Item 1</li><li class="item2">Item 2</li><li class="item3">Item 3</li></ul>'),
    ('siblings and attributes', "$`nav>a[href=#/]{Home}+a[href=#/about title=\"the page\"]{About}`", '<nav><a href="#/">Home</a><a href="#/about" title="the page">About</a></nav>'),
    ('implied tags', "$`table>.r*2>.c*2{$}`", '<table><tr class="r"><td class="c">1</td><td class="c">2</td></tr><tr class="r"><td class="c">1</td><td class="c">2</td></tr></table>'),
    ('climb', "$`div>p{a}^p{b}`.childNodes.length", 2),
    ('climb html', "(f => { const d = document.createElement('i'); d.append(f); return d.innerHTML })($`div>p{a}^p{b}`)", '<div><p>a</p></div><p>b</p>'),
    ('groups', "(f => { const d = document.createElement('i'); d.append(f); return d.innerHTML })($`(header>h1{T})+main>p{x}`)", '<header><h1>T</h1></header><main><p>x</p></main>'),
    ('id and default div', "$`#app.box.wide{hi}`", '<div class="box wide" id="app">hi</div>'),
    ('items and fields', "$`ul>li*${[{n:'x',u:'/x'},{n:'y',u:'/y'}]}>a[href=$.u]{$.n}`", '<ul><li><a href="/x">x</a></li><li><a href="/y">y</a></li></ul>'),
    ('array items', "$`ol>li*${['a','b']}{$!}`", '<ol><li>a!</li><li>b!</li></ol>'),
    ('slot as children', "$`div>${['a', $`b{x}`, 3]}`", '<div>a<b>x</b>3</div>'),
    ('text with values', "$`p{${'one'} and ${2}}`", '<p>one and 2</p>'),
    ('object attributes', "$`input${{type:'checkbox', checked:true, placeholder:'p'}}`.outerHTML", '<input type="checkbox" placeholder="p">'),
    ('a dollar sign', "$`p{costs $$5}`", '<p>costs $5</p>'),
    ('main attributes', "(f => { const d = document.createElement('i'); d.append(f); return d.innerHTML })($`a:#/about{About}+img:pic.png+input:email@m+form:/send+label:m{Mail}`)", '<a href="#/about">About</a><img src="pic.png"><input type="email" name="m"><form action="/send"></form><label for="m">Mail</label>'),
    ('main attribute from a slot', "$`a:${'/x?y=1'}{x}`", '<a href="/x?y=1">x</a>'),
    ('void and nesting', "$`form>label{Name}+input[name=n]+button{Go}`", '<form><label>Name</label><input name="n"><button>Go</button></form>'),
]

with sync_playwright() as pw:
    b = pw.chromium.launch()
    pg = b.new_page()
    errs = []
    pg.on('pageerror', lambda e: errs.append(str(e)))
    def fresh():
        pg.goto(f'http://127.0.0.1:{srv.server_port}/test/blank.html')
        pg.wait_for_function('window.ready')
    fresh()
    for name, expr, want in HTML:
        got = pg.evaluate(f"(() => {{ const r = {expr}; return r instanceof Node ? (r.outerHTML ?? r.textContent) : r }})()")
        eq(name, got, want)

    # a counter: a signal as text, a function as the click
    pg.evaluate("window.n = $(0); $.at(document.body, $`div>h1${n}+button{more}${() => n(x => x + 1)}+button{less}${() => n(x => x - 1)}`)")
    pg.click('text=more'); pg.click('text=more'); pg.click('text=less')
    eq('counter', pg.inner_text('h1'), '1')
    # an input holds a signal; text follows it
    pg.evaluate("window.who = $(''); $.at(document.body, $`input.who${who}+p.hi{Hello ${who}}`)")
    pg.fill('input.who', 'Ann')
    eq('two-way', [pg.inner_text('p.hi'), pg.evaluate('who()')], ['Hello Ann', 'Ann'])
    pg.evaluate("who('Bo')")
    eq('signal into input', pg.input_value('input.who'), 'Bo')
    # a number stays a number
    pg.evaluate("window.k = $(3); $.at(document.body, $`input.k${k}`)")
    pg.fill('input.k', '7')
    eq('number input', pg.evaluate('typeof k() + k()'), 'number7')
    # checkbox
    pg.evaluate("window.on = $(false); $.at(document.body, $`input.cb[type=checkbox]${on}+span.st{${() => on() ? 'on' : 'off'}}`)")
    pg.check('input.cb')
    eq('checkbox', [pg.evaluate('on()'), pg.inner_text('span.st')], [True, 'on'])
    # a live list keeps the elements of items that stay
    pg.evaluate("window.todos = $(['a', 'b']); window.picked = null; $.at(document.body, $`ul.todo>li*${todos}{$}${(e, it) => picked = it}`); window.first = document.querySelector('ul.todo li')")
    pg.evaluate("todos(t => [...t, 'c'])")
    eq('list grows', pg.evaluate("[...document.querySelectorAll('ul.todo li')].map(l => l.textContent)"), ['a', 'b', 'c'])
    eq('items that stay keep their elements', pg.evaluate("document.querySelector('ul.todo li') === first"), True)
    pg.click('ul.todo li:nth-child(2)')
    eq('a handler gets its item', pg.evaluate('picked'), 'b')
    pg.evaluate("todos(t => t.filter(x => x !== 'a'))")
    eq('list shrinks', pg.evaluate("[...document.querySelectorAll('ul.todo li')].map(l => l.textContent)"), ['b', 'c'])
    # live attributes and classes
    pg.evaluate("window.f = $(false); $.at(document.body, $`div.live[title=${() => 'n is ' + n()}]${{class: {on: f}}}{x}`)")
    pg.evaluate("n(9); f(true)")
    eq('live attribute', pg.get_attribute('div.live', 'title'), 'n is 9')
    eq('class toggle', pg.evaluate("document.querySelector('div.live').classList.contains('on')"), True)
    # regions
    pg.evaluate("$.at(document.body, $`div.reg>${() => n() > 10 ? $`b{big}` : 'small'}`)")
    eq('region', pg.inner_html('div.reg'), 'small')
    pg.evaluate("n(11)")
    eq('region again', pg.inner_html('div.reg'), '<b>big</b>')
    # refs
    eq('refs', pg.evaluate("$`div>input@name+button@go{go}`.$.name.tagName"), 'INPUT')
    # forms: a function on a form is its submit, and the page stays
    pg.evaluate("window.sent = null; $.at(document.body, $`form.f${(e, v) => sent = v}>input@who+input:email@mail+button{send}`)")
    pg.fill('form.f input[name=who]', 'Ann'); pg.fill('form.f input[name=mail]', 'a@b.c')
    pg.click('form.f button')
    eq('submit gives the values', pg.evaluate('sent'), {'who': 'Ann', 'mail': 'a@b.c'})
    eq('no navigation', pg.evaluate('window.ready'), 1)
    # events by name
    pg.evaluate("window.keys = ''; $.at(document.body, $`input.kd[onkeydown=${e => keys += e.key}]`)")
    pg.press('input.kd', 'x')
    eq('event by name', pg.evaluate('keys'), 'x')
    # effects follow only what they read; effects of a region go with it
    pg.evaluate("window.runs = 0; window.a = $(1); window.bb = $(1); $.fx(() => { a(); runs++ }); bb(2); a(2)")
    eq('effects', pg.evaluate('runs'), 2)
    pg.evaluate("window.inner = 0; window.sw = $(true); window.x = $(0); $.at(document.body, $`div>${() => sw() ? $`p{${() => (inner++, x())}}` : 'none'}`); sw(false); x(5)")
    eq('effects go with their region', pg.evaluate('inner'), 1)
    # a computed signal
    eq('computed', pg.evaluate("(() => { const s = $(2); const d = $.c(() => s() * 10); s(3); return d() })()"), 30)
    # css
    pg.evaluate("window.col = $('rgb(255, 0, 0)'); $.css`.paint { color: ${col} }`; $.at(document.body, $`p.paint{red}`)")
    pg.evaluate("col('rgb(0, 0, 255)')")
    eq('live css', pg.evaluate("getComputedStyle(document.querySelector('.paint')).color"), 'rgb(0, 0, 255)')
    # keep
    pg.evaluate("localStorage.clear(); window.kk = $.keep('count', $(0)); kk(4)")
    fresh()
    eq('kept', pg.evaluate("$.keep('count', $(0))()"), 4)
    # routes
    pg.evaluate("$.at(document.body, $`main>${() => ({ '': $`h2{home}`, about: $`h2{about}` })[$.hash()] || $`h2{lost}`}`)")
    eq('route home', pg.inner_text('main h2'), 'home')
    pg.evaluate("location.hash = 'about'")
    pg.wait_for_function("document.querySelector('main h2').textContent == 'about'")
    eq('route about', pg.inner_text('main h2'), 'about')
    # errors name the place
    eq('error', pg.evaluate("(() => { try { $`div{x`; return 'none' } catch (e) { return e.message.split(' at ')[0] } })()"), '$: a { without }')

    # how many characters of HTML a character of template makes
    print('characters: template → HTML')
    for expr in ["$`ul>li.item$*3{Item $}`", "$`table>tr*10>td*10{$}`", "$`nav>a:#/{Home}+a:#/about{About}+a:#/list{List}`", "$`form>label:n{Name}+input@n+label:m{Mail}+input:email@m+button{Send}`"]:
        t = expr[2:-1]
        h = pg.evaluate(f"{expr}.outerHTML")
        print(f'  {len(t):>4} → {len(h):>5}  ×{len(h)/len(t):.1f}  {t}')
    eq('no page errors', errs, [])
    b.close()
print(f'{ok} pass, {bad} fail')
sys.exit(1 if bad else 0)
