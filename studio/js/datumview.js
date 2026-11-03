// Datum cubes in the scene: tiles near, one color far, and the gimbal that scales and turns them.
import * as D from './datum.js'

const AXIS_COL = ['#d9534f', '#4caf50', '#4a7bd0']
const ON = '#f4f4f4', OFF = '#161616'

export function datumLayer(THREE) {
  const group = new THREE.Group()
  const cube = new THREE.BoxGeometry(1, 1, 1).translate(0, 0.5, 0)
  const body = new THREE.BoxGeometry(0.97, 0.97, 0.97).translate(0, 0.5, 0)
  const tile = new THREE.PlaneGeometry(0.43, 0.43)
  const edges = new THREE.EdgesGeometry(cube)
  const bodyMat = new THREE.MeshStandardMaterial({ roughness: 0.6 })
  const tileMat = new THREE.MeshStandardMaterial({ roughness: 0.35, polygonOffset: true, polygonOffsetFactor: -1, polygonOffsetUnits: -1 })
  const farMat = new THREE.MeshBasicMaterial()
  const selMat = new THREE.LineBasicMaterial({ color: 0xff5fa2, depthTest: false, transparent: true })
  selMat.userData.shared = true
  // the 24 tiles in the cube's frame: plane x along u, plane y against v, plane z out of the face
  const tiles = []
  for (let f = 0; f < 6; f++) for (let v = 0; v < 2; v++) for (let u = 0; u < 2; u++) {
    const t = D.tileCenter(f, u, v)
    const m = new THREE.Matrix4().makeBasis(new THREE.Vector3(...t.u), new THREE.Vector3(...t.v).negate(), new THREE.Vector3(...t.n))
    m.setPosition(t.c[0] + t.n[0] * 0.003, t.c[1] + t.n[1] * 0.003, t.c[2] + t.n[2] * 0.003)
    tiles.push({ m, bit: D.tileBit(f, u, v) })
  }
  const shared = [cube, body, tile, edges]
  let eyeAt = null
  const clear = () => {
    for (const o of [...group.children]) {
      group.remove(o)
      if (o.isInstancedMesh) o.dispose()
    }
  }
  // items: { id, cell, parts, value }
  function build(items, eye, sel) {
    clear()
    eyeAt = eye.slice()
    const near = [], far = []
    for (const d of items) (D.lod(d.cell, d.parts, eye) === 'near' ? near : far).push(d)
    const M = new THREE.Matrix4(), T = new THREE.Matrix4(), col = new THREE.Color()
    if (near.length) {
      const bodies = new THREE.InstancedMesh(body, bodyMat, near.length)
      const faces = new THREE.InstancedMesh(tile, tileMat, near.length * 24)
      near.forEach((d, i) => {
        M.fromArray(D.matrixOf(d.cell, d.parts))
        bodies.setMatrixAt(i, M)
        bodies.setColorAt(i, col.set(D.colorOf(d.value)).multiplyScalar(0.55))
        tiles.forEach((t, j) => {
          faces.setMatrixAt(i * 24 + j, T.multiplyMatrices(M, t.m))
          faces.setColorAt(i * 24 + j, col.set((d.value >> t.bit) & 1 ? ON : OFF))
        })
      })
      group.add(bodies, faces)
    }
    if (far.length) {
      const flat = new THREE.InstancedMesh(cube, farMat, far.length)
      far.forEach((d, i) => {
        flat.setMatrixAt(i, M.fromArray(D.matrixOf(d.cell, d.parts)))
        flat.setColorAt(i, col.set(D.colorOf(d.value)))
      })
      group.add(flat)
    }
    const s = items.find((d) => d.id === sel)
    if (s) {
      const l = new THREE.LineSegments(edges, selMat)
      l.matrixAutoUpdate = false
      l.matrix.fromArray(D.matrixOf(s.cell, s.parts)).multiply(new THREE.Matrix4().makeScale(1.02, 1.02, 1.02))
      l.renderOrder = 10
      group.add(l)
    }
    return { near: near.map((d) => d.id), far: far.map((d) => d.id) }
  }
  // whether the eye has moved enough to change what is near
  function stale(eye) {
    return !eyeAt || Math.hypot(eye[0] - eyeAt[0], eye[1] - eyeAt[1], eye[2] - eyeAt[2]) > 0.5
  }
  return { group, build, stale, shared, disposeGeo: (g) => shared.includes(g) }
}

