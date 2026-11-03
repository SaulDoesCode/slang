// Curve text: glyph outlines as quadratic curves in a float texture, coverage computed per
// pixel from the curves (the kuis.html renderer). Two uses: labels that face the screen,
// and words lying on brick faces.
import * as THREE from '../vendor/three.module.js'
import opentype from '../vendor/opentype.module.js'

const CW = 2048, CH = 128
const curveData = new Float32Array(CW * CH * 4)
export const curveTex = new THREE.DataTexture(curveData, CW, CH, THREE.RGBAFormat, THREE.FloatType)
curveTex.minFilter = curveTex.magFilter = THREE.NearestFilter
curveTex.generateMipmaps = false
curveTex.needsUpdate = true
let curveCount = 0
const glyphs = new Map()
let fonts = []

export async function loadFonts(urls) {
  fonts = []
  for (const u of urls) {
    const r = await fetch(u)
    if (!r.ok) throw new Error(u + ': HTTP ' + r.status)
    fonts.push(opentype.parse(await r.arrayBuffer()))
  }
}

function outline(og, upm) {
  const out = []
  let sx = 0, sy = 0, cx = 0, cy = 0
  const q = (a, b, c, d, e, f) => out.push([a / upm, b / upm, c / upm, d / upm, e / upm, f / upm])
  const cubic = (x0, y0, x1, y1, x2, y2, x3, y3, depth) => {
    if (depth > 0) {
      const m = (a, b) => (a + b) / 2
      const x01 = m(x0, x1), y01 = m(y0, y1), x12 = m(x1, x2), y12 = m(y1, y2), x23 = m(x2, x3), y23 = m(y2, y3)
      const xa = m(x01, x12), ya = m(y01, y12), xb = m(x12, x23), yb = m(y12, y23)
      const xm = m(xa, xb), ym = m(ya, yb)
      cubic(x0, y0, x01, y01, xa, ya, xm, ym, depth - 1)
      cubic(xm, ym, xb, yb, x23, y23, x3, y3, depth - 1)
    } else {
      q(x0, y0, (3 * (x1 + x2) - x0 - x3) / 4, (3 * (y1 + y2) - y0 - y3) / 4, x3, y3)
    }
  }
  for (const c of og.path.commands) {
    if (c.type === 'M') { sx = cx = c.x; sy = cy = c.y }
    else if (c.type === 'L') { q(cx, cy, (cx + c.x) / 2, (cy + c.y) / 2, c.x, c.y); cx = c.x; cy = c.y }
    else if (c.type === 'Q') { q(cx, cy, c.x1, c.y1, c.x, c.y); cx = c.x; cy = c.y }
    else if (c.type === 'C') { cubic(cx, cy, c.x1, c.y1, c.x2, c.y2, c.x, c.y, 2); cx = c.x; cy = c.y }
    else if (c.type === 'Z') { if (cx !== sx || cy !== sy) q(cx, cy, (cx + sx) / 2, (cy + sy) / 2, sx, sy); cx = sx; cy = sy }
  }
  return out
}

export function glyph(ch) {
  let g = glyphs.get(ch)
  if (g) return g
  let font = fonts[0], og = font.charToGlyph(ch)
  for (const f of fonts) {
    const o = f.charToGlyph(ch)
    if (o && o.index !== 0) { font = f; og = o; break }
  }
  const upm = font.unitsPerEm
  const cv = outline(og, upm)
  const bb = og.getBoundingBox()
  g = { start: curveCount, count: cv.length, adv: (og.advanceWidth || upm * 0.6) / upm, x0: bb.x1 / upm, y0: bb.y1 / upm, x1: bb.x2 / upm, y1: bb.y2 / upm }
  if ((curveCount + cv.length) * 2 > CW * CH) { g.count = 0 } else {
    for (const k of cv) { curveData.set([k[0], k[1], k[2], k[3], k[4], k[5], 0, 0], curveCount * 8); curveCount++ }
    curveTex.needsUpdate = true
  }
  glyphs.set(ch, g)
  return g
}

