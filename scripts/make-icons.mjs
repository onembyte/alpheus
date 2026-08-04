// Zero-dependency icon generator: writes design/app-icon.png (1024²) and
// src-tauri/icons/tray.png (44², black+alpha macOS template icon).
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
  ihdr[8] = 8; // bit depth
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

function drawAppIcon(S = 1024) {
  const img = Buffer.alloc(S * S * 4);
  const inset = S * 0.08;
  const bgR = S * 0.22;
  const barW = S * 0.52;
  const barH = S * 0.115;
  const gap = S * 0.055;
  const barX = (S - barW) / 2;
  const startY = S / 2 - (3 * barH + 2 * gap) / 2;
  for (let y = 0; y < S; y++) {
    for (let x = 0; x < S; x++) {
      const bg = roundRectCov(x + 0.5, y + 0.5, inset, inset, S - 2 * inset, S - 2 * inset, bgR);
      if (bg <= 0) continue;
      const t = y / S; // vertical gradient #38bdf8 → #1e40af
      let r = 56 + (30 - 56) * t;
      let g = 189 + (64 - 189) * t;
      let b = 248 + (175 - 248) * t;
      let bar = 0;
      for (let i = 0; i < 3; i++) {
        bar = Math.max(
          bar,
          roundRectCov(x + 0.5, y + 0.5, barX, startY + i * (barH + gap), barW, barH, barH / 2),
        );
      }
      r += (255 - r) * bar;
      g += (255 - g) * bar;
      b += (255 - b) * bar;
      const idx = (y * S + x) * 4;
      img[idx] = Math.round(r);
      img[idx + 1] = Math.round(g);
      img[idx + 2] = Math.round(b);
      img[idx + 3] = Math.round(255 * bg);
    }
  }
  return encodePNG(S, S, img);
}

function drawTray(S = 44) {
  const img = Buffer.alloc(S * S * 4);
  const barW = 30;
  const barH = 7;
  const gap = 4;
  const barX = (S - barW) / 2;
  const startY = (S - (3 * barH + 2 * gap)) / 2;
  for (let y = 0; y < S; y++) {
    for (let x = 0; x < S; x++) {
      let cov = 0;
      for (let i = 0; i < 3; i++) {
        cov = Math.max(
          cov,
          roundRectCov(x + 0.5, y + 0.5, barX, startY + i * (barH + gap), barW, barH, barH / 2),
        );
      }
      const idx = (y * S + x) * 4;
      img[idx + 3] = Math.round(255 * cov); // black, alpha only (template icon)
    }
  }
  return encodePNG(S, S, img);
}

fs.mkdirSync("design", { recursive: true });
fs.writeFileSync("design/app-icon.png", drawAppIcon());
fs.mkdirSync("src-tauri/icons", { recursive: true });
fs.writeFileSync("src-tauri/icons/tray.png", drawTray());
console.log("wrote design/app-icon.png and src-tauri/icons/tray.png");
