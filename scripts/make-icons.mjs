// Zero-dependency icon generator. Renders the app icon designed in the
// Claude Design project: a dark gradient tile with a blurred conic glow, a
// top-left sheen, and the white ring-arcs mark. Writes design/app-icon.png
// (1024²) and src-tauri/icons/tray.png (44², black+alpha template icon).
import zlib from "node:zlib";
import fs from "node:fs";

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
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const t = Buffer.from(type, "ascii");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([t, data])));
  return Buffer.concat([len, t, data, crc]);
}

function encodePNG(width, height, rgba) {
  const stride = 1 + width * 4;
  const raw = Buffer.alloc(height * stride);
  for (let y = 0; y < height; y++) {
    rgba.copy(raw, y * stride + 1, y * width * 4, (y + 1) * width * 4);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6; // RGBA
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", zlib.deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

// Anti-aliased coverage of a rounded rectangle via signed distance.
function roundRectCov(px, py, x, y, w, h, r) {
  const qx = Math.abs(px - (x + w / 2)) - (w / 2 - r);
  const qy = Math.abs(py - (y + h / 2)) - (h / 2 - r);
  const d = Math.hypot(Math.max(qx, 0), Math.max(qy, 0)) + Math.min(Math.max(qx, qy), 0) - r;
  return Math.min(1, Math.max(0, 0.5 - d));
}

const clamp01 = (v) => Math.min(1, Math.max(0, v));
const lerp = (a, b, t) => a + (b - a) * t;

/** Angle in degrees, 0 at 12 o'clock, increasing clockwise. */
function clockAngle(dx, dy) {
  return (Math.atan2(dx, -dy) * (180 / Math.PI) + 360) % 360;
}

/**
 * Coverage of a stroked arc with round caps. Start at the top (12 o'clock),
 * sweeping clockwise — matches the SVG mark's rotate(-90) + dasharray.
 */
function arcCov(px, py, cx, cy, r, sw, sweepDeg) {
  const dx = px - cx;
  const dy = py - cy;
  const dist = Math.hypot(dx, dy);
  const half = sw / 2;
  const a = clockAngle(dx, dy);
  if (a <= sweepDeg) {
    return clamp01(0.5 + (half - Math.abs(dist - r)));
  }
  // round caps at both arc ends
  const capAt = (deg) => {
    const rad = (deg * Math.PI) / 180;
    const ex = cx + r * Math.sin(rad);
    const ey = cy - r * Math.cos(rad);
    return clamp01(0.5 + (half - Math.hypot(px - ex, py - ey)));
  };
  return Math.max(capAt(0), capAt(sweepDeg));
}

/** The ring mark, as alpha coverage per layer: [outer, inner, dot]. */
function markCov(px, py, cx, cy, box) {
  const outer = arcCov(px, py, cx, cy, 0.38 * box, 0.115 * box, 290);
  const inner = arcCov(px, py, cx, cy, 0.22 * box, 0.095 * box, 240);
  const dot = clamp01(0.5 + (0.055 * box - Math.hypot(px - cx, py - cy)));
  return [outer, inner, dot];
}

// Conic glow stops (angle°, r, g, b, a) — from the design's conic-gradient.
const GLOW = [
  [0, 255, 140, 200, 0.6],
  [90, 120, 220, 255, 0.55],
  [180, 255, 214, 130, 0.5],
  [270, 160, 140, 255, 0.6],
  [360, 255, 140, 200, 0.6],
];

function glowAt(deg) {
  for (let i = 0; i < GLOW.length - 1; i++) {
    const [a0, ...c0] = GLOW[i];
    const [a1, ...c1] = GLOW[i + 1];
    if (deg >= a0 && deg <= a1) {
      const t = (deg - a0) / (a1 - a0);
      return c0.map((v, k) => lerp(v, c1[k], t));
    }
  }
  return GLOW[0].slice(1);
}

function drawAppIcon(S = 1024) {
  const img = Buffer.alloc(S * S * 4);
  const inset = S * 0.08;
  const tile = S - 2 * inset;
  const radius = tile * 0.23;
  const cx = S / 2;
  const cy = S / 2;
  const box = tile * 0.6; // glyph box, like the 46px mark in the 76px tile

  for (let y = 0; y < S; y++) {
    for (let x = 0; x < S; x++) {
      const cov = roundRectCov(x + 0.5, y + 0.5, inset, inset, tile, tile, radius);
      if (cov <= 0) continue;

      // base: linear-gradient(155deg, #3d4877, #171b2b 52%, #090b12)
      const dir = { x: Math.sin((155 * Math.PI) / 180), y: -Math.cos((155 * Math.PI) / 180) };
      const proj = ((x - inset) * dir.x + (y - inset) * dir.y) / (tile * (Math.abs(dir.x) + Math.abs(dir.y)));
      const t = clamp01(proj);
      let r, g, b;
      if (t < 0.52) {
        const u = t / 0.52;
        r = lerp(0x3d, 0x17, u);
        g = lerp(0x48, 0x1b, u);
        b = lerp(0x77, 0x2b, u);
      } else {
        const u = (t - 0.52) / 0.48;
        r = lerp(0x17, 0x09, u);
        g = lerp(0x1b, 0x0b, u);
        b = lerp(0x2b, 0x12, u);
      }

      // blurred conic glow, from 200deg — subtle in the center, colorful at
      // the rim, so the tile stays deep navy like the design
      const deg = (clockAngle(x - cx, y - cy) - 200 + 720) % 360;
      const [gr, gg, gb, ga] = glowAt(deg);
      const norm = Math.hypot(x - cx, y - cy) / (tile * 0.5);
      const rim = clamp01(norm * norm * 0.55);
      const glowA = ga * 0.6 * rim;
      r = lerp(r, gr, glowA);
      g = lerp(g, gg, glowA);
      b = lerp(b, gb, glowA);

      // top-left sheen: linear-gradient(150deg, rgba(255,255,255,.32), transparent 44%)
      const sdir = { x: Math.sin((150 * Math.PI) / 180), y: -Math.cos((150 * Math.PI) / 180) };
      const sproj = clamp01(
        ((x - inset) * sdir.x + (y - inset) * sdir.y) / (tile * (Math.abs(sdir.x) + Math.abs(sdir.y))),
      );
      const sheenA = sproj < 0.44 ? 0.32 * (1 - sproj / 0.44) : 0;
      r = lerp(r, 255, sheenA);
      g = lerp(g, 255, sheenA);
      b = lerp(b, 255, sheenA);

      // the white ring mark
      const [outer, inner, dot] = markCov(x + 0.5, y + 0.5, cx, cy, box);
      const markA = Math.max(outer, inner * 0.72, dot);
      r = lerp(r, 255, markA);
      g = lerp(g, 255, markA);
      b = lerp(b, 255, markA);

      const idx = (y * S + x) * 4;
      img[idx] = Math.round(r);
      img[idx + 1] = Math.round(g);
      img[idx + 2] = Math.round(b);
      img[idx + 3] = Math.round(255 * cov);
    }
  }
  return encodePNG(S, S, img);
}

function drawTray(S = 44) {
  const img = Buffer.alloc(S * S * 4);
  const cx = S / 2;
  const cy = S / 2;
  const box = S * 0.74;
  for (let y = 0; y < S; y++) {
    for (let x = 0; x < S; x++) {
      const [outer, inner, dot] = markCov(x + 0.5, y + 0.5, cx, cy, box);
      const a = Math.max(outer, inner * 0.62, dot);
      const idx = (y * S + x) * 4;
      img[idx + 3] = Math.round(255 * a); // black, alpha only (template icon)
    }
  }
  return encodePNG(S, S, img);
}

fs.mkdirSync("design", { recursive: true });
fs.writeFileSync("design/app-icon.png", drawAppIcon());
fs.mkdirSync("src-tauri/icons", { recursive: true });
fs.writeFileSync("src-tauri/icons/tray.png", drawTray());
console.log("wrote design/app-icon.png and src-tauri/icons/tray.png");
