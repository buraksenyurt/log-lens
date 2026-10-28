# Log Lens

Nginx access log'larını **tamamen tarayıcıda** analiz eden, Rust + [Leptos](https://leptos.dev) ile yazılmış bir WebAssembly uygulaması. Log dosyası hiçbir sunucuya gönderilmez.

## Çalıştırma

```bash
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
trunk serve --open

# release modda çalıştırmak için
trunk serve --release --open
```

Test verisi üretmek için:

```bash
python3 tools/logs_gen.py 500000 > samples/big.log
```

## Performans

### Native ölçümler (criterion)

Ayrıştırma, filtreleme ve istatistik kodu UI'dan bağımsız olduğu için tarayıcı olmadan, [criterion](https://crates.io/crates/criterion) ile ölçülebiliyor. Bu ölçümler kodun kendi hızını gösterir; tarayıcıdaki (WASM) süreler ayrıca ölçülmelidir.

Tek komutla girdi dosyalarını üretip benchmark'ları çalıştırmak ve aşağıdaki tabloyu güncellemek için:

```bash
python tools/bench_native.py               # veri üret + cargo bench + README'yi güncelle
python tools/bench_native.py --skip-bench  # sadece son sonuçlardan tabloyu yeniden yaz
```

- Girdiler `tools/logs_gen.py` ile (seed 42) üretilen 10.000, 100.000 ve 500.000 satırlık `samples/bench_*.log` dosyalarıdır.
- Benchmark kodu: [`benches/native.rs`](benches/native.rs). Sadece `cargo bench` ile de çalıştırılabilir (girdi dosyaları önceden üretilmiş olmalı).
- Grafikli ayrıntılı rapor: `target/criterion/report/index.html`
- Ölçüm sırasında dizüstü bilgisayarı prize takılı tutmak ve diğer uygulamaları kapatmak sonuçları daha kararlı hale getirir.

<!-- BENCH-NATIVE:START -->
| İşlem | 10.000 satır | 100.000 satır | 500.000 satır |
|---|---:|---:|---:|
| Ayrıştırma (parse_all) | 5.55 ms | 53.49 ms | 282.08 ms |
| Filtreleme (5xx + checkout) | 45.4 µs | 942.6 µs | 6.46 ms |
| Özet istatistik (summarize) | 160.1 µs | 2.46 ms | 13.58 ms |
| Saatlik dağılım (hourly) | 16.1 µs | 354.9 µs | 2.82 ms |
| Top endpoint'ler (top_endpoints) | 205.0 µs | 2.86 ms | 16.23 ms |
| Ayrıştırma hızı | 1.8 M satır/sn | 1.9 M satır/sn | 1.8 M satır/sn |

- **Tarih:** 2026-10-28
- **CPU:** 12th Gen Intel(R) Core(TM) i7-1255U
- **İşletim sistemi:** Windows 11 (AMD64)
- **Derleyici:** rustc 1.98.0 (88d9e12ae 2026-08-18), `bench` profili (release + LTO)
- **Veri:** `tools/logs_gen.py` (seed 42), değerler criterion medyanıdır
<!-- BENCH-NATIVE:END -->

### Tarayıcı ölçümleri (Firefox, uçtan uca)

Uygulamanın release build'i [Playwright](https://playwright.dev) ile headless Firefox'ta açılır, aynı `samples/bench_*.log` dosyaları yüklenir ve kullanıcının gördüğü süreler ölçülür. Uygulama; dosya okuma (`File.arrayBuffer()`), JS'ten WASM'a byte kopyası ve ayrıştırma adımlarını `performance.measure` ile (`loglens:read`, `loglens:copy`, `loglens:parse`) işaretler; bu ölçümler Firefox profiler'ında da görünür.

İlk kurulum (bir kez):

```bash
npm install
npx playwright install firefox
```

Ölçüm:

```bash
npm run bench                         # trunk build --release + ölçüm + README güncelleme
npm run bench -- --skip-build         # mevcut dist/ ile ölç
npm run bench -- --runs 7 --headed    # daha fazla tur, tarayıcıyı görerek
npm run bench -- --browser chromium   # başka motorla karşılaştırma (README güncellenmez)
```

