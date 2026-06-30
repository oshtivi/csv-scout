use std::path::Path;

use csv_scout::{
    SampleSize, Sniffer,
    metadata::{Dialect, Metadata, Quote},
};

#[test]
fn test_double_quote() {
    let data_filepath = Path::new(file!())
        .parent()
        .unwrap()
        .join("data/double_quoted.csv");
    let metadata = Sniffer::new()
        .sample_size(SampleSize::All)
        .sniff_path(data_filepath)
        .unwrap();
    assert_eq!(
        metadata,
        Metadata {
            dialect: Dialect {
                delimiter: b',',
                quote: Quote::Some(b'"'),
            },
        }
    );
}

#[test]
fn test_most_fields_unquoted() {
    let data_filepath = Path::new(file!())
        .parent()
        .unwrap()
        .join("data/most_fields_unquoted.csv");
    let metadata = Sniffer::new()
        .sample_size(SampleSize::All)
        .sniff_path(data_filepath)
        .unwrap();
    assert_eq!(
        metadata,
        Metadata {
            dialect: Dialect {
                delimiter: b',',
                quote: Quote::Some(b'"'),
            },
        }
    );
}

#[test]
fn test_flaky_quote_detection() {
    let data_filepath = Path::new(file!())
        .parent()
        .unwrap()
        .join("data/not_properly_escaped.csv");
    let metadata = Sniffer::new()
        .sample_size(SampleSize::All)
        .sniff_path(data_filepath)
        .unwrap();
    assert_eq!(
        metadata,
        Metadata {
            dialect: Dialect {
                delimiter: b',',
                quote: Quote::Some(b'"'),
            },
        }
    );
}

// Quoted JSON cells contain colons, producing an exact tie between ',' and ':'
// in the candidate-delimiter tally. The winner must be deterministic (comma),
// not dependent on HashMap iteration order. Repeat to defeat the per-run random seed.
#[test]
fn test_colon_in_quoted_json_is_stable() {
    let data_filepath = Path::new(file!())
        .parent()
        .unwrap()
        .join("data/colon_in_quoted_json.csv");
    let expected = Metadata {
        dialect: Dialect {
            delimiter: b',',
            quote: Quote::Some(b'"'),
        },
    };
    for _ in 0..50 {
        let metadata = Sniffer::new()
            .sample_size(SampleSize::All)
            .sniff_path(&data_filepath)
            .unwrap();
        assert_eq!(metadata, expected);
    }
}

#[test]
fn test_multiline_quoted_field() {
    let data_filepath = Path::new(file!())
        .parent()
        .unwrap()
        .join("data/multi-line-cell.csv");
    let metadata = Sniffer::new()
        .sample_size(SampleSize::All)
        .sniff_path(data_filepath)
        .unwrap();
    assert_eq!(
        metadata,
        Metadata {
            dialect: Dialect {
                delimiter: b',',
                quote: Quote::Some(b'"'),
            },
        }
    );
}
