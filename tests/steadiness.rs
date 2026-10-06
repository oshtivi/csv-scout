//! Tests for configurable delimiter candidates, steadiness/confidence reporting on `Metadata`,
//! and the opt-in `require_steady` tabular validation.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use csv_scout::{
    SampleSize, Sniffer,
    error::SnifferError,
    metadata::{Dialect, Metadata, Quote, Steadiness},
};

fn data(name: &str) -> PathBuf {
    Path::new(file!()).parent().unwrap().join("data").join(name)
}

fn sniff(sniffer: &mut Sniffer, content: &[u8]) -> csv_scout::error::Result<Metadata> {
    sniffer.sniff_reader(Cursor::new(content.to_vec()))
}

/// `rows` records of `cols` fields joined by `delim`.
fn delimited(delim: u8, rows: usize, cols: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            if c > 0 {
                out.push(delim);
            }
            out.extend_from_slice(format!("r{r}c{c}").as_bytes());
        }
        out.push(b'\n');
    }
    out
}

const PROSE: &str = "\
It was the best of times, it was the worst of times, it was the age of wisdom.
It was the age of foolishness; it was the epoch of belief.
In short, the period was so far like the present period: some of its noisiest authorities
insisted on its being received, for good or for evil, in the superlative degree of comparison only.
There were a king with a large jaw and a queen with a plain face, on the throne of England.
It was clearer than crystal to the lords of the State preserves of loaves and fishes.
That things in general were settled for ever.
France, less favoured on the whole as to matters spiritual than her sister of the shield and trident,
rolled with exceeding smoothness down hill, making paper money and spending it.
Under the guidance of her Christian pastors, she entertained herself, besides, with such humane
achievements as sentencing a youth to have his hands cut off: his tongue torn out with pincers.
All these things, and a thousand like them, came to pass in and close upon the dear old year.
";

const LOGS: &str = "\
2026-10-04 12:00:01 INFO  server started on port 8080
2026-10-04 12:00:02 DEBUG loaded config: path=/etc/app.yaml, env=prod
2026-10-04 12:00:05 WARN  slow request: GET /api/v1/users took 1532ms
2026-10-04 12:00:07 INFO  user login ok
2026-10-04 12:00:09 ERROR db timeout; retrying (attempt 1|3)
2026-10-04 12:00:10 ERROR db timeout; retrying (attempt 2|3), backoff: 200ms, jitter: 15ms
2026-10-04 12:00:12 INFO  db reconnected
2026-10-04 12:00:15 INFO  request id=abc,def,ghi status=200
2026-10-04 12:00:16 DEBUG cache stats: hits=10 misses=2 ratio=0.83
2026-10-04 12:00:20 INFO  shutting down
2026-10-04 12:00:21 INFO  bye: drained 3 connections; 0 pending, 1 aborted, 2 closed
2026-10-04 12:00:22 INFO  exit code 0
";

// ---------------------------------------------------------------------------------------------
// 1. Default behavior is unchanged
// ---------------------------------------------------------------------------------------------

#[test]
fn defaults_unchanged_on_fixtures() {
    let cases: &[(&str, u8, Quote)] = &[
        ("library-visitors.csv", b',', Quote::None),
        ("2016_presidential_election_durham.csv", b';', Quote::None),
        ("tab_separated.csv", b'\t', Quote::None),
        ("boston311.csv", b',', Quote::None),
        ("double_quoted.csv", b',', Quote::Some(b'"')),
        ("colon_in_quoted_json.csv", b',', Quote::Some(b'"')),
    ];
    for (file, delimiter, quote) in cases {
        let expected = Dialect {
            delimiter: *delimiter,
            quote: quote.clone(),
        };
        // implicit defaults
        let implicit = Sniffer::new()
            .sample_size(SampleSize::All)
            .sniff_path(data(file))
            .unwrap();
        assert_eq!(implicit.dialect, expected, "{file}");
        // explicitly passing the default configuration yields the same full metadata
        let explicit = Sniffer::new()
            .sample_size(SampleSize::All)
            .candidates(csv_scout::DEFAULT_CANDIDATES)
            .require_steady(false)
            .sniff_path(data(file))
            .unwrap();
        assert_eq!(explicit, implicit, "{file}");
    }
}