- Her boyut için 1 ısınma + 5 ölçüm turu yapılır, her tur yeni bir sekmede başlar; tabloda medyan değerler yer alır.
- "→ ekranda" süreleri, olay tetiklendikten sonra iki animasyon karesi geçene kadar, yani sonuç boyanana kadar ölçülür. Bu yöntemin alt sınırı 60 Hz'de yaklaşık 1–2 karedir (~16–33 ms); bu aralıktaki değerler kullanıcı için "anında" demektir ve küçük farklar ölçüm gürültüsüdür.
- Native satırı ve WASM / native oranı, `python tools/bench_native.py` ile alınan son criterion sonuçlarından okunur.
- Firefox varsayılan olarak `performance.now()` hassasiyetini düşürür; ölçüm sırasında `privacy.reduceTimerPrecision` kapatılır.

<!-- BENCH-BROWSER:START -->
| Ölçüm | 10.000 satır | 100.000 satır | 500.000 satır |
|---|---:|---:|---:|
| Dosya okuma (`File.arrayBuffer()`) | 4.64 ms | 27.5 ms | 149 ms |
| JS → WASM kopyası (byte kopyası + UTF-8 doğrulama) | 0.92 ms | 5.30 ms | 27.4 ms |
| Ayrıştırma (WASM, parçalı) | 9.12 ms | 58.3 ms | 298 ms |
| İlk çizim (Memo'lar + DOM + boyama) | 26.9 ms | 31.0 ms | 74.9 ms |
| Dosya seçimi → panel ekranda | 43.4 ms | 124 ms | 544 ms |
| Status filtresi (5xx) → ekranda | 24.0 ms | 28.0 ms | 29.2 ms |
| Path araması (checkout) → ekranda | 37.0 ms | 37.5 ms | 57.5 ms |
| Native ayrıştırma (criterion) | 5.55 ms | 53.5 ms | 282 ms |
| WASM / native ayrıştırma oranı | 1.64× | 1.09× | 1.06× |

- **Tarih:** 2026-10-28
- **Tarayıcı:** Firefox 157.0 (Playwright 1.64.1, headless: true)
- **CPU:** 12th Gen Intel(R) Core(TM) i7-1255U
- **İşletim sistemi:** Windows 11 (build 26300, x64)
- **.wasm boyutu:** 506 KB (gzip: 171 KB)
- **Yöntem:** 5 ölçüm + 1 ısınma turu, her tur yeni sekme; değerler medyandır. Ayrıştırma, UI'ı dondurmamak için parçalar arasında tarayıcıya kontrol verdiğinden native süreyle birebir aynı iş değildir.
<!-- BENCH-BROWSER:END -->

### Optimizasyon günlüğü

Her adım ayrı bir commit; değerler 500.000 satırlık sütundan (medyan). Native satırlar criterion, diğerleri Firefox ölçümüdür.

| # | Değişiklik | Ölçüm | Önce | Sonra |
|---|---|---|---:|---:|
| 1 | `File.text()` yerine `File.arrayBuffer()`: UTF-8 → UTF-16 → UTF-8 dönüşümleri yerine tek byte kopyası | Dosya okuma | 281 ms | 152 ms (−%46) |
| | | JS → WASM kopyası | 176 ms | 29.1 ms (−%83) |
| | | Dosya seçimi → panel ekranda | 924 ms | **589 ms (−%36)** |
| 2 | İstatistikler: SipHash yerine `FxHash` (rustc-hash), p95 için tam sıralama yerine `select_nth_unstable` | `summarize` (native) | 21.34 ms | 13.58 ms (−%36) |
| | | `top_endpoints` (native) | 18.20 ms | 16.23 ms (−%11) |
| | | İlk çizim (Memo'lar + DOM + boyama) | 98.8 ms | 74.9 ms (−%24) |
| | | Dosya seçimi → panel ekranda | 589 ms | **544 ms (−%8)** |
| | | Path araması → ekranda | 57.7 ms | 57.5 ms (değişim yok) |

## Örnek Çalışma Zamanı Görüntüleri

![Runtime 00](runtime_00.png)

![Runtime 01](runtime_01.png)
