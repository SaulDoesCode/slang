// Windows: every panel moves by its strip, folds by a double-click, snaps to the screen
// edges and to other windows, and flips when docked at the bottom so its strip stays
// at the edge. The output also sets its height by its free edge. Layout is kept per browser.

const SNAP = 12
const KEY = 's3.layout'

function load() {
  try { return JSON.parse(localStorage.getItem(KEY) || '{}') } catch { return {} }
}
function save(l) {
  try { localStorage.setItem(KEY, JSON.stringify(l)) } catch {}
}

export function resetLayout() {
  try { localStorage.removeItem(KEY) } catch {}
  location.reload()
}

function rectOf(el) {
  const r = el.getBoundingClientRect()
  return { l: r.left, t: r.top, r: r.right, b: r.bottom, w: r.width, h: r.height }
}

// fix the window to explicit left, top, width, height so it can move freely
function pin(el) {
  const r = rectOf(el)
  el.style.left = r.l + 'px'
  el.style.top = r.t + 'px'
  el.style.right = 'auto'
  el.style.bottom = 'auto'
  if (el.id !== 'bar') {
    el.style.width = r.w + 'px'
    if (!el.classList.contains('collapsed')) el.style.height = r.h + 'px'
  } else {
    el.style.width = r.w + 'px'
  }
  el.classList.add('placed')
}

function snap(el, x, y) {
  const me = rectOf(el)
  const W = innerWidth, H = innerHeight
  const others = [...document.querySelectorAll('.win')].filter((o) => o !== el && !o.hidden).map(rectOf)
  let nx = x, ny = y
  const xs = [[0, 10], [W - me.w, W - me.w - 10]]
  const ys = [[0, 10], [H - me.h, H - me.h - 10]]
  for (const o of others) {
    xs.push([o.r, o.r + 6], [o.l - me.w, o.l - me.w - 6], [o.l, o.l])
    ys.push([o.b, o.b + 6], [o.t - me.h, o.t - me.h - 6], [o.t, o.t])
  }
  for (const [edge, to] of xs) if (Math.abs(x - edge) < SNAP || Math.abs(x - to) < SNAP) { nx = to; break }
  for (const [edge, to] of ys) if (Math.abs(y - edge) < SNAP || Math.abs(y - to) < SNAP) { ny = to; break }
  return [Math.max(0, Math.min(W - 40, nx)), Math.max(0, Math.min(H - 24, ny))]
}

function dockState(el) {
  const r = rectOf(el)
  el.classList.toggle('flip', innerHeight - r.b < 30 && r.t > innerHeight / 3)
}

export function initWindows(onChange = () => {}) {
  const layout = load()
  const wins = [...document.querySelectorAll('.win')]
  const record = (el) => {
    const r = rectOf(el)
    layout[el.dataset.win] = { x: r.l, y: r.t, w: r.w, h: el.classList.contains('collapsed') ? (layout[el.dataset.win] || {}).h : r.h, c: el.classList.contains('collapsed'), f: el.classList.contains('flip') }
    save(layout)
    onChange(el)
  }
  for (const el of wins) {
    const id = el.dataset.win
    const st = layout[id]
    if (st) {
      el.style.left = st.x + 'px'
      el.style.top = st.y + 'px'
      el.style.right = 'auto'
      el.style.bottom = 'auto'
      if (st.w) el.style.width = st.w + 'px'
      if (st.h && id !== 'bar') el.style.height = st.h + 'px'
      el.classList.toggle('collapsed', !!st.c)
      el.classList.toggle('flip', !!st.f)
      el.classList.add('placed')
    }
    const strip = el.querySelector('.strip')
    if (!strip) continue
    el.addEventListener('mousedown', () => {
      for (const o of wins) o.style.zIndex = ''
      el.style.zIndex = '7'
    })
    strip.addEventListener('mousedown', (e) => {
      if (e.button !== 0 || e.target.closest('button, input, select, label')) return
      e.preventDefault()
      pin(el)
      const r = rectOf(el)
      const dx = e.clientX - r.l, dy = e.clientY - r.t
      const move = (m) => {
        const [x, y] = snap(el, m.clientX - dx, m.clientY - dy)
        el.style.left = x + 'px'
        el.style.top = y + 'px'
        dockState(el)
      }
      const up = () => {
        removeEventListener('mousemove', move)
        removeEventListener('mouseup', up)
        record(el)
      }
      addEventListener('mousemove', move)
      addEventListener('mouseup', up)
    })
    const fold = () => {
      if (el.id === 'bar') return
      if (!el.classList.contains('collapsed')) pin(el)
      el.classList.toggle('collapsed')
      if (!el.classList.contains('collapsed') && layout[id] && layout[id].h) el.style.height = layout[id].h + 'px'
      record(el)
    }
    strip.addEventListener('dblclick', (e) => { if (!e.target.closest('button, input, select')) fold() })
    const fb = el.querySelector('.strip .fold')
    if (fb) fb.addEventListener('click', fold)
  }
  // the output's free edge sets its height
  const con = document.getElementById('console')
  const rz = con && con.querySelector('.rz')
  if (rz) {
    rz.addEventListener('mousedown', (e) => {
      e.preventDefault()
      pin(con)
      const r0 = rectOf(con)
      const flip = con.classList.contains('flip')
      const move = (m) => {
        if (flip) {
          const top = Math.max(0, Math.min(r0.b - 60, m.clientY))
          con.style.top = top + 'px'
          con.style.height = (r0.b - top) + 'px'
        } else {
          con.style.height = Math.max(60, m.clientY - r0.t) + 'px'
        }
      }
      const up = () => {
        removeEventListener('mousemove', move)
        removeEventListener('mouseup', up)
        record(con)
      }
      addEventListener('mousemove', move)
      addEventListener('mouseup', up)
    })
  }
  return { record }
}

// the console drops over the scene from its docked edge, and goes back
let dropped = null
export function dropConsole() {
  const con = document.getElementById('console')
  pin(con)
  const r = rectOf(con)
  const flip = con.classList.contains('flip')
  if (dropped) {
    con.style.top = dropped.t + 'px'
    con.style.height = dropped.h + 'px'
    dropped = null
    return false
  }
  dropped = { t: r.t, h: r.h }
  const h = Math.round(innerHeight * 0.55)
  con.classList.remove('collapsed')
  if (flip || r.t > innerHeight / 2) con.style.top = (r.b - h) + 'px'
  con.style.height = h + 'px'
  return true
}