export function measure(text) {
  let w = 0
  for (const ch of text) w += glyph(ch).adv
  return w
}

const FRAG_CORE = /* glsl */`
precision highp float;
precision highp sampler2D;
uniform sampler2D uCurves;
uniform vec3 uColor;
uniform float uOpacity;
varying vec2 vEm;
varying vec2 vCurve;
void ray(vec2 a, vec2 b, vec2 c, inout float cov, inout float wgt) {
  bool sa = a.y < 0.0; bool sb = b.y < 0.0; bool sc = c.y < 0.0;
  if (sa == sc && sb == sa) return;
  float A = a.y - 2.0 * b.y + c.y; float B = a.y - b.y; float C = a.y;
  float d = sqrt(max(B * B - A * C, 0.0));
  float m = B >= 0.0 ? B + d : B - d;
  float q = m == 0.0 ? 0.0 : C / m;
  float tp = B >= 0.0 ? m / A : q;
  float tm = B >= 0.0 ? q : m / A;
  float Ax = a.x - 2.0 * b.x + c.x; float Bx = a.x - b.x;
  if (sa != sc) {
    if (!sa) { float x = (Ax * tm - 2.0 * Bx) * tm + a.x; cov -= clamp(x + 0.5, 0.0, 1.0); wgt = max(wgt, clamp(1.0 - abs(x) * 2.0, 0.0, 1.0)); }
    else { float x = (Ax * tp - 2.0 * Bx) * tp + a.x; cov += clamp(x + 0.5, 0.0, 1.0); wgt = max(wgt, clamp(1.0 - abs(x) * 2.0, 0.0, 1.0)); }
  } else {
    float xp = (Ax * tp - 2.0 * Bx) * tp + a.x; float xm = (Ax * tm - 2.0 * Bx) * tm + a.x;
    cov += clamp(xp + 0.5, 0.0, 1.0) - clamp(xm + 0.5, 0.0, 1.0);
    wgt = max(wgt, max(clamp(1.0 - abs(xp) * 2.0, 0.0, 1.0), clamp(1.0 - abs(xm) * 2.0, 0.0, 1.0)));
  }
}
void main() {
  vec2 ppe = 1.0 / max(fwidth(vEm), vec2(1e-5));
  int W = textureSize(uCurves, 0).x;
  int start = int(floor(vCurve.x + 0.5));
  int count = int(floor(vCurve.y + 0.5));
  float xcov = 0.0, xwgt = 0.0, ycov = 0.0, ywgt = 0.0;
  for (int i = 0; i < 1024; i++) {
    if (i >= count) break;
    int idx = (start + i) * 2;
    vec4 t0 = texelFetch(uCurves, ivec2(idx % W, idx / W), 0);
    vec4 t1 = texelFetch(uCurves, ivec2((idx + 1) % W, (idx + 1) / W), 0);
    vec2 a = (t0.xy - vEm) * ppe; vec2 b = (t0.zw - vEm) * ppe; vec2 c = (t1.xy - vEm) * ppe;
    ray(a, b, c, xcov, xwgt);
    ray(a.yx, b.yx, c.yx, ycov, ywgt);
  }
  ycov = -ycov;
  float cov = max(abs(xcov * xwgt + ycov * ywgt) / max(xwgt + ywgt, 1.0 / 65536.0), min(abs(xcov), abs(ycov)));
  cov = clamp(cov, 0.0, 1.0) * uOpacity;
  if (cov < 0.003) discard;
  gl_FragColor = vec4(uColor, cov);
  #include <colorspace_fragment>
}`