#[test]
fn default_candidates_value() {
    assert_eq!(csv_scout::DEFAULT_CANDIDATES, b"\t,;|:");
}

#[test]
fn default_does_not_fail_on_prose() {
    // Without require_steady, non-tabular input still yields a best-effort dialect (legacy
    // behavior), but the metadata exposes that it is not steadily tabular.
    let m = sniff(&mut Sniffer::new(), PROSE.as_bytes()).unwrap();
    assert_eq!(m.dialect.quote, Quote::None);
    assert_eq!(m.steadiness, Steadiness::Unsteady);
}

#[test]
fn default_falls_back_to_comma_without_any_delimiter() {
    let content = b"alpha\nbeta\ngamma\ndelta\n";
    let m = sniff(&mut Sniffer::new(), content).unwrap();
    assert_eq!(
        m.dialect,
        Dialect {
            delimiter: b',',
            quote: Quote::None
        }
    );
    assert_eq!(m.steadiness, Steadiness::Unsteady);
    assert_eq!(m.num_fields, 1);
}

#[test]
fn default_ignores_non_default_separators() {
    // ^A is not a default candidate, so it is not detected without opting in
    let m = sniff(&mut Sniffer::new(), &delimited(0x01, 20, 4)).unwrap();
    assert_ne!(m.dialect.delimiter, 0x01);
}

// ---------------------------------------------------------------------------------------------
// 2. Custom candidates
// ---------------------------------------------------------------------------------------------

#[test]
fn custom_candidates_detect_hive_ctrl_a() {
    let m = sniff(
        Sniffer::new().candidates(&[b'|', 0x01, 0x1F]),
        &delimited(0x01, 50, 6),
    )
    .unwrap();
    assert_eq!(
        m,
        Metadata {
            dialect: Dialect {
                delimiter: 0x01,
                quote: Quote::None
            },
            steadiness: Steadiness::SteadyStrict,
            num_fields: 6,
            is_utf8: true,
        }
    );
}

#[test]
fn custom_candidates_detect_unit_separator() {
    let m = sniff(
        Sniffer::new().candidates(&[b'|', 0x01, 0x1F]),
        &delimited(0x1F, 30, 3),
    )
    .unwrap();
    assert_eq!(m.dialect.delimiter, 0x1F);
    assert_eq!(m.steadiness, Steadiness::SteadyStrict);
    assert_eq!(m.num_fields, 3);
}

#[test]
fn custom_candidates_detect_pipe_dat_unload() {
    // typical database unload: pipe-delimited with trailing delimiter, values contain ',' and ':'
    let mut content = String::new();
    for i in 0..40 {
        content.push_str(&format!(
            "{i}|Smith, John|2026-01-{:02} 10:{:02}:00|{}.50|\n",
            i % 28 + 1,
            i % 60,
            i * 3
        ));
    }
    let m = sniff(
        Sniffer::new()
            .candidates(&[b'|', 0x01, 0x1F])
            .require_steady(true),
        content.as_bytes(),
    )
    .unwrap();
    assert_eq!(m.dialect.delimiter, b'|');
    assert_eq!(m.steadiness, Steadiness::SteadyStrict);
    assert_eq!(m.num_fields, 5); // 4 values + empty trailing field
}

#[test]
fn custom_candidates_restrict_detection() {
    // the data is comma-delimited, but ',' is not a candidate: '|' appears only on some lines
    let mut content = delimited(b',', 20, 4);
    content.extend_from_slice(b"a|b\n");
    let m = sniff(Sniffer::new().candidates(b"|"), &content).unwrap();
    assert_ne!(m.dialect.delimiter, b',');
}

