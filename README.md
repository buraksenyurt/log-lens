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
_Henüz ölçüm yapılmadı. `python tools/bench_native.py` komutunu çalıştırın._
<!-- BENCH-NATIVE:END -->

## Örnek Çalışma Zamanı Görüntüleri

![Runtime 00](runtime_00.png)

![Runtime 01](runtime_01.png)