// screen-facing labels: positioned at the mesh origin, laid out in view space
const LABEL_VERT = /* glsl */`
attribute vec2 aPos; attribute vec2 aCorner; attribute vec2 aOrigin; attribute vec2 aCurve;
uniform float uSize; uniform float uViewH; uniform float uTanHalf; uniform float uMinPx;
varying vec2 vEm; varying vec2 vCurve;
void main() {
  vec4 mv = viewMatrix * modelMatrix * vec4(0.0, 0.0, 0.0, 1.0);
  float depth = max(-mv.z, 1e-4);
  float pxPerWorld = uViewH * 0.5 / (uTanHalf * depth);
  if (uSize * pxPerWorld < uMinPx) { gl_Position = vec4(2.0, 2.0, 2.0, 1.0); return; }
  float emPerPx = 1.0 / (uSize * pxPerWorld);
  vec2 p = aPos + aCorner * emPerPx;
  vEm = p - aOrigin; vCurve = aCurve;
  mv.xy += p * uSize;
  gl_Position = projectionMatrix * mv;
}`

// words on faces: world-space quads
const FACE_VERT = /* glsl */`
attribute vec2 aEm; attribute vec2 aCurve;
varying vec2 vEm; varying vec2 vCurve;
void main() {
  vEm = aEm; vCurve = aCurve;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}`

export const shared = {
  uCurves: { value: curveTex },
  uViewH: { value: 800 },
  uTanHalf: { value: Math.tan(Math.PI / 6) },
  uMinPx: { value: 5 },
}

export const LINE = 1.25, CENTER = 0.34

/** A label that faces the screen. size = world height of one em. */
export function makeLabel(text, color = '#3b2d20', size = 0.4) {
  const mat = new THREE.ShaderMaterial({
    uniforms: { ...shared, uSize: { value: size }, uColor: { value: new THREE.Color(color) }, uOpacity: { value: 1 } },
    vertexShader: LABEL_VERT, fragmentShader: FRAG_CORE, transparent: true, depthWrite: false, side: THREE.DoubleSide,
  })
  const mesh = new THREE.Mesh(new THREE.BufferGeometry(), mat)
  mesh.frustumCulled = false
  mesh.renderOrder = 10
  mesh.userData.setText = (t) => { mesh.geometry.dispose(); mesh.geometry = labelGeometry(t); mesh.userData.text = t }
  mesh.userData.setText(text)
  return mesh
}

function labelGeometry(text) {
  const lines = String(text).split('\n'), n = lines.length
  const pos = [], corner = [], origin = [], curve = [], idx = []
  const top = n * LINE / 2
  lines.forEach((line, i) => {
    const w = measure(line)
    let pen = -w / 2
    const base = top - (i + 0.5) * LINE - CENTER
    for (const ch of line) {
      const g = glyph(ch)
      if (g.count) {
        const v = pos.length / 2
        const xs = [g.x0, g.x1, g.x1, g.x0], ys = [g.y0, g.y0, g.y1, g.y1]
        const cs = [-1, -1, 1, -1, 1, 1, -1, 1]
        for (let k = 0; k < 4; k++) {
          pos.push(pen + xs[k], base + ys[k]); corner.push(cs[k * 2], cs[k * 2 + 1]); origin.push(pen, base); curve.push(g.start, g.count)
        }
        idx.push(v, v + 1, v + 2, v, v + 2, v + 3)
      }
      pen += g.adv
    }
  })
  const geo = new THREE.BufferGeometry()
  geo.setAttribute('aPos', new THREE.BufferAttribute(new Float32Array(pos), 2))
  geo.setAttribute('aCorner', new THREE.BufferAttribute(new Float32Array(corner), 2))
  geo.setAttribute('aOrigin', new THREE.BufferAttribute(new Float32Array(origin), 2))
  geo.setAttribute('aCurve', new THREE.BufferAttribute(new Float32Array(curve), 2))
  geo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(pos.length / 2 * 3), 3))
  geo.setIndex(idx)
  return geo
}

export function faceMaterial(color = '#2e2216') {
  return new THREE.ShaderMaterial({
    uniforms: { uCurves: shared.uCurves, uColor: { value: new THREE.Color(color) }, uOpacity: { value: 1 } },
    vertexShader: FACE_VERT, fragmentShader: FRAG_CORE, transparent: true, depthWrite: false,
    polygonOffset: true, polygonOffsetFactor: -2, polygonOffsetUnits: -2,
  })
}

/**
 * Builds one geometry of words lying on planes. items: [{text, c:[x,y,z], r:[x,y,z], u:[x,y,z], size}]
 * c is the center of the word, r the reading direction, u up.
 */