#[test]
fn custom_candidates_with_regex_metacharacters_and_quotes() {
    // quoted data where the delimiter is ^A: exercises the quote-detection regex with
    // non-default (and non-printable) candidates
    let mut content = Vec::new();
    for i in 0..20 {
        content.extend_from_slice(format!("{i}\x01\"name, {i}\"\x01\"x|y\"\n").as_bytes());
    }
    let m = sniff(
        Sniffer::new().candidates(&[0x01, b'|', b']', b'-', b'^']),
        &content,
    )
    .unwrap();
    assert_eq!(m.dialect.delimiter, 0x01);
    assert_eq!(m.dialect.quote, Quote::Some(b'"'));
    assert_eq!(m.num_fields, 3);
    assert_eq!(m.steadiness, Steadiness::SteadyStrict);
}

#[test]
fn custom_candidates_more_than_six() {
    let candidates = b"\t,;|:\x01\x1F~^";
    let m = sniff(
        Sniffer::new().candidates(candidates),
        &delimited(b'~', 25, 5),
    )
    .unwrap();
    assert_eq!(m.dialect.delimiter, b'~');
    assert_eq!(m.num_fields, 5);
}

#[test]
fn custom_candidates_line_breaks_ignored() {
    // '\n' / '\r' are record terminators, never field delimiters
    let m = sniff(Sniffer::new().candidates(b"\n\r|"), &delimited(b'|', 20, 4)).unwrap();
    assert_eq!(m.dialect.delimiter, b'|');
    assert_eq!(m.num_fields, 4);

    // only line breaks -> effectively empty set
    let err = sniff(Sniffer::new().candidates(b"\r\n"), &delimited(b',', 5, 3)).unwrap_err();
    assert!(matches!(err, SnifferError::SniffingFailed(_)), "{err}");
}

#[test]
fn user_delimiter_regex_metacharacters_with_quotes() {
    // A user-supplied delimiter is interpolated into the quote-detection regex; it must be
    // matched literally (and must not panic) for regex metacharacters.
    for &delim in br"|[(.*+?^$\{" {
        let d = char::from(delim);
        let mut content = String::new();
        for i in 0..10 {
            content.push_str(&format!("{i}{d}\"name {i}\"{d}x\n"));
        }
        let m = sniff(Sniffer::new().delimiter(delim), content.as_bytes())
            .unwrap_or_else(|e| panic!("delimiter {d:?}: {e}"));
        assert_eq!(m.dialect.delimiter, delim, "{d:?}");
        assert_eq!(m.dialect.quote, Quote::Some(b'"'), "{d:?}");
        assert_eq!(m.num_fields, 3, "{d:?}");
    }
}

#[test]
fn custom_candidates_non_ascii_ignored() {
    // only non-ASCII candidates -> effectively empty set
    let err = sniff(
        Sniffer::new().candidates(&[0xFE, 0xFF]),
        &delimited(b',', 5, 3),
    )
    .unwrap_err();
    assert!(matches!(err, SnifferError::SniffingFailed(_)), "{err}");
}

#[test]
fn empty_candidates_allowed_when_delimiter_known() {
    let m = sniff(
        Sniffer::new().candidates(&[]).delimiter(b';'),
        &delimited(b';', 10, 3),
    )
    .unwrap();
    assert_eq!(m.dialect.delimiter, b';');
    assert_eq!(m.num_fields, 3);
}

// ---------------------------------------------------------------------------------------------
// 3. require_steady rejects non-tabular data
// ---------------------------------------------------------------------------------------------

fn assert_rejected(result: csv_scout::error::Result<Metadata>) {
    match result {
        Err(SnifferError::SniffingFailed(_)) => {}
        other => panic!("expected SniffingFailed, got {other:?}"),
    }
}

#[test]
fn require_steady_rejects_prose() {
    assert_rejected(sniff(Sniffer::new().require_steady(true), PROSE.as_bytes()));
    assert_rejected(sniff(
        Sniffer::new()
            .candidates(&[b'\t', b',', b';', b'|', b':', 0x01, 0x1F])
            .require_steady(true),
        PROSE.as_bytes(),
    ));
}

