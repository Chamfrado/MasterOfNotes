#!/usr/bin/env node
// Fetches the official ONNX Runtime DLL (SPEC-012, Law 12) for release bundles:
// reads only the needed byte ranges of Microsoft's NuGet package (~6 MB of 160 MB), checks the
// pinned SHA-256 and writes src-tauri/resources/onnxruntime.dll. Idempotent; exits != 0 on any
// mismatch. Usage: node fetch-onnxruntime.mjs [outPath]
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { inflateRawSync } from "node:zlib";

const URL =
  "https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime/1.31.1/microsoft.ml.onnxruntime.1.31.1.nupkg";
const MEMBER = "runtimes/win-x64/native/onnxruntime.dll";
const SHA256 = "cb2fa518b18356617f6f9f460c012501da2819ec7e5c1d80bd8f5de20492db0c";

const out = process.argv[2] ?? join(dirname(fileURLToPath(import.meta.url)), "..", "resources", "onnxruntime.dll");
const sha = (buf) => createHash("sha256").update(buf).digest("hex");

if (existsSync(out) && sha(readFileSync(out)) === SHA256) {
  console.log(`onnxruntime.dll ok (${out})`);
  process.exit(0);
}

async function range(spec) {
  const res = await fetch(URL, { headers: { Range: `bytes=${spec}` } });
  if (res.status !== 206) throw new Error(`range ${spec}: HTTP ${res.status}`);
  return { buf: Buffer.from(await res.arrayBuffer()), total: Number(res.headers.get("content-range")?.split("/")[1]) };
}

// End of central directory: in the last 22 + 64 KiB (max comment) bytes.
const { buf: tail, total } = await range(`-${22 + 0xffff}`);
const eocd = tail.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]));
if (eocd < 0) throw new Error("zip: end of central directory not found");
const cdSize = tail.readUInt32LE(eocd + 12);
const cdOffset = tail.readUInt32LE(eocd + 16);
if (cdOffset === 0xffffffff || cdOffset + cdSize > total) throw new Error("zip: unsupported layout (zip64?)");

const { buf: cd } = await range(`${cdOffset}-${cdOffset + cdSize - 1}`);
let entry;
for (let p = 0; p + 46 <= cd.length && cd.readUInt32LE(p) === 0x02014b50; ) {
  const nameLen = cd.readUInt16LE(p + 28);
  const name = cd.toString("utf8", p + 46, p + 46 + nameLen);
  if (name === MEMBER) {
    entry = { method: cd.readUInt16LE(p + 10), size: cd.readUInt32LE(p + 20), offset: cd.readUInt32LE(p + 42) };
    break;
  }
  p += 46 + nameLen + cd.readUInt16LE(p + 30) + cd.readUInt16LE(p + 32);
}
if (!entry) throw new Error(`zip: ${MEMBER} not found`);

const { buf: local } = await range(`${entry.offset}-${entry.offset + 29}`);
if (local.readUInt32LE(0) !== 0x04034b50) throw new Error("zip: bad local header");
const start = entry.offset + 30 + local.readUInt16LE(26) + local.readUInt16LE(28);
const { buf: raw } = await range(`${start}-${start + entry.size - 1}`);
const dll = entry.method === 8 ? inflateRawSync(raw) : entry.method === 0 ? raw : null;
if (!dll) throw new Error(`zip: unsupported compression method ${entry.method}`);

const got = sha(dll);
if (got !== SHA256) {
  console.error(`onnxruntime.dll: SHA-256 mismatch (got ${got}, expected ${SHA256})`);
  process.exit(1);
}
mkdirSync(dirname(out), { recursive: true });
writeFileSync(`${out}.part`, dll);
renameSync(`${out}.part`, out);
console.log(`onnxruntime.dll fetched and verified (${dll.length} bytes) -> ${out}`);
