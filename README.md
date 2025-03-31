# CSV Scout

[![Documentation](https://docs.rs/csv-scout/badge.svg)](https://docs.rs/csv-scout)

**CSV Scout** is a Rust library for inferring basic CSV metadata — currently focused on detecting the **delimiter** and **quote character**.


This is a fork of ([qsv-reader](https://github.com/jqnatividad/qsv-sniffer))


---

## 📦 Usage

```shell
cargo add csv-scout
```

Or directly to to Cargo.toml

```toml
[dependencies]
csv-scout = "*"
```

Import it in your crate:

```rust
use csv_scout;
```

### Example

```rust
use csv_scout;

fn main() {
    let path = "data/example.csv";
    match csv_scout::Sniffer::new().sniff_path(path) {
        Ok(metadata) => println!("{}", metadata),
        Err(err) => eprintln!("ERROR: {}", err),
    }
}
```

---

## 🔬 Feature Flags

- `runtime-dispatch-simd` – enables runtime SIMD detection for x86/x86_64 (SSE2, AVX2)
- `generic-simd` – enables architecture-independent SIMD (requires Rust nightly)

> These features are **mutually exclusive** and improve performance when sampling large files.
