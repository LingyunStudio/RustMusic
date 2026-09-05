// 生成 RustMusic 应用图标（1024x1024 PNG）：渐变圆角方块 + 白色播放三角
import { deflateSync } from "node:zlib";
import { writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const S = 1024;
const R = 210; // 圆角半径

function clamp(v, lo, hi) { return v < lo ? lo : v > hi ? hi : v; }
function lerp(a, b, t) { return a + (b - a) * t; }

// 圆角矩形 SDF（负值在内部）
function sdRoundRect(px, py, cx, cy, hw, hh, r) {
  const qx = Math.abs(px - cx) - (hw - r);
  const qy = Math.abs(py - cy) - (hh - r);
  const ax = Math.max(qx, 0), ay = Math.max(qy, 0);
  return Math.min(Math.max(qx, qy), 0) + Math.hypot(ax, ay) - r;
}

// 圆角三角形 SDF（顶点 + 圆角半径），凸多边形：三半平面最大值
function sdTriangle(px, py, verts, r) {
  // 顶点按顺时针或逆时针排列；内部 sd < 0
  let d = -Infinity;
  const n = verts.length;
  for (let i = 0; i < n; i++) {
    const [x1, y1] = verts[i];
    const [x2, y2] = verts[(i + 1) % n];
    const ex = x2 - x1, ey = y2 - y1;
    const wx = px - x1, wy = py - y1;
    // 外法线（假设顶点顺时针，法线指向右侧）
    const len = Math.hypot(ex, ey);
    const nx = ey / len, ny = -ex / len;
    d = Math.max(d, wx * nx + wy * ny);
  }
  return d - r;
}

const tri = [
  [400, 348],
  [728, 512],
  [400, 676],
];

const px = Buffer.alloc(S * S * 4);
for (let y = 0; y < S; y++) {
  for (let x = 0; x < S; x++) {
    // 2x2 超采样
    let alpha = 0, r = 0, g = 0, b = 0;
    for (let sy = 0; sy < 2; sy++) {
      for (let sx = 0; sx < 2; sx++) {
        const fx = x + 0.25 + sx * 0.5, fy = y + 0.25 + sy * 0.5;
        const dRect = sdRoundRect(fx, fy, S / 2, S / 2, S / 2 - 4, S / 2 - 4, R);
        const a = clamp(0.5 - dRect, 0, 1);
        // 对角渐变：紫罗兰 -> 青蓝
        const t = clamp((fx + fy) / (2 * S), 0, 1);
        let cr = lerp(0x8b, 0x06, t) / 255;
        let cg = lerp(0x5c, 0xb6, t) / 255;
        let cb = lerp(0xf6, 0xd4, t) / 255;
        // 左上高光
        const hl = clamp(1 - Math.hypot(fx - S * 0.18, fy - S * 0.14) / (S * 0.75), 0, 1) * 0.16;
        cr = lerp(cr, 1, hl); cg = lerp(cg, 1, hl); cb = lerp(cb, 1, hl);
        // 白色圆角播放三角
        const dTri = sdTriangle(fx, fy, tri, 26);
        const ta = clamp(0.5 - dTri, 0, 1) * 0.96;
        cr = lerp(cr, 1, ta); cg = lerp(cg, 1, ta); cb = lerp(cb, 1, ta);
        alpha += a / 4;
        r += cr * a / 4; g += cg * a / 4; b += cb * a / 4;
      }
    }
    const i = (y * S + x) * 4;
    px[i] = clamp(r * 255, 0, 255) | 0;
    px[i + 1] = clamp(g * 255, 0, 255) | 0;
    px[i + 2] = clamp(b * 255, 0, 255) | 0;
    px[i + 3] = clamp(alpha * 255, 0, 255) | 0;
  }
}

// ---- PNG 编码 ----
const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();
function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
function chunk(type, data) {
  const out = Buffer.alloc(8 + data.length + 4);
  out.writeUInt32BE(data.length, 0);
  out.write(type, 4, "ascii");
  data.copy(out, 8);
  out.writeUInt32BE(crc32(Buffer.concat([Buffer.from(type, "ascii"), data])), 8 + data.length);
  return out;
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(S, 0); ihdr.writeUInt32BE(S, 4);
ihdr[8] = 8; ihdr[9] = 6; // 8bit RGBA
const raw = Buffer.alloc(S * (S * 4 + 1));
for (let y = 0; y < S; y++) {
  raw[y * (S * 4 + 1)] = 0;
  px.copy(raw, y * (S * 4 + 1) + 1, y * S * 4, (y + 1) * S * 4);
}
const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw, { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);

const here = dirname(fileURLToPath(import.meta.url));
const out = join(here, "logo.png");
writeFileSync(out, png);
console.log("written:", out, png.length, "bytes");