export function faceGeometry(items) {
  const P = [], E = [], C = [], idx = []
  const pad = 0.06
  for (const it of items) {
    const w = measure(it.text)
    let pen = -w / 2
    const base = -CENTER
    const s = it.size
    for (const ch of it.text) {
      const g = glyph(ch)
      if (g.count) {
        const v = P.length / 3
        const xs = [g.x0 - pad, g.x1 + pad, g.x1 + pad, g.x0 - pad], ys = [g.y0 - pad, g.y0 - pad, g.y1 + pad, g.y1 + pad]
        for (let k = 0; k < 4; k++) {
          const ex = pen + xs[k], ey = base + ys[k]
          P.push(it.c[0] + (it.r[0] * ex + it.u[0] * ey) * s, it.c[1] + (it.r[1] * ex + it.u[1] * ey) * s, it.c[2] + (it.r[2] * ex + it.u[2] * ey) * s)
          E.push(xs[k], ys[k])
          C.push(g.start, g.count)
        }
        idx.push(v, v + 1, v + 2, v, v + 2, v + 3)
      }
      pen += g.adv
    }
  }
  const geo = new THREE.BufferGeometry()
  geo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(P), 3))
  geo.setAttribute('aEm', new THREE.BufferAttribute(new Float32Array(E), 2))
  geo.setAttribute('aCurve', new THREE.BufferAttribute(new Float32Array(C), 2))
  geo.setIndex(idx)
  geo.computeBoundingSphere()
  return geo
}

// Words in their own colors: the face geometry with a color per glyph, for code where
// each token carries its class.
const COLOR_VERT = /* glsl */`
attribute vec2 aEm; attribute vec2 aCurve; attribute vec3 aColor;
varying vec2 vEm; varying vec2 vCurve; varying vec3 vColor;
void main() {
  vEm = aEm; vCurve = aCurve; vColor = aColor;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}`
const COLOR_FRAG = FRAG_CORE.replace('uniform vec3 uColor;', 'varying vec3 vColor;').replace('gl_FragColor = vec4(uColor, cov);', 'gl_FragColor = vec4(vColor, cov);')

export function colorMaterial() {
  return new THREE.ShaderMaterial({
    uniforms: { uCurves: shared.uCurves, uOpacity: { value: 1 } },
    vertexShader: COLOR_VERT, fragmentShader: COLOR_FRAG, transparent: true, depthWrite: true,
    side: THREE.DoubleSide,
  })
}

/** runs: [{text, x, y, size, color: THREE.Color}] laid in the xy plane, x the pen start, y the baseline. */
export function colorGeometry(runs) {
  const P = [], E = [], C = [], K = [], idx = []
  const pad = 0.06
  for (const it of runs) {
    let pen = 0
    const s = it.size, col = it.color
    for (const ch of it.text) {
      const g = glyph(ch)
      if (g.count) {
        const v = P.length / 3
        const xs = [g.x0 - pad, g.x1 + pad, g.x1 + pad, g.x0 - pad], ys = [g.y0 - pad, g.y0 - pad, g.y1 + pad, g.y1 + pad]
        for (let k = 0; k < 4; k++) {
          P.push(it.x + (pen + xs[k]) * s, it.y + ys[k] * s, it.z || 0)
          E.push(xs[k], ys[k])
          C.push(g.start, g.count)
          K.push(col.r, col.g, col.b)
        }
        idx.push(v, v + 1, v + 2, v, v + 2, v + 3)
      }
      pen += g.adv
    }
  }
  const geo = new THREE.BufferGeometry()
  geo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(P), 3))
  geo.setAttribute('aEm', new THREE.BufferAttribute(new Float32Array(E), 2))
  geo.setAttribute('aCurve', new THREE.BufferAttribute(new Float32Array(C), 2))
  geo.setAttribute('aColor', new THREE.BufferAttribute(new Float32Array(K), 3))
  geo.setIndex(idx)
  geo.computeBoundingSphere()
  return geo
}
