#!/usr/bin/env node
// End-to-end browser benchmark for Log Lens (Firefox by default, via Playwright).
//
// Usage (from the repository root):
//   npm run bench                         # build + measure in Firefox + update README.md
//   npm run bench -- --skip-build         # reuse the existing dist/ folder
//   npm run bench -- --runs 7 --headed    # more runs, visible browser window
//   npm run bench -- --browser chromium   # compare with another engine (README is not updated)
//
// What it does:
//   1. `trunk build --release` (unless --skip-build)
//   2. Generates samples/bench_<N>.log with tools/logs_gen.py if missing (same seed-42
//      inputs as the native criterion benchmarks)
//   3. Serves dist/ with a tiny static server and opens it in the browser
//   4. For every size: 1 warm-up + N measured page loads, reports the median of
//      - loglens:read / copy / parse     (performance.measure entries set by the app)
//      - load -> dashboard painted       (file pick until two animation frames later)
//      - status filter / path search     (input event until two animation frames later)
//   5. Writes a Markdown table between the BENCH-BROWSER markers in README.md

import { spawnSync } from "node:child_process";
import { createServer } from "node:http";
import { readFileSync, writeFileSync, existsSync, readdirSync, mkdirSync, openSync, closeSync, statSync } from "node:fs";
import { join, extname, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import os from "node:os";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const SIZES = [10_000, 100_000, 500_000];
const START = "<!-- BENCH-BROWSER:START -->";
const END = "<!-- BENCH-BROWSER:END -->";
const IS_WIN = process.platform === "win32";

// ---------- arguments ----------
const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const option = (name, fallback) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};
const RUNS = Number(option("--runs", "5"));
const BROWSER = option("--browser", "firefox");
const HEADED = flag("--headed");
const SKIP_BUILD = flag("--skip-build");

