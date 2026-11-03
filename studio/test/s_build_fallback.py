# python3 studio/test/s_build_fallback.py: the build button runs `s3 build` beside the server,
# so a broken or stub runtime library beside it changes nothing.
import os, sys, json, shutil, subprocess, tempfile, time, socket, urllib.request

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
here = tempfile.mkdtemp(prefix='s-fallback-')
os.makedirs(os.path.join(here, 'deps'))
exe = os.path.join(here, 's3-studio')
shutil.copy(os.path.join(REPO, 'target', 'release', 's3-studio'), exe)
shutil.copy(os.path.join(REPO, 'target', 'release', 's3'), os.path.join(here, 's3'))
open(os.path.join(here, 'libs3_core.rlib'), 'wb').write(b'not a library')
files = tempfile.mkdtemp(prefix='s-files-')
s = socket.socket(); s.bind(('127.0.0.1', 0)); port = s.getsockname()[1]; s.close()
srv = subprocess.Popen([exe, '--port', str(port), '--files', files], cwd=REPO, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
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
    req = urllib.request.Request(f'http://127.0.0.1:{port}/api', data=json.dumps({'cmd': 'build', 'text': '<_ _~ > 1 2 + =~ >', 'path': 'x.s', 'stdin': 'hey\n', 'ms': 60000}).encode())
    j = json.load(urllib.request.urlopen(req, timeout=600))
    eq('it builds and runs', j.get('out'), '\n hey\n 3')
    eq('it says how', j.get('how', '').startswith('s3 build'), True)
    eq('no library error', 'did not link' in j.get('how', '') or 'metadata' in json.dumps(j), False)
    print('how:', j.get('how'), '· built in', round(j.get('build_ms', 0) / 1000, 1), 's')
finally:
    srv.terminate()
print(f'{ok} pass, {bad} fail')
sys.exit(1 if bad else 0)
