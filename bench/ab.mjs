#!/usr/bin/env node
// A/B comparison of two versions of Log Lens in the same browser session.
//
// Usage (from the repository root):
//   npm run bench:ab -- <refA> <refB>                 # e.g. 34e3d75 HEAD
//   npm run bench:ab -- HEAD .                        # "." = working tree, uncommitted changes included
//   npm run bench:ab -- 34e3d75 HEAD --runs 15 --size 100000 --headed
//
// Why: separate benchmark runs happen at different moments (power mode, heat,
// background work), so a 10-20 % difference between two runs can be noise.
// Here both versions are measured interleaved (A B, B A, A B ...) in one browser
// session; noise hits both sides the same way.
//
// Steps:
//   1. Each git ref is checked out into .bench-ab/<sha> (git worktree) and built
//      with `trunk build --release`. "." builds the current working tree.
//   2. Both dist/ folders are served; the same samples/bench_<size>.log is loaded
//      alternately in both versions (1 warm-up round + N measured rounds).
//   3. Prints medians, the difference and in how many rounds B was faster.
//      Nothing is written to README.md.

import { spawnSync } from "node:child_process";
import { createServer } from "node:http";
import { readFileSync, existsSync, mkdirSync, openSync, closeSync, statSync } from "node:fs";
import { join, extname, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const IS_WIN = process.platform === "win32";

// ---------- arguments ----------
const args = process.argv.slice(2);
const VALUE_OPTIONS = ["--runs", "--size", "--browser"];
const positional = args.filter((a, i) => !a.startsWith("--") && !VALUE_OPTIONS.includes(args[i - 1]));
const flag = (name) => args.includes(name);
const option = (name, fallback) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};
if (positional.length !== 2) {
  console.error("Kullanım: npm run bench:ab -- <refA> <refB> [--runs 10] [--size 500000] [--browser firefox] [--headed] [--keep]");
  console.error('Örnek:    npm run bench:ab -- 34e3d75 HEAD     ("." = commit edilmemiş çalışma kopyası)');
  process.exit(1);
}
const RUNS = Number(option("--runs", "10"));
const SIZE = Number(option("--size", "500000"));
const BROWSER = option("--browser", "firefox");
const HEADED = flag("--headed");
const KEEP = flag("--keep");

// ---------- helpers ----------
function run(cmd, cmdArgs, opts = {}) {
  console.log(`$ ${cmd} ${cmdArgs.join(" ")}`);
  const r = spawnSync(cmd, cmdArgs, { cwd: ROOT, stdio: "inherit", shell: IS_WIN, ...opts });
  if (r.status !== 0) {
    console.error(`\n"${cmd}" başarısız oldu (çıkış kodu ${r.status}).`);
    process.exit(1);
  }
}

function git(gitArgs) {
  const r = spawnSync("git", gitArgs, { cwd: ROOT, encoding: "utf8" });
  if (r.status !== 0) throw new Error(r.stderr.trim());
  return r.stdout.trim();
}

const median = (xs) => {
  const s = [...xs].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};

function fmtMs(ms) {
  if (ms == null || Number.isNaN(ms)) return "—";
  if (ms >= 1000) return `${(ms / 1000).toFixed(2)} s`;
  if (ms >= 100) return `${ms.toFixed(0)} ms`;
  if (ms >= 10) return `${ms.toFixed(1)} ms`;
  return `${ms.toFixed(2)} ms`;
}

function ensureInput() {
  const path = join(ROOT, "samples", `bench_${SIZE}.log`);
  if (existsSync(path)) return path;
  const python = process.env.PYTHON || (IS_WIN ? "python" : "python3");
  mkdirSync(join(ROOT, "samples"), { recursive: true });
  const code =
    "import sys, runpy; sys.stdout.reconfigure(newline='\\n'); " +
    `sys.argv = ['logs_gen.py', '${SIZE}']; runpy.run_path('tools/logs_gen.py', run_name='__main__')`;
  const fd = openSync(path, "w");
  console.log(`$ ${python} tools/logs_gen.py ${SIZE} > samples/bench_${SIZE}.log`);
  const r = spawnSync(python, ["-c", code], { cwd: ROOT, stdio: ["ignore", fd, "inherit"] });
  closeSync(fd);
  if (r.status !== 0) {
    console.error("Girdi üretilemedi. Python PATH'te değilse PYTHON ortam değişkenini ayarlayın.");
    process.exit(1);
  }
  return path;
}