// ---------- helpers ----------
function run(cmd, cmdArgs, opts = {}) {
  console.log(`$ ${cmd} ${cmdArgs.join(" ")}`);
  const r = spawnSync(cmd, cmdArgs, { cwd: ROOT, stdio: "inherit", shell: IS_WIN, ...opts });
  if (r.status !== 0) {
    console.error(`\n"${cmd}" failed (exit code ${r.status}).`);
    process.exit(1);
  }
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

function generateInputs() {
  const python = process.env.PYTHON || (IS_WIN ? "python" : "python3");
  mkdirSync(join(ROOT, "samples"), { recursive: true });
  for (const n of SIZES) {
    const path = join(ROOT, "samples", `bench_${n}.log`);
    if (existsSync(path)) continue;
    // Same generator call as tools/bench_native.py: "\n" line endings on every OS.
    const code =
      "import sys, runpy; sys.stdout.reconfigure(newline='\\n'); " +
      `sys.argv = ['logs_gen.py', '${n}']; runpy.run_path('tools/logs_gen.py', run_name='__main__')`;
    const fd = openSync(path, "w");
    console.log(`$ ${python} tools/logs_gen.py ${n} > samples/bench_${n}.log`);
    const r = spawnSync(python, ["-c", code], { cwd: ROOT, stdio: ["ignore", fd, "inherit"] });
    closeSync(fd);
    if (r.status !== 0) {
      console.error("Generating inputs failed. Set the PYTHON environment variable if python is not on PATH.");
      process.exit(1);
    }
  }
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

function wasmSize(dir) {
  const wasm = readdirSync(dir).find((f) => f.endsWith("_bg.wasm"));
  if (!wasm) return null;
  const bytes = readFileSync(join(dir, wasm));
  return { raw: bytes.length, gzip: gzipSync(bytes, { level: 9 }).length };
}

function nativeParseMs(n) {
  // Median of the criterion run (tools/bench_native.py), if available.
  try {
    const meta = spawnSync("cargo", ["metadata", "--format-version", "1", "--no-deps"], {
      cwd: ROOT, encoding: "utf8", shell: IS_WIN,
    });
    const target = JSON.parse(meta.stdout).target_directory;
    const est = join(target, "criterion", String(n), "parse", "new", "estimates.json");
    return JSON.parse(readFileSync(est, "utf8")).median.point_estimate / 1e6;
  } catch {
    return null;
  }
}

// ---------- in-page measurement code ----------
// Resolves when the app reports "loglens:loaded" (dataset set) and two animation
// frames have passed (= the dashboard has been painted).
function armLoadProbe() {
  window.__llLoaded = new Promise((resolve) => {
    window.addEventListener("loglens:loaded", () => {
      requestAnimationFrame(() =>
        requestAnimationFrame(() => {
          const start = performance.getEntriesByName("loglens:read:start").at(-1).startTime;
          resolve({
            total: performance.now() - start,
            read: performance.getEntriesByName("loglens:read").at(-1).duration,
            copy: performance.getEntriesByName("loglens:copy").at(-1).duration,
            parse: performance.getEntriesByName("loglens:parse").at(-1).duration,
          });
        }),
      );
    }, { once: true });
  });
}

// Changes a bound form field like a user would and waits until the result is painted.
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

async function measureSize(browser, baseUrl, n) {
  const file = join(ROOT, "samples", `bench_${n}.log`);
  const samples = { read: [], copy: [], parse: [], render: [], total: [], status: [], path: [] };

  for (let i = 0; i <= RUNS; i++) {
    const context = await browser.newContext();
    const page = await context.newPage();
    page.on("pageerror", (e) => console.error("  page error:", e.message));
    await page.goto(baseUrl);
    await page.waitForSelector("input[type=file]", { state: "attached" });
    await page.evaluate(armLoadProbe);
    await page.setInputFiles("input[type=file]", file);
    const load = await page.evaluate(() => window.__llLoaded);
    await page.waitForSelector(STATUS);

    const status = await page.evaluate(measureInput, [STATUS, "5", "change"]);
    await page.evaluate(measureInput, [STATUS, "", "change"]);
    const path = await page.evaluate(measureInput, [PATH, "checkout", "input"]);
    await page.evaluate(measureInput, [PATH, "", "input"]);
    await context.close();

    const label = i === 0 ? "warm-up" : `run ${i}/${RUNS}`;
    console.log(
      `  ${String(n).padStart(7)} ${label.padEnd(9)} read ${fmtMs(load.read)}, copy ${fmtMs(load.copy)}, ` +
        `parse ${fmtMs(load.parse)}, ` +
        `total ${fmtMs(load.total)}, status ${fmtMs(status)}, path ${fmtMs(path)}`,
    );
    if (i === 0) continue; // discard warm-up
    samples.read.push(load.read);
    samples.copy.push(load.copy);
    // Whatever is left: Memo chain + first DOM render + paint.
    samples.render.push(load.total - load.read - load.copy - load.parse);
    samples.parse.push(load.parse);
    samples.total.push(load.total);
    samples.status.push(status);
    samples.path.push(path);
  }
  return Object.fromEntries(Object.entries(samples).map(([k, v]) => [k, median(v)]));
}

function cpuName() {
  return os.cpus()[0]?.model?.trim() ?? "bilinmiyor";
}

function osName() {
  if (os.type() === "Windows_NT") {
    // os.release() reports 10.0.<build> on both Windows 10 and 11; builds >= 22000 are Windows 11.
    const build = Number(os.release().split(".")[2] ?? 0);
    return `Windows ${build >= 22000 ? 11 : 10} (build ${build}, ${os.arch()})`;
  }
  const names = { Darwin: "macOS", Linux: "Linux" };
  return `${names[os.type()] ?? os.type()} ${os.release()} (${os.arch()})`;
}

function buildTable(results, env) {
  const head = `| Ölçüm | ${SIZES.map((n) => `${n.toLocaleString("tr-TR")} satır`).join(" | ")} |`;
  const sep = `|---|${SIZES.map(() => "---:").join("|")}|`;
  const row = (label, f) => `| ${label} | ${SIZES.map((n) => f(n)).join(" | ")} |`;
  const native = Object.fromEntries(SIZES.map((n) => [n, nativeParseMs(n)]));
  const lines = [
    head,
    sep,
    row("Dosya okuma (`File.arrayBuffer()`)", (n) => fmtMs(results[n].read)),
    row("JS → WASM kopyası (byte kopyası + UTF-8 doğrulama)", (n) => fmtMs(results[n].copy)),
    row("Ayrıştırma (WASM, parçalı)", (n) => fmtMs(results[n].parse)),
    row("İlk çizim (Memo'lar + DOM + boyama)", (n) => fmtMs(results[n].render)),
    row("Dosya seçimi → panel ekranda", (n) => fmtMs(results[n].total)),
    row("Status filtresi (5xx) → ekranda", (n) => fmtMs(results[n].status)),
    row("Path araması (checkout) → ekranda", (n) => fmtMs(results[n].path)),
    row("Native ayrıştırma (criterion)", (n) => fmtMs(native[n])),
    row("WASM / native ayrıştırma oranı", (n) =>
      native[n] ? `${(results[n].parse / native[n]).toFixed(2)}×` : "—",
    ),
  ];
  const size = env.wasm
    ? `${(env.wasm.raw / 1024).toFixed(0)} KB (gzip: ${(env.wasm.gzip / 1024).toFixed(0)} KB)`
    : "—";
  return (
    lines.join("\n") +
    "\n\n" +
    [
      `- **Tarih:** ${new Date().toISOString().slice(0, 10)}`,
      `- **Tarayıcı:** ${env.browser} ${env.version} (Playwright ${env.playwright}, headless: ${!HEADED})`,
      `- **CPU:** ${cpuName()}`,
      `- **İşletim sistemi:** ${osName()}`,
      `- **.wasm boyutu:** ${size}`,
      `- **Yöntem:** ${RUNS} ölçüm + 1 ısınma turu, her tur yeni sekme; değerler medyandır. ` +
        "Ayrıştırma, UI'ı dondurmamak için parçalar arasında tarayıcıya kontrol verdiğinden native süreyle birebir aynı iş değildir.",
    ].join("\n")
  );
}

function updateReadme(block) {
  const path = join(ROOT, "README.md");
  const raw = readFileSync(path, "utf8");
  const crlf = raw.includes("\r\n");
  let text = raw.replace(/\r\n/g, "\n");
  const s = text.indexOf(START);
  const e = text.indexOf(END);
  if (s < 0 || e < 0) {
    console.error(`README.md içinde ${START} / ${END} işaretleri bulunamadı.`);
    process.exit(1);
  }
  text = `${text.slice(0, s)}${START}\n${block}\n${text.slice(e)}`;
  writeFileSync(path, crlf ? text.replace(/\n/g, "\r\n") : text);
  console.log("README.md güncellendi.");
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

if (!SKIP_BUILD) run("trunk", ["build", "--release"]);
const dist = join(ROOT, "dist");
if (!existsSync(join(dist, "index.html"))) {
  console.error("dist/index.html yok. --skip-build olmadan çalıştırın.");
  process.exit(1);
}
generateInputs();

const server = await serve(dist);
const baseUrl = `http://127.0.0.1:${server.address().port}/`;

let browser;
try {
  browser = await browserType.launch({
    headless: !HEADED,
    // Firefox rounds performance.now() to ~1 ms by default; full precision for benchmarking.
    firefoxUserPrefs: BROWSER === "firefox" ? { "privacy.reduceTimerPrecision": false } : undefined,
  });
} catch (e) {
  console.error(e.message.split("\n")[0]);
  console.error(`\nTarayıcı başlatılamadı. Kurmak için: npx playwright install ${BROWSER}`);
  server.close();
  process.exit(1);
}

const results = {};
console.log(`\n${BROWSER} ${browser.version()} · ${RUNS} ölçüm + 1 ısınma\n`);
for (const n of SIZES) results[n] = await measureSize(browser, baseUrl, n);
const version = browser.version();
await browser.close();
server.close();

const pwVersion = JSON.parse(readFileSync(join(ROOT, "node_modules", "playwright", "package.json"), "utf8")).version;
const names = { firefox: "Firefox", chromium: "Chromium", webkit: "WebKit" };
const block = buildTable(results, {
  browser: names[BROWSER],
  version,
  playwright: pwVersion,
  wasm: wasmSize(dist),
});
console.log(`\n${block}\n`);
if (BROWSER === "firefox") updateReadme(block);
else console.log("README yalnızca Firefox ölçümüyle güncellenir.");
