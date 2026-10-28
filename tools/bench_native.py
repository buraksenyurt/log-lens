#!/usr/bin/env python3
"""Runs the native (criterion) benchmarks and writes the results into README.md.

Usage (from the repository root):

    python tools/bench_native.py               # generate inputs + cargo bench + update README
    python tools/bench_native.py --skip-bench  # only rebuild the README table from the last run

Steps:
  1. Generates samples/bench_<N>.log with tools/logs_gen.py (seed 42, deterministic).
  2. Runs `cargo bench --bench native`.
  3. Reads target/criterion/<N>/<op>/new/estimates.json (median).
  4. Replaces the block between the BENCH-NATIVE markers in README.md.

Only the Python standard library is used.
"""
import argparse
import datetime
import json
import platform
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SIZES = [10_000, 100_000, 500_000]
OPS = [
    ("parse", "Ayrıştırma (parse_all)"),
    ("filter", "Filtreleme (5xx + checkout)"),
    ("summarize", "Özet istatistik (summarize)"),
    ("hourly", "Saatlik dağılım (hourly)"),
    ("top_endpoints", "Top endpoint'ler (top_endpoints)"),
]
START = "<!-- BENCH-NATIVE:START -->"
END = "<!-- BENCH-NATIVE:END -->"


def run(cmd, **kw):
    print("$", " ".join(str(c) for c in cmd), flush=True)
    return subprocess.run(cmd, cwd=ROOT, check=True, **kw)


def generate_inputs():
    samples = ROOT / "samples"
    samples.mkdir(exist_ok=True)
    for n in SIZES:
        path = samples / f"bench_{n}.log"
        if path.exists():
            print(f"= {path.relative_to(ROOT)} already exists")
            continue
        # Force "\n" line endings so the input is byte-identical on every OS.
        code = (
            "import sys, runpy; sys.stdout.reconfigure(newline='\\n'); "
            f"sys.argv = ['logs_gen.py', '{n}']; "
            "runpy.run_path('tools/logs_gen.py', run_name='__main__')"
        )
        with open(path, "wb") as out:
            run([sys.executable, "-c", code], stdout=out)


def target_dir():
    meta = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=ROOT, check=True, capture_output=True, text=True,
    )
    return Path(json.loads(meta.stdout)["target_directory"])


def median_ns(criterion_dir, n, op):
    est = criterion_dir / str(n) / op / "new" / "estimates.json"
    if not est.exists():
        return None
    return json.loads(est.read_text(encoding="utf-8"))["median"]["point_estimate"]


def fmt_time(ns):
    if ns is None:
        return "—"
    if ns >= 1e9:
        return f"{ns / 1e9:.2f} s"
    if ns >= 1e6:
        return f"{ns / 1e6:.2f} ms"
    if ns >= 1e3:
        return f"{ns / 1e3:.1f} µs"
    return f"{ns:.0f} ns"


def fmt_rate(lines, ns):
    if not ns:
        return "—"
    per_sec = lines / (ns / 1e9)
    return f"{per_sec / 1e6:.1f} M satır/sn"


def cpu_name():
    system = platform.system()
    try:
        if system == "Windows":
            out = subprocess.run(
                ["powershell", "-NoProfile", "-Command", "(Get-CimInstance Win32_Processor).Name"],
                capture_output=True, text=True, check=True,
            ).stdout
            return out.strip().splitlines()[0].strip()
        if system == "Darwin":
            return subprocess.run(
                ["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, text=True, check=True
            ).stdout.strip()
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    except Exception:
        pass
    return platform.processor() or "bilinmiyor"


def rustc_version():
    try:
        return subprocess.run(["rustc", "-V"], capture_output=True, text=True, check=True).stdout.strip()
    except Exception:
        return "bilinmiyor"


def build_table(criterion_dir):
    header = "| İşlem | " + " | ".join(f"{n:,} satır".replace(",", ".") for n in SIZES) + " |"
    sep = "|---|" + "---:|" * len(SIZES)
    rows = [header, sep]
    for op, label in OPS:
        cells = [fmt_time(median_ns(criterion_dir, n, op)) for n in SIZES]
        rows.append(f"| {label} | " + " | ".join(cells) + " |")
    rates = [fmt_rate(n, median_ns(criterion_dir, n, "parse")) for n in SIZES]
    rows.append("| Ayrıştırma hızı | " + " | ".join(rates) + " |")

    env = (
        f"- **Tarih:** {datetime.date.today().isoformat()}\n"
        f"- **CPU:** {cpu_name()}\n"
        f"- **İşletim sistemi:** {platform.system()} {platform.release()} ({platform.machine()})\n"
        f"- **Derleyici:** {rustc_version()}, `bench` profili (release + LTO)\n"
        f"- **Veri:** `tools/logs_gen.py` (seed 42), değerler criterion medyanıdır"
    )
    return "\n".join(rows) + "\n\n" + env


def update_readme(block):
    readme = ROOT / "README.md"
    raw = readme.read_bytes().decode("utf-8")
    crlf = "\r\n" in raw
    text = raw.replace("\r\n", "\n")
    if START not in text or END not in text:
        sys.exit(f"README.md içinde {START} / {END} işaretleri bulunamadı.")
    before, rest = text.split(START, 1)
    _, after = rest.split(END, 1)
    text = f"{before}{START}\n{block}\n{END}{after}"
    if crlf:
        text = text.replace("\n", "\r\n")
    readme.write_bytes(text.encode("utf-8"))
    print("README.md güncellendi.")


def main():
    # Windows consoles may not default to UTF-8 (µ, Turkish characters).
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--skip-bench", action="store_true", help="cargo bench çalıştırmadan son sonuçlardan tablo üret")
    args = parser.parse_args()

    if not args.skip_bench:
        generate_inputs()
        run(["cargo", "bench", "--bench", "native"])

    block = build_table(target_dir() / "criterion")
    print()
    print(block)
    print()
    update_readme(block)


if __name__ == "__main__":
    main()