/** Builds a git ref (or "." = working tree) and returns { label, dist, cleanup }. */
function prepare(ref) {
  // Shared target dir: dependencies are compiled once for both versions.
  const env = { ...process.env, CARGO_TARGET_DIR: join(ROOT, "target") };
  if (ref === ".") {
    run("trunk", ["build", "--release"], { env });
    return { label: "çalışma kopyası", dist: join(ROOT, "dist"), cleanup: () => {} };
  }
  let sha;
  try {
    sha = git(["rev-parse", "--short", `${ref}^{commit}`]);
  } catch (e) {
    console.error(`Geçersiz git ref: ${ref}\n${e.message}`);
    process.exit(1);
  }
  const dir = join(ROOT, ".bench-ab", sha);
  if (existsSync(dir)) git(["worktree", "remove", "--force", dir]);
  run("git", ["worktree", "add", "--detach", dir, sha]);
  run("trunk", ["build", "--release"], { cwd: dir, env });
  const label = ref === sha ? sha : `${ref} (${sha})`;
  return {
    label,
    dist: join(dir, "dist"),
    cleanup: () => (KEEP ? null : git(["worktree", "remove", "--force", dir])),
  };
}

const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".wasm": "application/wasm",
};

function serve(dir) {
  const server = createServer((req, res) => {
    const url = decodeURIComponent(new URL(req.url, "http://x").pathname);
    let file = join(dir, url === "/" ? "index.html" : url);
    if (!file.startsWith(dir) || !existsSync(file) || statSync(file).isDirectory()) {
      file = join(dir, "index.html");
    }
    res.writeHead(200, { "Content-Type": MIME[extname(file)] ?? "application/octet-stream" });
    res.end(readFileSync(file));
  });
  return new Promise((ok) => server.listen(0, "127.0.0.1", () => ok(server)));
}

// ---------- in-page measurement (same method as bench/browser.mjs) ----------
function armLoadProbe() {
  window.__llLoaded = new Promise((resolve) => {
    window.addEventListener(
      "loglens:loaded",
      () => {
        requestAnimationFrame(() =>
          requestAnimationFrame(() => {
            const entry = (name) => performance.getEntriesByName(name).at(-1);
            const start = entry("loglens:read:start").startTime;
            resolve({
              total: performance.now() - start,
              read: entry("loglens:read")?.duration,
              copy: entry("loglens:copy")?.duration,
              parse: entry("loglens:parse")?.duration,
            });
          }),
        );
      },
      { once: true },
    );
  });
}

function measureInput([selector, value, eventName]) {
  const el = document.querySelector(selector);
  const t0 = performance.now();
  el.value = value;
  el.dispatchEvent(new Event(eventName, { bubbles: true }));
  return new Promise((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now() - t0))),
  );
}

const STATUS = ".filters select";
const PATH = ".filters input[type=search]";

async function measureOnce(browser, url, file) {
  const context = await browser.newContext();
  const page = await context.newPage();
  page.on("pageerror", (e) => console.error("  sayfa hatası:", e.message));
  await page.goto(url);
  await page.waitForSelector("input[type=file]", { state: "attached" });
  await page.evaluate(armLoadProbe);
  await page.setInputFiles("input[type=file]", file);
  const load = await Promise.race([
    page.evaluate(() => window.__llLoaded),
    new Promise((_, reject) =>
      setTimeout(
        () => reject(new Error('"loglens:loaded" olayı gelmedi. Bu sürümde ölçüm noktaları (8345d5c ve sonrası) var mı?')),
        120_000,
      ),
    ),
  ]);
  await page.waitForSelector(STATUS);
  const status = await page.evaluate(measureInput, [STATUS, "5", "change"]);
  await page.evaluate(measureInput, [STATUS, "", "change"]);
  const path = await page.evaluate(measureInput, [PATH, "checkout", "input"]);
  await page.evaluate(measureInput, [PATH, "", "input"]);
  await context.close();
  return { ...load, render: load.total - (load.read ?? 0) - (load.copy ?? 0) - (load.parse ?? 0), status, path };
}

