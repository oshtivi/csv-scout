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

### Metadata

`Sniffer::sniff_path` / `sniff_reader` return a `Metadata` with:

| Field        | Type         | Description                                                                 |
|--------------|--------------|-----------------------------------------------------------------------------|
| `dialect`    | `Dialect`    | Detected `delimiter` and `quote`.                                           |
| `steadiness` | `Steadiness` | `SteadyStrict` (every record has the same field count), `SteadyFlex` (most records have the max field count) or `Unsteady` (no consistent structure). |
| `num_fields` | `usize`      | (Maximum) fields per record. `1` means the delimiter never appears.         |
| `is_utf8`    | `bool`       | Whether the sampled data is valid UTF-8.                                    |

### Custom delimiter candidates

By default the delimiter is chosen from `csv_scout::DEFAULT_CANDIDATES` (`b"\t,;|:"`).
Use `candidates` to override the set, e.g. to detect Hive/Hadoop `^A` (`0x01`) or the
ASCII Unit Separator (`0x1F`). Only ASCII bytes are supported; line breaks (`\n`, `\r`) are
ignored since they terminate records.

```rust
use csv_scout::Sniffer;

let metadata = Sniffer::new()
    .candidates(&[b'|', 0x01, 0x1F])
    .sniff_path("unload.dat")?;
```

### Validating that data is tabular

By default the sniffer always returns a best-effort dialect (falling back to `,` with no quotes
for non-delimited input). Set `require_steady(true)` to instead fail with
`SnifferError::SniffingFailed` when the input is not steadily delimited — i.e. the chosen
delimiter never appears (`num_fields < 2`) or the steadiness is `Unsteady` (prose, logs, binary...).

```rust
use csv_scout::{Sniffer, metadata::Steadiness};

match Sniffer::new()
    .candidates(&[b'\t', b',', b';', b'|', 0x01, 0x1F])
    .require_steady(true)
    .sniff_path("export.dat")
{
    Ok(metadata) => {
        // safe to extract with metadata.dialect
        assert!(metadata.num_fields >= 2);
        assert_ne!(metadata.steadiness, Steadiness::Unsteady);
    }
    Err(err) => eprintln!("not tabular: {err}"),
}
```

Setting `candidates` or `require_steady(true)` also makes delimiter selection use a numerically
stable (log-space) chain search, which stays accurate on long samples. Callers that set neither
get exactly the same delimiter/quote detection as previous versions.

---

## 🔬 Feature Flags

- `runtime-dispatch-simd` – enables runtime SIMD detection for x86/x86_64 (SSE2, AVX2)
- `generic-simd` – enables architecture-independent SIMD (requires Rust nightly)

> These features are **mutually exclusive** and improve performance when sampling large files.
