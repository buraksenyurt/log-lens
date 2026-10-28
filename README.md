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

## Örnek Çalışma Zamanı Görüntüleri

![Runtime 00](runtime_00.png)

![Runtime 01](runtime_01.png)