#[test]
fn require_steady_rejects_logs() {
    assert_rejected(sniff(Sniffer::new().require_steady(true), LOGS.as_bytes()));
}

#[test]
fn require_steady_rejects_undelimited() {
    assert_rejected(sniff(
        Sniffer::new().require_steady(true),
        b"alpha\nbeta\ngamma\ndelta\n",
    ));
    // single column of numbers
    let col: String = (0..100).map(|i| format!("{i}\n")).collect();
    assert_rejected(sniff(Sniffer::new().require_steady(true), col.as_bytes()));
}

#[test]
fn require_steady_rejects_empty_input() {
    assert_rejected(sniff(Sniffer::new().require_steady(true), b""));
}

#[test]
fn require_steady_rejects_binary() {
    let bin: Vec<u8> = (0..4096u32).map(|i| (i * 7919 % 251) as u8).collect();
    assert_rejected(sniff(
        Sniffer::new()
            .candidates(&[b'|', 0x01, 0x1F])
            .require_steady(true),
        &bin,
    ));
}

#[test]
fn require_steady_rejects_long_unsteady_sample() {
    // Long enough that a plain-probability Viterbi underflows to 0.0 for every chain; the
    // underflow must not be mistaken for a steady state.
    let mut content = String::new();
    for i in 0..3000 {
        let n = i % 4;
        content.push_str(&"x,".repeat(n));
        content.push_str("end\n");
    }
    assert_rejected(sniff(
        Sniffer::new()
            .sample_size(SampleSize::All)
            .require_steady(true),
        content.as_bytes(),
    ));
    // and the default path reports it as unsteady too
    let m = sniff(
        Sniffer::new().sample_size(SampleSize::All),
        content.as_bytes(),
    )
    .unwrap();
    assert_eq!(m.steadiness, Steadiness::Unsteady);
}

#[test]
fn require_steady_rejects_user_delimiter_that_is_absent() {
    assert_rejected(sniff(
        Sniffer::new().delimiter(b'|').require_steady(true),
        &delimited(b',', 10, 3),
    ));
}

#[test]
fn require_steady_accepts_tabular_fixtures() {
    for (file, steadiness) in [
        ("library-visitors.csv", Steadiness::SteadyStrict),
        (
            "2016_presidential_election_durham.csv",
            Steadiness::SteadyStrict,
        ),
        ("boston311.csv", Steadiness::SteadyStrict),
        ("double_quoted.csv", Steadiness::SteadyStrict),
        ("tab_separated.csv", Steadiness::SteadyFlex),
        (
            "gotriangle-routes-cary-ch-duke-durham-raleigh-wofline.csv",
            Steadiness::SteadyFlex,
        ),
    ] {
        let m = Sniffer::new()
            .sample_size(SampleSize::All)
            .require_steady(true)
            .sniff_path(data(file))
            .unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!(m.steadiness, steadiness, "{file}");
        assert!(m.num_fields >= 2, "{file}");
    }
}

#[test]
fn require_steady_accepts_long_steady_sample() {
    let m = sniff(
        Sniffer::new()
            .sample_size(SampleSize::All)
            .require_steady(true),
        &delimited(b'\t', 5000, 4),
    )
    .unwrap();
    assert_eq!(m.dialect.delimiter, b'\t');
    assert_eq!(m.steadiness, Steadiness::SteadyStrict);
    assert_eq!(m.num_fields, 4);
}

#[test]
fn require_steady_can_be_reset() {
    let mut sniffer = Sniffer::new();
    sniffer.require_steady(true);
    assert_rejected(sniff(&mut sniffer, PROSE.as_bytes()));
    sniffer.require_steady(false);
    assert!(sniff(&mut sniffer, PROSE.as_bytes()).is_ok());
}

// ---------------------------------------------------------------------------------------------
// 4. Metadata accurately reflects steadiness, num_fields, is_utf8
// ---------------------------------------------------------------------------------------------