// the gimbal: axis and plane handles scale along the cube's own axes, rings turn it about x y z
export function gimbal(THREE) {
  const group = new THREE.Group()
  group.visible = false
  const handles = []
  const mat = (c, o = 1) => new THREE.MeshBasicMaterial({ color: c, depthTest: false, transparent: true, opacity: o })
  const stem = new THREE.CylinderGeometry(0.025, 0.025, 1, 6).translate(0, 0.5, 0)
  const knob = new THREE.BoxGeometry(0.16, 0.16, 0.16)
  const square = new THREE.PlaneGeometry(0.28, 0.28)
  const ring = new THREE.TorusGeometry(1, 0.022, 6, 64)
  const ringHit = new THREE.TorusGeometry(1, 0.11, 4, 48)
  const parts = {}
  const add = (o, name, order = 20) => { o.renderOrder = order; if (name) { o.userData.gimbal = name; handles.push(o) } group.add(o); return o }
  for (let i = 0; i < 3; i++) {
    const a = 'xyz'[i]
    parts['stem' + a] = add(new THREE.Mesh(stem, mat(AXIS_COL[i])), 's' + a)
    parts['knob' + a] = add(new THREE.Mesh(knob, mat(AXIS_COL[i])), 's' + a)
    parts['ring' + a] = add(new THREE.Mesh(ring, mat(AXIS_COL[i], 0.85)), null)
    parts['ringHit' + a] = add(new THREE.Mesh(ringHit, mat(AXIS_COL[i], 0)), 'r' + a)
  }
  for (const [pl, i, j, c] of [['sxy', 0, 1, AXIS_COL[2]], ['syz', 1, 2, AXIS_COL[0]], ['sxz', 0, 2, AXIS_COL[1]]]) {
    parts[pl] = add(new THREE.Mesh(square, mat(c, 0.55)), pl)
    parts[pl].userData.ij = [i, j]
    parts[pl].material.side = THREE.DoubleSide
  }
  parts.center = add(new THREE.Mesh(new THREE.BoxGeometry(0.22, 0.22, 0.22), mat('#ffffff', 0.9)), 's')
  const up = new THREE.Vector3(0, 1, 0)
  let at = null
  // place the gimbal on a cube: its center, its axes, its size
  function show(cell, p) {
    group.visible = true
    const c = new THREE.Vector3(...D.toWorld(cell, p, D.CENTER))
    const ax = [0, 1, 2].map((i) => new THREE.Vector3(...D.dirToWorld(p, [i === 0 ? 1 : 0, i === 1 ? 1 : 0, i === 2 ? 1 : 0])))
    const ext = [p.sx / 2, p.sy / 2, p.sz / 2]
    at = { c, ax, ext }
    for (let i = 0; i < 3; i++) {
      const a = 'xyz'[i]
      const len = ext[i] + 0.7
      const s = parts['stem' + a]
      s.position.copy(c)
      s.quaternion.setFromUnitVectors(up, ax[i])
      s.scale.set(1, len, 1)
      parts['knob' + a].position.copy(c).addScaledVector(ax[i], len)
      parts['knob' + a].quaternion.copy(s.quaternion)
    }
    const R = Math.max(...ext) * 1.25 + 0.9
    const rq = { x: new THREE.Quaternion().setFromAxisAngle(up, Math.PI / 2), y: new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(1, 0, 0), Math.PI / 2), z: new THREE.Quaternion() }
    for (const a of 'xyz') for (const k of ['ring', 'ringHit']) {
      const r = parts[k + a]
      r.position.copy(c)
      r.quaternion.copy(rq[a])
      r.scale.setScalar(R)
    }
    for (const pl of ['sxy', 'syz', 'sxz']) {
      const [i, j] = parts[pl].userData.ij
      const m = parts[pl]
      m.position.copy(c).addScaledVector(ax[i], ext[i] * 0.5 + 0.35).addScaledVector(ax[j], ext[j] * 0.5 + 0.35)
      const n = ax[i].clone().cross(ax[j]).normalize()
      m.quaternion.setFromRotationMatrix(new THREE.Matrix4().makeBasis(ax[i], ax[j], n))
    }
    parts.center.position.copy(c)
    parts.center.quaternion.setFromUnitVectors(up, ax[1])
  }
  function hide() { group.visible = false; at = null }
  // the handle under a ray, if any
  function hit(raycaster) {
    if (!group.visible) return null
    const h = raycaster.intersectObjects(handles, false)
    if (!h.length) return null
    // a knob or square wins over a ring it overlaps
    const best = h.find((x) => !x.object.userData.gimbal.startsWith('r')) || h[0]
    return best.object.userData.gimbal
  }
  // highlight the active handle
  function active(name) {
    for (const o of handles) {
      const on = o.userData.gimbal === name
      const m = o.material
      if (o.userData.gimbal.startsWith('r')) {
        const ring = parts['ring' + o.userData.gimbal[1]]
        ring.material.opacity = on ? 1 : 0.85
        ring.scale.setScalar(o.scale.x * (on ? 1.0 : 1))
        ring.material.color.set(on ? '#ffd75e' : AXIS_COL['xyz'.indexOf(o.userData.gimbal[1])])
      } else if (m.color) m.color.set(on ? '#ffd75e' : o.userData.gimbal === 's' ? '#ffffff' : o.userData.gimbal.length === 3 ? AXIS_COL[3 - o.userData.ij.reduce((a, b) => a + b, 0)] : AXIS_COL['xyz'.indexOf(o.userData.gimbal[1])])
    }
  }
  return { group, show, hide, hit, active, where: () => at }
}
