// Capture helpers for the user guide (SPEC-011 R3–R5): PNG, GIF (gifenc) and WebM, each checked against the R4 budget.
import { expect, type Locator, type Page } from "@playwright/test";
import { mkdirSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import gifenc from "gifenc"; // CommonJS: Node sees only a default export

const { GIFEncoder, applyPalette, quantize } = gifenc;

export const MEDIA_DIR = fileURLToPath(new URL("../../../../docs/user/media/", import.meta.url));
mkdirSync(MEDIA_DIR, { recursive: true });

const KB = 1024;
export const BUDGET = { png: 400 * KB, gif: 2048 * KB, webm: 5120 * KB, total: 40 * 1024 * KB }; // R4
const GIF_SCALE = 0.75; // 1280×800 → 960×600
const GIF_COLORS = 128;
const GIF_FPS = 8;

const fileSize = (path: string) => statSync(path).size;
export function checkBudget(name: string) {
  const ext = name.split(".").pop() as "png" | "gif" | "webm";
  const size = fileSize(MEDIA_DIR + name);
  expect(size, `${name}: ${Math.round(size / KB)} KB passa do orçamento`).toBeLessThanOrEqual(BUDGET[ext]);
  return size;
}
export function totalMediaBytes() {
  return readdirSync(MEDIA_DIR).reduce((t, f) => t + fileSize(MEDIA_DIR + f), 0);
}

const PAD = 16;
/** Viewport (Page) or element (Locator, with a 16 px margin) → docs/user/media/<id>.png. Animations off for stable images. */
export async function shot(target: Page | Locator, id: string, opts: { fullPage?: boolean } = {}) {
  const path = `${MEDIA_DIR}${id}.png`;
  const options = { path, animations: "disabled", caret: "hide" } as const;
  if ("goto" in target) await target.screenshot({ ...options, fullPage: opts.fullPage });
  else {
    const box = await target.boundingBox();
    if (!box) throw new Error(`${id}: elemento não visível`);
    const clip = { x: Math.max(0, box.x - PAD), y: Math.max(0, box.y - PAD), width: box.width + 2 * PAD, height: box.height + 2 * PAD };
    await target.page().screenshot({ ...options, clip, fullPage: true });
  }
  return checkBudget(`${id}.png`);
}

// ---------- GIF ----------

/** Minimal PNG decoder for Chromium screenshots (8-bit RGB/RGBA, not interlaced). Inflate is the web DecompressionStream. */
export async function decodePng(png: Uint8Array): Promise<{ width: number; height: number; rgba: Uint8Array }> {
  const dv = new DataView(png.buffer, png.byteOffset, png.byteLength);
  let width = 0;
  let height = 0;
  let bpp = 0;
  const idat: Uint8Array<ArrayBuffer>[] = [];
  for (let p = 8; p < png.length; ) {
    const len = dv.getUint32(p);
    const type = String.fromCharCode(...png.subarray(p + 4, p + 8));
    if (type === "IHDR") {
      width = dv.getUint32(p + 8);
      height = dv.getUint32(p + 12);
      const [depth, color, , , interlace] = png.subarray(p + 16, p + 21);
      bpp = color === 6 ? 4 : color === 2 ? 3 : 0;
      if (depth !== 8 || !bpp || interlace) throw new Error(`PNG não suportado (bits ${depth}, cor ${color}, entrelaçado ${interlace})`);
    } else if (type === "IDAT") idat.push(png.slice(p + 8, p + 8 + len));
    p += 12 + len;
  }
  const stream = new Blob(idat).stream().pipeThrough(new DecompressionStream("deflate"));
  const raw = new Uint8Array(await new Response(stream).arrayBuffer());
  const stride = width * bpp;
  const rgba = new Uint8Array(width * height * 4);
  let prev = new Uint8Array(stride);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    const cur = new Uint8Array(stride);
    for (let x = 0; x < stride; x++) {
      const a = x >= bpp ? cur[x - bpp] : 0;
      const b = prev[x];
      const c = x >= bpp ? prev[x - bpp] : 0;
      let pred = 0;
      if (filter === 1) pred = a;
      else if (filter === 2) pred = b;
      else if (filter === 3) pred = (a + b) >> 1;
      else if (filter === 4) {
        const pa = Math.abs(b - c);
        const pb = Math.abs(a - c);
        const pc = Math.abs(a + b - 2 * c);
        pred = pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      cur[x] = (line[x] + pred) & 0xff;
    }
    for (let x = 0; x < width; x++) {
      const o = (y * width + x) * 4;
      rgba[o] = cur[x * bpp];
      rgba[o + 1] = cur[x * bpp + 1];
      rgba[o + 2] = cur[x * bpp + 2];
      rgba[o + 3] = bpp === 4 ? cur[x * bpp + 3] : 255;
    }
    prev = cur;
  }
  return { width, height, rgba };
}

/** Box-filter downscale by an exact ratio (0.75 → each 4×4 block becomes 3×3 via area averaging). */
export function downscale(src: { width: number; height: number; rgba: Uint8Array }, scale: number) {
  const width = Math.round(src.width * scale);
  const height = Math.round(src.height * scale);
  const out = new Uint8Array(width * height * 4);
  const inv = 1 / scale;
  for (let y = 0; y < height; y++) {
    const y0 = y * inv;
    const y1 = y0 + inv;
    for (let x = 0; x < width; x++) {
      const x0 = x * inv;
      const x1 = x0 + inv;
      const acc = [0, 0, 0];
      let wsum = 0;
      for (let sy = Math.floor(y0); sy < Math.ceil(y1); sy++) {
        const wy = Math.min(y1, sy + 1) - Math.max(y0, sy);
        for (let sx = Math.floor(x0); sx < Math.ceil(x1); sx++) {
          const w = wy * (Math.min(x1, sx + 1) - Math.max(x0, sx));
          const o = (sy * src.width + sx) * 4;
          acc[0] += src.rgba[o] * w;
          acc[1] += src.rgba[o + 1] * w;
          acc[2] += src.rgba[o + 2] * w;
          wsum += w;
        }
      }
      const o = (y * width + x) * 4;
      out[o] = acc[0] / wsum;
      out[o + 1] = acc[1] / wsum;
      out[o + 2] = acc[2] / wsum;
      out[o + 3] = 255;
    }
  }
  return { width, height, rgba: out };
}

const sameBytes = (a: Uint8Array, b: Uint8Array) => a.length === b.length && a.every((v, i) => v === b[i]);

/**
 * Records the page while `action` runs (≈ GIF_FPS screenshots/s, real timing kept as frame delays), then writes
 * docs/user/media/<id>.gif at 960×600. Identical consecutive frames become one longer frame. `holdMs` keeps the
 * final state on screen before the loop restarts.
 */
export async function gif(page: Page, id: string, action: () => Promise<void>, holdMs = 1500) {
  const frames: { png: Uint8Array; at: number }[] = [];
  let recording = true;
  const t0 = Date.now();
  const loop = (async () => {
    while (recording) {
      const started = Date.now();
      frames.push({ png: await page.screenshot({ caret: "hide" }), at: started - t0 });
      await page.waitForTimeout(Math.max(0, 1000 / GIF_FPS - (Date.now() - started)));
    }
  })();
  try {
    await action();
  } finally {
    recording = false;
    await loop;
  }
  frames.push({ png: await page.screenshot({ caret: "hide" }), at: Date.now() - t0 });

  const enc = GIFEncoder();
  let pending: { rgba: Uint8Array; width: number; height: number; delay: number } | null = null;
  let shown: Uint8Array | null = null; // source pixels of the last written frame
  /** After the first frame, pixels equal to the previous frame are transparent (kept on screen with dispose = 1),
   * so only the changed area costs bytes, and the palette is fitted to that area. */
  const flush = (p: NonNullable<typeof pending>) => {
    const prev = shown;
    const changed = (i: number) => !prev || prev[i] !== p.rgba[i] || prev[i + 1] !== p.rgba[i + 1] || prev[i + 2] !== p.rgba[i + 2];
    const sample: number[] = [];
    for (let i = 0; i < p.rgba.length; i += 4) if (changed(i)) sample.push(p.rgba[i], p.rgba[i + 1], p.rgba[i + 2], 255);
    const palette = quantize(new Uint8Array(sample), GIF_COLORS - 1);
    const index = applyPalette(p.rgba, palette);
    if (!prev) enc.writeFrame(index, p.width, p.height, { palette, delay: p.delay });
    else {
      const transparentIndex = palette.length;
      for (let i = 0; i < index.length; i++) if (!changed(i * 4)) index[i] = transparentIndex;
      enc.writeFrame(index, p.width, p.height, { palette: [...palette, [0, 0, 0]], delay: p.delay, transparent: true, transparentIndex, dispose: 1 });
    }
    shown = p.rgba;
  };
  for (let i = 0; i < frames.length; i++) {
    const img = downscale(await decodePng(frames[i].png), GIF_SCALE);
    const delay = i + 1 < frames.length ? frames[i + 1].at - frames[i].at : holdMs;
    if (pending && sameBytes(pending.rgba, img.rgba)) pending.delay += delay;
    else {
      if (pending) flush(pending);
      pending = { ...img, delay };
    }
  }
  if (pending) flush(pending);
  enc.finish();
  writeFileSync(`${MEDIA_DIR}${id}.gif`, enc.bytes());
  return checkBudget(`${id}.gif`);
}