// ---------- main ----------
const playwright = await import("playwright").catch(() => {
  console.error("Playwright bulunamadı. Önce: npm install && npx playwright install firefox");
  process.exit(1);
});
const browserType = playwright[BROWSER];
if (!browserType) {
  console.error(`Bilinmeyen tarayıcı: ${BROWSER} (firefox | chromium | webkit)`);
  process.exit(1);
}

const file = ensureInput();
const [refA, refB] = positional;
const A = prepare(refA);
const B = prepare(refB);
const servers = await Promise.all([serve(A.dist), serve(B.dist)]);
const urls = servers.map((s) => `http://127.0.0.1:${s.address().port}/`);

let browser;
try {
  browser = await browserType.launch({
    headless: !HEADED,
    firefoxUserPrefs: BROWSER === "firefox" ? { "privacy.reduceTimerPrecision": false } : undefined,
  });
} catch (e) {
  console.error(e.message.split("\n")[0]);
  console.error(`\nTarayıcı başlatılamadı. Kurmak için: npx playwright install ${BROWSER}`);
  process.exit(1);
}

console.log(`\n${BROWSER} ${browser.version()} · ${SIZE.toLocaleString("tr-TR")} satır · 1 ısınma + ${RUNS} tur, dönüşümlü\n`);
const results = { A: [], B: [] };
try {
  for (let round = 0; round <= RUNS; round++) {
    // Alternate the order every round (A B, B A, ...) so drift does not favour one side.
    const order = round % 2 === 0 ? ["A", "B"] : ["B", "A"];
    const line = [];
    for (const side of order) {
      const r = await measureOnce(browser, side === "A" ? urls[0] : urls[1], file);
      if (round > 0) results[side].push(r);
      line.push(`${side}: parse ${fmtMs(r.parse)}, toplam ${fmtMs(r.total)}`);
    }
    console.log(`  ${round === 0 ? "ısınma " : `tur ${String(round).padStart(2)}`}  ${line.join("  |  ")}`);
  }
} finally {
  await browser.close();
  servers.forEach((s) => s.close());
  A.cleanup();
  B.cleanup();
}

const rows = [
  ["read", "Dosya okuma"],
  ["copy", "JS → WASM kopyası"],
  ["parse", "Ayrıştırma (WASM)"],
  ["render", "İlk çizim"],
  ["total", "Dosya seçimi → panel ekranda"],
  ["status", "Status filtresi → ekranda"],
  ["path", "Path araması → ekranda"],
];
const table = [
  `| Ölçüm | A: ${A.label} | B: ${B.label} | Fark | B daha hızlı |`,
  "|---|---:|---:|---:|---:|",
];
for (const [key, label] of rows) {
  const a = results.A.map((r) => r[key]);
  const b = results.B.map((r) => r[key]);
  if (a.some((v) => v == null) || b.some((v) => v == null)) continue;
  const ma = median(a);
  const mb = median(b);
  const diff = ((mb - ma) / ma) * 100;
  // Paired by round: how often was B faster than A in the same round?
  const wins = a.filter((v, i) => b[i] < v).length;
  table.push(
    `| ${label} | ${fmtMs(ma)} | ${fmtMs(mb)} | ${diff >= 0 ? "+" : "−"}%${Math.abs(diff).toFixed(1)} | ${wins}/${a.length} tur |`,
  );
}
console.log(`\n${table.join("\n")}\n`);
console.log(
  `Okuma: "B daha hızlı" sütunu ${RUNS} turun büyük çoğunluğunda (ör. 10 turda 8+) ya da azınlığında (2-) ise fark büyük olasılıkla gerçektir; yarı yarıya ise gürültü düzeyindedir.`,
);
