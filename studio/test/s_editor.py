# python3 studio/test/s_editor.py: the editor at / against a running s3-studio: files and tabs,
# runs with output beside the lines, definitions and uses, completion, the palette, find and
# replace, arguments and stdin, the console, the Rust, builds, and how each fork ran.
import os, sys, subprocess, tempfile, time, socket
from playwright.sync_api import sync_playwright

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BIN = os.path.join(REPO, 'target', 'release', 's3-studio')
files = tempfile.mkdtemp(prefix='s-ed-')
os.makedirs(os.path.join(files, 'lib'))
open(os.path.join(files, 'lib', 'util.s'), 'w').write('{2 \\* =~} double$\n')
MAIN = 'std/core.s.\nstd/check.s.\nlib/util.s.\n\n3 double.. >\n4 5 add >\n2 2 add 5 expect\n2 2 add 4 expect\n'
open(os.path.join(files, 'main.s'), 'w').write(MAIN)
s = socket.socket(); s.bind(('127.0.0.1', 0)); port = s.getsockname()[1]; s.close()
srv = subprocess.Popen([BIN, '--port', str(port), '--files', files], cwd=REPO, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
for _ in range(100):
    try:
        socket.create_connection(('127.0.0.1', port), 0.2).close(); break
    except OSError: time.sleep(0.1)
ok = bad = 0
def eq(name, got, want):
    global ok, bad
    if got == want: ok += 1
    else:
        bad += 1
        print(f'--- {name}\n  want: {want!r}\n  got:  {got!r}')
try:
    with sync_playwright() as pw:
        b = pw.chromium.launch()
        pg = b.new_page(viewport={'width': 1400, 'height': 860})
        errs = []
        pg.on('pageerror', lambda e: errs.append(str(e)))
        pg.on('dialog', lambda d: d.accept())
        pg.goto(f'http://127.0.0.1:{port}/')
        pg.wait_for_selector('#list div')
        names = pg.evaluate("[...document.querySelectorAll('#list div')].map(d => d.title)")
        eq('the folder and std are listed', [n in names for n in ['lib/', 'lib/util.s', 'main.s', 'std/', 'std/core.s']], [True] * 5)
        val = lambda: pg.evaluate('ed.value')
        tabs = lambda: pg.evaluate("[...document.querySelectorAll('#tabs .tab')].map(t => t.title)")
        def caret_on(word, nth=0):
            pg.evaluate('([w, n]) => { let i = -1; for (let k = 0; k <= n; k++) i = ed.value.indexOf(w, i + 1); ed.goTo(i + 1) }', [word, nth])
        # opening with the picker
        pg.keyboard.press('Control+p'); pg.keyboard.type('main'); pg.keyboard.press('Enter')
        pg.wait_for_function("ed.value.includes('double')")
        eq('opened in a tab', tabs(), ['main.s'])
        # a run: what each line printed stands at its left, a failed check in red
        pg.keyboard.press('Control+Enter')
        pg.wait_for_selector('.ed-lines .note')
        notes = pg.evaluate("[...document.querySelectorAll('.ed-lines .note')].map(d => [d.textContent, d.classList.contains('fail')])")
        eq('output beside the lines', notes, [['6', False], ['9', False], ['fail got 4 want 5', True], ['ok', False]])
        eq('line numbers', pg.evaluate("document.querySelectorAll('.ed-lines .ln').length"), 9)
        pg.wait_for_function("document.querySelectorAll('.ed-doc .k-lib').length > 0", timeout=5000)
        eq('words as the machine reads them', pg.evaluate("[...document.querySelectorAll('.ed-doc .k-lib')].map(s => s.textContent)"), ['add', 'add', 'expect', 'add', 'expect'])
        # what a word is
        caret_on('add')
        r = pg.evaluate("(() => { const r = ed.charRect(ed.b); return [r.left + 2, r.top + r.height / 2] })()")
        pg.mouse.move(r[0], r[1]); pg.wait_for_selector('#tip:not([hidden])')
        eq('the tip names the std file', 'std/core.s' in pg.inner_text('#tip'), True)
        pg.mouse.move(5, 500)
        # the definition: in a loaded file, and back
        caret_on('double..')
        pg.keyboard.press('F12')
        pg.wait_for_function("document.querySelector('#tabs .tab.on').title == 'lib/util.s'")
        eq('the definition in the loaded file', pg.evaluate('ed.selected()'), 'double$')
        pg.keyboard.press('Alt+ArrowLeft')
        pg.wait_for_function("document.querySelector('#tabs .tab.on').title == 'main.s'")
        eq('back to the word', pg.evaluate("(() => { const [a, b] = ed.wordAt(ed.b); return ed.value.slice(a, b) })()"), 'double..')
        # in std, read-only
        caret_on('add')
        pg.keyboard.press('F12')
        pg.wait_for_function("document.querySelector('#tabs .tab.on').title.startsWith('std/core.s')")
        eq('the std definition', pg.evaluate('ed.selected()'), 'add')
        before = val(); pg.keyboard.type('xyz')
        eq('std is read-only', val(), before)
        pg.keyboard.press('Alt+w')
        eq('Alt+W closes the tab', tabs(), ['main.s', 'lib/util.s'])
        pg.click('#tabs .tab[title="main.s"]')
        # uses and the outline
        caret_on('add')
        pg.keyboard.press('Shift+F12')
        eq('the uses', pg.evaluate("document.querySelectorAll('#flist div').length"), 3)
        eq('the uses are marked', pg.evaluate("document.querySelectorAll('.ed .mark.use').length"), 3)
        pg.keyboard.press('Escape')
        # completion
        pg.evaluate("ed.goTo(ed.value.length)")
        pg.keyboard.type('1 1 exp')
        pg.wait_for_selector('#comp:not([hidden])')
        eq('completion offers expect', pg.evaluate("[...document.querySelectorAll('#comp div')].map(d => d.firstChild.textContent).slice(0, 2)"), ['expect', 'expectn'])
        pg.keyboard.press('Tab')
        eq('Tab takes it', val().endswith('1 1 expect'), True)
        # the palette
        pg.keyboard.press('F2')
        pg.click('#palette button:text-is(">")')
        eq('a palette word goes in at the caret', val().endswith('1 1 expect >'), True)
        pg.keyboard.press('F2')
        # find and replace, undone
        pg.keyboard.press('Control+f'); pg.keyboard.type('expect')
        eq('three found', pg.inner_text('#qn').endswith('of 3'), True)
        pg.fill('#rep', 'expectn'); pg.click('#rall')
        eq('replace all', val().count('expectn'), 3)
        pg.click('#qclose'); pg.keyboard.press('Control+z')
        eq('one undo takes it back', val().count('expectn'), 0)
        # save, and the file on disk
        pg.keyboard.press('Control+s')
        pg.wait_for_function("!document.querySelector('#tabs .tab.on').classList.contains('dirty')")
        eq('saved', open(os.path.join(files, 'main.s')).read(), val())
        # the console runs s with the file's definitions
        pg.keyboard.press('Control+`'); pg.keyboard.type('21 double.. >'); pg.keyboard.press('Enter')
        pg.wait_for_function("document.querySelector('#out').textContent.includes('42')")
        eq('the console', '› 21 double.. >' in pg.inner_text('#out'), True)
        # a new file with arguments and stdin
        pg.keyboard.press('Control+p'); pg.keyboard.type('io.s'); pg.keyboard.press('Enter')
        pg.wait_for_function("document.querySelector('#tabs .tab.on') && document.querySelector('#tabs .tab.on').title == 'io.s'")
        pg.evaluate("ed.focus()")
        pg.keyboard.insert_text('args 0 a [l]  a~ argc~ >\n<_ _~ >\n<_ _~ eof~ >')
        pg.click('#rtabs button[data-tab=input]')
        pg.fill('#args', "hello 'two words'"); pg.fill('#stdin', 'line one')
        pg.evaluate("ed.focus()")
        pg.keyboard.press('Control+Enter')
        pg.wait_for_function("document.querySelector('#out').textContent.includes('line one')")
        eq('arguments and stdin read', [x.strip() for x in pg.inner_text('#out').split('\n')[1:]], ['hello 2', 'line one', '1'])
        # the Rust and a build, with how each fork ran
        pg.evaluate("ed.value = '{1 2 3 4 5 6 7 8} xs /l\\n⟪xs…⟫ 2 \\\\* =~ 1 \\\\+ =~ ⨝ ys into ys o l/ o~ >\\n⟪{a} >¦{b} >⟫ ⨝'; ed.goTo(0)")
        pg.uncheck('#fold'); pg.evaluate('ed.focus()')
        pg.keyboard.press('Control+r')
        pg.wait_for_function("document.querySelector('#rs').textContent.includes('fn main')")
        eq('the Rust', 'kb0' in pg.inner_text('#rs'), True)
        pg.keyboard.press('Control+Enter')
        pg.wait_for_function("document.querySelectorAll('#forks div').length >= 2")
        hows = pg.evaluate("[...document.querySelectorAll('#forks div')].map(d => d.textContent)")
        eq('a vector program on the machine', 'machine kernels' in hows[0], True)
        eq('threads', 'threads' in hows[1], True)
        eq('join lines', pg.evaluate("document.querySelector('.joinres').textContent"), '8 lanes · 3  ¦  5  ¦  7  ¦  9  ¦  11  ¦  13  ¦  15  ¦  17')
        pg.keyboard.press('Control+Shift+Enter')
        pg.wait_for_function("document.querySelector('#out').textContent.includes('compiled')", timeout=300000)
        eq('compiled output', pg.inner_text('#out').split('\n')[2:5], [' 3 5 7 9 11 13 15 17', ' a', ' b'])
        eq('compiled kernel', 'compiled kernel' in pg.evaluate("document.querySelector('#forks div').textContent"), True)
        print('build:', pg.inner_text('#stats'), '·', pg.inner_text('#out').split('\n')[1])
        # folded: the same output, done at build time
        pg.check('#fold'); pg.evaluate('ed.focus()')
        pg.keyboard.press('Control+Shift+Enter')
        pg.wait_for_function("document.querySelector('#out .note') && document.querySelector('#out .note').textContent.includes('folded')", timeout=300000)
        eq('folded whole', 'folded whole' in pg.inner_text('#out .note'), True)
        eq('folded output', pg.inner_text('#out').split('\n')[2:5], [' 3 5 7 9 11 13 15 17', ' a', ' b'])
        eq('no forks run in a folded program', pg.evaluate("document.querySelectorAll('#forks div').length"), 0)
        # an error goes to its place
        pg.evaluate("ed.value = '1 >\\nnope.'; ed.goTo(0)")
        pg.keyboard.press('Control+Enter')
        pg.wait_for_function("document.querySelector('#where').classList.contains('err')")
        pg.click('#where')
        eq('error place', pg.evaluate('ed.b'), 4)
        eq('error beside its line', pg.evaluate("document.querySelector('.ed-lines .note.err').textContent.startsWith('× ')"), True)
        # zoom
        pg.keyboard.press('Control+Equal')
        eq('zoom', pg.evaluate("getComputedStyle(document.querySelector('#ed')).getPropertyValue('--fs')"), '15px')
        pg.keyboard.press('Control+0')
        # a change on disk comes back into a tab with nothing unsaved
        pg.click('#tabs .tab[title="main.s"]')
        open(os.path.join(files, 'main.s'), 'w').write('{changed} >\n')
        pg.evaluate("dispatchEvent(new Event('focus'))")
        pg.wait_for_function("ed.value == '{changed} >\\n'")
        eq('changed on disk', val(), '{changed} >\n')
        # the room is still there
        pg.goto(f'http://127.0.0.1:{port}/room.html')
        pg.wait_for_selector('canvas#view')
        eq('the room', pg.evaluate("!!document.querySelector('canvas#view')"), True)
        eq('no page errors', errs, [])
        b.close()
finally:
    srv.terminate()
print(f'{ok} pass, {bad} fail')
sys.exit(1 if bad else 0)
