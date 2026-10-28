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
| Ayrıştırma (parse_all) | 5.41 ms | 59.84 ms | 285.39 ms |
| Filtreleme (5xx + checkout) | 8.4 µs | 358.6 µs | 3.01 ms |
| Path araması (checkout) | 15.0 µs | 436.7 µs | 3.65 ms |
| Özet istatistik (summarize) | 161.0 µs | 2.59 ms | 14.54 ms |
| Saatlik dağılım (hourly) | 17.5 µs | 481.6 µs | 3.22 ms |
| Top endpoint'ler (top_endpoints) | 24.6 µs | 505.8 µs | 3.96 ms |
| Ayrıştırma hızı | 1.8 M satır/sn | 1.7 M satır/sn | 1.8 M satır/sn |

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
| Dosya okuma (`File.arrayBuffer()`) | 5.14 ms | 33.8 ms | 177 ms |
| JS → WASM kopyası (byte kopyası + UTF-8 doğrulama) | 0.88 ms | 6.00 ms | 29.1 ms |
| Ayrıştırma (WASM, parçalı) | 8.54 ms | 75.4 ms | 323 ms |
| İlk çizim (Memo'lar + DOM + boyama) | 20.1 ms | 28.7 ms | 56.3 ms |
| Dosya seçimi → panel ekranda | 34.5 ms | 145 ms | 573 ms |
| Status filtresi (5xx) → ekranda | 14.8 ms | 26.1 ms | 26.7 ms |
| Path araması (checkout) → ekranda | 36.5 ms | 37.5 ms | 37.7 ms |
| Native ayrıştırma (criterion) | 5.41 ms | 59.8 ms | 285 ms |
| WASM / native ayrıştırma oranı | 1.58× | 1.26× | 1.13× |

- **Tarih:** 2026-10-28
- **Tarayıcı:** Firefox 157.0 (Playwright 1.64.1, headless: true)
- **CPU:** 12th Gen Intel(R) Core(TM) i7-1255U
- **İşletim sistemi:** Windows 11 (build 26300, x64)
- **.wasm boyutu:** 530 KB (gzip: 177 KB)
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
| 3 | Path interning: her farklı path bir kez saklanır, kayıtlar sadece numarasını tutar (`PathTable`); path araması farklı path başına bir kez yapılır, `top_endpoints` hash tablosu yerine dizi sayacı kullanır | Path araması → ekranda | 57.5 ms | **37.3 ms (−%35)** |
| | | `top_endpoints` (native) | 16.23 ms | 3.75 ms (−%77) |
| | | Filtreleme, 5xx + checkout (native) | 6.46 ms | 2.92 ms (−%55) |
| | | İlk çizim (Memo'lar + DOM + boyama) | 74.9 ms | 53.2 ms (−%29) |
| | | Ayrıştırma (native) | 282.08 ms | 285.39 ms (≈ aynı) |
| | | Dosya seçimi → panel ekranda | 544 ms | 573 ms (+%5)¹ |
| 4 | Tüm metin alanlarını numaralandırma: IP, method, user agent, tarih ve saat dilimi de `Tables` içinde bir kez saklanır, kayıt tamamen sayılardan oluşur (heap ayırması yok, ~56 byte) | Ayrıştırma (native) | 285.39 ms | _ölçülecek_ |
| | | `summarize` (native) | 14.54 ms | _ölçülecek_ |
| | | Ayrıştırma (Firefox) | 323 ms | _ölçülecek_ |
| | | Dosya seçimi → panel ekranda | 573 ms | _ölçülecek_ |

¹ İlk ölçümde 612 ms çıkmıştı; kodu değişmeyen adımlar da yavaşladığı için ölçüm tekrarlandı (tablodaki değerler tekrardan). Kalan farkın çoğu kodu değişmeyen dosya okumasından geliyor (149 → 177 ms); bu adım tur tura ±%15 oynuyor.

## Örnek Çalışma Zamanı Görüntüleri

![Runtime 00](runtime_00.png)

![Runtime 01](runtime_01.png)