#[test]
fn metadata_steady_strict() {
    let m = sniff(&mut Sniffer::new(), &delimited(b',', 20, 7)).unwrap();
    assert_eq!(m.steadiness, Steadiness::SteadyStrict);
    assert_eq!(m.num_fields, 7);
    assert!(m.is_utf8);
}

#[test]
fn metadata_steady_flex() {
    // most rows have 5 fields, every 4th row has 3
    let mut content = String::new();
    for i in 0..40 {
        if i % 4 == 3 {
            content.push_str("a;b;c\n");
        } else {
            content.push_str("a;b;c;d;e\n");
        }
    }
    let m = sniff(&mut Sniffer::new(), content.as_bytes()).unwrap();
    assert_eq!(m.dialect.delimiter, b';');
    assert_eq!(m.steadiness, Steadiness::SteadyFlex);
    assert_eq!(m.num_fields, 5);
}

#[test]
fn metadata_non_utf8() {
    let mut content = delimited(b'|', 10, 3);
    content.extend_from_slice(b"caf\xE9|na\xEFve|x\n"); // latin-1
    let m = sniff(&mut Sniffer::new(), &content).unwrap();
    assert_eq!(m.dialect.delimiter, b'|');
    assert!(!m.is_utf8);
    assert_eq!(m.num_fields, 3);

    // the same with an explicitly-supplied delimiter and quote (no inference passes run)
    let m = sniff(Sniffer::new().delimiter(b'|').quote(Quote::None), &content).unwrap();
    assert!(!m.is_utf8);
    assert_eq!(m.num_fields, 3);
    assert_eq!(m.steadiness, Steadiness::SteadyStrict);
}

#[test]
fn metadata_utf8_flag_resets_between_sniffs() {
    let mut sniffer = Sniffer::new();
    let m = sniff(&mut sniffer, b"a,b\n\xFF,c\n").unwrap();
    assert!(!m.is_utf8);
    let mut sniffer = Sniffer::new();
    let m = sniff(&mut sniffer, b"a,b\nd,c\n").unwrap();
    assert!(m.is_utf8);
}

#[test]
fn metadata_quoted_fields_do_not_inflate_num_fields() {
    // delimiters inside quoted fields must not be counted
    let mut content = String::from("id,name,notes\n");
    for i in 0..10 {
        content.push_str(&format!("{i},\"Doe, Jane\",\"a, b, c\"\n"));
    }
    let m = sniff(&mut Sniffer::new(), content.as_bytes()).unwrap();
    assert_eq!(m.dialect.quote, Quote::Some(b'"'));
    assert_eq!(m.num_fields, 3);
    assert_eq!(m.steadiness, Steadiness::SteadyStrict);
}

#[test]
fn metadata_user_supplied_delimiter() {
    let m = sniff(Sniffer::new().delimiter(b'\t'), &delimited(b'\t', 15, 9)).unwrap();
    assert_eq!(m.num_fields, 9);
    assert_eq!(m.steadiness, Steadiness::SteadyStrict);
}

#[test]
fn metadata_respects_records_sample_size() {
    // first 10 records have 3 fields, the rest are garbage; sampling 10 records sees only the
    // steady part
    let mut content = delimited(b',', 10, 3);
    for i in 0..50 {
        content.extend_from_slice(&b",".repeat(i % 7));
        content.push(b'\n');
    }
    let m = sniff(
        Sniffer::new()
            .delimiter(b',')
            .quote(Quote::None)
            .sample_size(SampleSize::Records(10)),
        &content,
    )
    .unwrap();
    assert_eq!(m.num_fields, 3);
    assert_eq!(m.steadiness, Steadiness::SteadyStrict);
}

#[test]
fn metadata_display_includes_new_fields() {
    let m = sniff(&mut Sniffer::new(), &delimited(b',', 5, 2)).unwrap();
    let shown = m.to_string();
    assert!(shown.contains("Steadiness: SteadyStrict"), "{shown}");
    assert!(shown.contains("Number of fields: 2"), "{shown}");
    assert!(shown.contains("Is utf-8 encoded?: true"), "{shown}");
}
