use std::path::Path;

use csv_core as csvc;
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
