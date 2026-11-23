// Builds resources/THIRD-PARTY-NOTICES.txt for release builds (SPEC-014 R2, dependency-audit.md §4.1):
// the hand-written header (THIRD-PARTY-NOTICES.txt: ONNX Runtime) + the license files of every crate
// compiled into the Windows executable + every production JS package in the WebView bundle (incl. fonts).
// Identical license texts are printed once with the list of packages they cover.
// Usage (repo root or anywhere): node apps/desktop/src-tauri/scripts/gen-notices.mjs
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const tauriDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const desktopDir = join(tauriDir, "..");
const LICENSE_FILE = /^(licen[cs]e|copying|notice|unlicense|ofl)([-._].*)?(\.(txt|md))?$/i;

// pnpm is a .cmd shim on Windows (needs a shell); args here are constants, never user input.
const run = (cmd, args, cwd, shell = false) =>
  execFileSync(cmd, args, { cwd, encoding: "utf8", maxBuffer: 256 * 1024 * 1024, shell });

function licenseTexts(dir) {
  if (!existsSync(dir)) return [];
  return readdirSync(dir)
    .filter((f) => LICENSE_FILE.test(f))
    .sort()
    .map((f) => readFileSync(join(dir, f), "utf8").replace(/\r\n/g, "\n").trim());
}

// Rust: normal (non-dev, non-build) closure from the app crate, Windows target only.
const meta = JSON.parse(
  run("cargo", ["metadata", "--format-version", "1", "--locked", "--filter-platform", "x86_64-pc-windows-msvc"], tauriDir),
);
const pkgs = new Map(meta.packages.map((p) => [p.id, p]));
const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
const root = meta.packages.find((p) => p.name === "master-of-notes");
if (!root) throw new Error("master-of-notes not found in cargo metadata");
const seen = new Set();
const stack = [root.id];
while (stack.length) {
  const id = stack.pop();
  if (seen.has(id)) continue;
  seen.add(id);
  for (const d of nodes.get(id)?.deps ?? []) {
    if (d.dep_kinds.some((k) => k.kind === null)) stack.push(d.pkg);
  }
}
const entries = [];
for (const id of seen) {
  const p = pkgs.get(id);
  if (p.source === null) continue; // our own workspace crates
  entries.push({ name: `${p.name} ${p.version}`, spdx: p.license ?? "(see files)", url: p.repository ?? "", authors: p.authors?.join(", ") ?? "", texts: licenseTexts(dirname(p.manifest_path)) });
}

// JS: production packages bundled into the WebView (React, Tauri APIs, fonts).
const js = JSON.parse(run("pnpm licenses list --prod --json", [], desktopDir, true));
for (const list of Object.values(js)) {
  for (const p of list) {
    entries.push({ name: `${p.name} ${p.versions.join(", ")}`, spdx: p.license, url: p.homepage ?? "", authors: p.author ?? "", texts: p.paths.flatMap(licenseTexts) });
  }
}

entries.sort((a, b) => a.name.localeCompare(b.name));
const byText = new Map();
const noFile = [];
for (const e of entries) {
  if (e.texts.length === 0) noFile.push(e);
  for (const t of e.texts) byText.set(t, [...(byText.get(t) ?? []), e.name]);
}
if (noFile.some((e) => !e.spdx || e.spdx === "(see files)")) {
  throw new Error(`packages without license file or SPDX: ${noFile.map((e) => e.name).join(", ")}`);
}

const out = [readFileSync(join(tauriDir, "THIRD-PARTY-NOTICES.txt"), "utf8").replace(/\r\n/g, "\n").trim()];
out.push(
  "",
  "=".repeat(78),
  `Open-source components compiled into this application (${entries.length} packages).`,
  "MPL-2.0 components are used unmodified; their source code is available at the URL listed.",
  "=".repeat(78),
  "",
  ...entries.map((e) => `- ${e.name} (${e.spdx})${e.url ? ` ${e.url}` : ""}`),
);
for (const [text, names] of byText) {
  out.push("", "-".repeat(78), `Applies to: ${names.join("; ")}`, "-".repeat(78), "", text);
}
if (noFile.length) {
  // MIT/BSD require the copyright notice + license text: authors from the manifest, text = a package above with the same license.
  const sameLicense = (spdx) => entries.find((o) => o.texts.length && o.spdx.split(/ OR |\//)[0] === spdx.split(/ OR |\//)[0])?.name;
  out.push("", "-".repeat(78), "Packages that ship no license file (copyright from their manifest; license text as reproduced above):");
  out.push(...noFile.map((e) => `- ${e.name} (${e.spdx}). Copyright: ${e.authors || "the authors"}. License text: same as ${sameLicense(e.spdx) ?? e.spdx}.`));
}
mkdirSync(join(tauriDir, "resources"), { recursive: true });
const target = join(tauriDir, "resources", "THIRD-PARTY-NOTICES.txt");
writeFileSync(target, out.join("\n") + "\n");
console.log(`notices: ${entries.length} packages, ${byText.size} distinct license texts, ${noFile.length} without file -> ${target}`);
