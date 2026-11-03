const s_rng = { s: null };
const S_M = (1n << 64n) - 1n;
function s_mix(z) {
  z.v = (z.v + 0x9E3779B97F4A7C15n) & S_M;
  let x = z.v;
  x = ((x ^ (x >> 30n)) * 0xBF58476D1CE4E5B9n) & S_M;
  x = ((x ^ (x >> 27n)) * 0x94D049BB133111EBn) & S_M;
  return x ^ (x >> 31n);
}
function s_rotl(x, k) { return ((x << BigInt(k)) | (x >> BigInt(64 - k))) & S_M; }
function s_seed(a, b) {
  const z = { v: a ^ s_rotl(b, 32) };
  s_rng.s = [s_mix(z), s_mix(z), s_mix(z), s_mix(z)];
}
function s_next() {
  if (!s_rng.s) s_seed(BigInt(Math.floor(Math.random() * 2 ** 53)), BigInt(Date.now()));
  const s = s_rng.s;
  const r = (s_rotl((s[1] * 5n) & S_M, 7) * 9n) & S_M;
  const t = (s[1] << 17n) & S_M;
  s[2] ^= s[0]; s[3] ^= s[1]; s[1] ^= s[2]; s[0] ^= s[3];
  s[2] ^= t; s[3] = s_rotl(s[3], 45);
  return r;
}
function s_seed_from(b) {
  if (b === "") return;
  const p = b.trim().split(/\s+/).map(Number);
  const u = x => BigInt.asUintN(64, BigInt(Math.trunc(x)));
  s_seed(u(p[0]), p.length > 1 ? u(p[1]) : 0n);
}
const s_files = new Map();
function s_fx(op, a, b) {
  const fs = require("fs");
  try {
    if (op === "fo") {
      const [name, mode0] = b.split("\u0001");
      const mode = (mode0 || "r").replace(/b/g, "");
      if (!["r", "w", "a", "r+", "w+", "a+"].includes(mode)) return "0bad mode '" + mode0 + "'";
      const fd = fs.openSync(a, mode);
      if (s_files.has(name)) fs.closeSync(s_files.get(name).fd);
      s_files.set(name, { fd, pos: 0, eof: false });
      return "1";
    }
    if (op === "mv") { fs.renameSync(a, b); return "1"; }
    if (op === "fc") { if (s_files.has(a)) { fs.closeSync(s_files.get(a).fd); s_files.delete(a); } return "1"; }
    if (op === "feof") { const h = s_files.get(a); return "1" + (!h || h.eof ? "1" : "0"); }
    if (op.length > 1 && op[0] === "f") {
      const h = s_files.get(a);
      if (!h) return "0no open file";
      if (op === "fw") { fs.writeSync(h.fd, b); return "1"; }
      if (op === "fat") { const i = b.indexOf("\u0001"); fs.writeSync(h.fd, b.slice(i + 1), Number(b.slice(0, i))); return "1"; }
      if (op === "fra") {
        const [off, len] = b.trim().split(/\s+/).map(Number);
        const buf = Buffer.alloc(Math.max(0, len));
        const got = fs.readSync(h.fd, buf, 0, buf.length, off);
        h.eof = got < len; h.pos = off + got;
        return "1" + buf.toString("utf8", 0, got);
      }
      if (op === "frl") {
        const bytes = []; const one = Buffer.alloc(1); let got = false;
        while (fs.readSync(h.fd, one, 0, 1, h.pos) === 1) { h.pos++; got = true; if (one[0] === 10) break; bytes.push(one[0]); }
        h.eof = !got;
        let t = Buffer.from(bytes).toString("utf8");
        if (t.endsWith("\r")) t = t.slice(0, -1);
        return "1" + t;
      }
    }
    if (op === "chk") {
      const buf = fs.readFileSync(a);
      let h = 0xcbf29ce484222325n;
      for (const x of buf) { h ^= BigInt(x); h = (h * 0x100000001b3n) & 0xffffffffffffffffn; }
      return h.toString(16).padStart(16, "0") === b ? "1" : "0changed since this program was built; build it again";
    }
    if (op === "r") return "1" + fs.readFileSync(a, "utf8");
    if (op === "ra") {
      const [off, len] = b.trim().split(/\s+/).map(Number);
      const fd = fs.openSync(a, "r");
      const buf = Buffer.alloc(Math.max(0, len));
      const got = fs.readSync(fd, buf, 0, buf.length, off);
      fs.closeSync(fd);
      return "1" + buf.toString("utf8", 0, got);
    }
    if (op === "w") { fs.writeFileSync(a, b); return "1"; }
    if (op === "a") { fs.appendFileSync(a, b); return "1"; }
    if (op === "sh") {
      const cp = require("child_process");
      const r = cp.spawnSync("/bin/sh", ["-c", a], { stdio: ["ignore", "pipe", "inherit"] });
      const rc = r.status === null ? -1 : r.status;
      return "1" + rc + "\u0001" + (r.stdout ? r.stdout.toString("utf8") : "");
    }
    if (op === "pn") { s_seed_from(b); return "1" + String(Number(s_next() >> 11n) / 9007199254740992); }
    if (op === "ps") {
      s_seed_from(b);
      const az = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
      let t = "";
      for (let i = 0; i < 32; i++) t += az[Number(s_next() % 52n)];
      return "1" + t;
    }
    return "0unknown effect";
  } catch (err) {
    return "0" + (err && err.message ? err.message : String(err));
  }
}
io_eff(CID(S.fx), s_fx);
