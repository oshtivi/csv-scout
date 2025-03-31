extern crate csv;

use std::path::Path;

use csv_scout::metadata::{Dialect, Quote};

fn main() {
    let data_filepath = Path::new(file!())
        .parent()
        .unwrap()
        .join("../tests/data/gdp.csv");
    let dialect = Dialect {
        delimiter: b',',
        // header: Header {
        //     has_header_row: true,
        //     num_preamble_rows: 4,
        // },
        quote: Quote::Some(b'"'),
        // flexible: false,
        // is_utf8: true,
    };
    let mut reader = dialect.open_path(data_filepath).unwrap();
    let result = reader.records().next();
    let record = result.unwrap().unwrap();
    println!("{record:?}");
}
