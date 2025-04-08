use hashbrown::HashMap;
use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use csv::Reader;
use csv_core as csvc;
use regex::Regex;

use crate::{
    chain::{Chain, STATE_STEADYFLEX, STATE_STEADYSTRICT, STATE_UNSTEADY, ViterbiResults},
    error::{Result, SnifferError},
    // field_type::DatePreference,
    metadata::{Dialect, Metadata, Quote},
    sample::{SampleIter, SampleSize, take_sample_from_start},
};

const NUM_ASCII_CHARS: usize = 128;

thread_local! (pub static IS_UTF8: RefCell<bool> = const { RefCell::new(true) });
// thread_local! (pub static DATE_PREFERENCE: RefCell<DatePreference> = const { RefCell::new(DatePreference::MdyFormat) });

/// A CSV sniffer.
///
/// The sniffer examines a CSV file, passed in either through a file or a reader.
#[derive(Debug, Default)]
pub struct Sniffer {
    // CSV file dialect guesses
    delimiter: Option<u8>,
    // num_preamble_rows: Option<usize>,
    // has_header_row: Option<bool>,
    quote: Option<Quote>,
    flexible: Option<bool>,
    is_utf8: Option<bool>,

    // Metadata guesses
    delimiter_freq: Option<usize>,
    // fields: Vec<String>,
    // types: Vec<Type>,
    // avg_record_len: Option<usize>,

    // sample size to sniff
    sample_size: Option<SampleSize>,
    // date format preference
    // date_preference: Option<DatePreference>,
}
impl Sniffer {
    /// Create a new CSV sniffer.
    pub fn new() -> Self {
        Self::default()
    }
    /// Specify the delimiter character.
    pub fn delimiter(&mut self, delimiter: u8) -> &mut Self {
        self.delimiter = Some(delimiter);
        self
    }
    /// Specify the header type (whether the CSV file has a header row, and where the data starts).
    // pub fn header(&mut self, header: &Header) -> &mut Self {
    //     self.num_preamble_rows = Some(header.num_preamble_rows);
    //     self.has_header_row = Some(header.has_header_row);
    //     self
    // }
    /// Specify the quote character (if any), and whether two quotes in a row as to be interpreted
    /// as an escaped quote.
    pub fn quote(&mut self, quote: Quote) -> &mut Self {
        self.quote = Some(quote);
        self
    }

    /// The size of the sample to examine while sniffing. If using `SampleSize::Records`, the
    /// sniffer will use the `Terminator::CRLF` as record separator.
    ///
    /// The sample size defaults to `SampleSize::Bytes(4096)`.
    pub fn sample_size(&mut self, sample_size: SampleSize) -> &mut Self {
        self.sample_size = Some(sample_size);
        self
    }

    fn get_sample_size(&self) -> SampleSize {
        self.sample_size.unwrap_or(SampleSize::Bytes(1 << 14))
    }

    // The date format preference when sniffing.
    //
    // The date format preference defaults to `DatePreference::MDY`.
    // pub fn date_preference(&mut self, date_preference: DatePreference) -> &mut Self {
    //     DATE_PREFERENCE.with(|preference| {
    //         *preference.borrow_mut() = date_preference;
    //     });
    //     self.date_preference = Some(date_preference);
    //     self
    // }

    /// Sniff the CSV file located at the provided path, and return a `Reader` (from the
    /// [`csv`](https://docs.rs/csv) crate) ready to ready the file.
    ///
    /// Fails on file opening or rendering errors, or on an error examining the file.
    pub fn open_path<P: AsRef<Path>>(&mut self, path: P) -> Result<Reader<File>> {
        self.open_reader(File::open(path)?)
    }
    /// Sniff the CSV file provided by the reader, and return a [`csv`](https://docs.rs/csv)
    /// `Reader` object.
    ///
    /// Fails on file opening or rendering errors, or on an error examining the file.
    pub fn open_reader<R: Read + Seek>(&mut self, mut reader: R) -> Result<Reader<R>> {
        let metadata = self.sniff_reader(&mut reader)?;
        reader.seek(SeekFrom::Start(0))?;
        metadata.dialect.open_reader(reader)
    }

    /// Sniff the CSV file located at the provided path, and return a
    /// [`Metadata`](struct.Metadata.html) object containing information about the CSV file.
    ///
    /// Fails on file opening or rendering errors, or on an error examining the file.
    pub fn sniff_path<P: AsRef<Path>>(&mut self, path: P) -> Result<Metadata> {
        let file = File::open(path)?;
        self.sniff_reader(&file)
    }
    /// Sniff the CSV file provider by the reader, and return a
    /// [`Metadata`](struct.Metadata.html) object containing information about the CSV file.
    ///
    /// Fails on file opening or readering errors, or on an error examining the file.
    pub fn sniff_reader<R: Read + Seek>(&mut self, mut reader: R) -> Result<Metadata> {
        // init IS_UTF8 global var to true
        IS_UTF8.with(|flag| {
            *flag.borrow_mut() = true;
        });
        // guess quotes & delim
        self.infer_quotes_delim(&mut reader)?;

        // if we have a delimiter, we just need to search for num_preamble_rows and check for
        // flexible. Otherwise, we need to guess a delimiter as well.
        if self.delimiter.is_some() {
            self.infer_preamble_known_delim(&mut reader)?;
        } else {
            self.infer_delim_preamble(&mut reader)?;
        }

        // self.infer_types(&mut reader)?;
        self.is_utf8 = Some(IS_UTF8.with(|flag| *flag.borrow()));

        // as this point of the process, we should have all these filled in.
        // assert!(
        //     self.delimiter.is_some()
        //         && self.num_preamble_rows.is_some()
        //         && self.quote.is_some()
        //         && self.flexible.is_some()
        //         && self.is_utf8.is_some()
        //         && self.delimiter_freq.is_some()
        //         && self.has_header_row.is_some()
        //         && self.avg_record_len.is_some()
        //         && self.delimiter_freq.is_some()
        // );
        if !(self.delimiter.is_some()
            // && self.num_preamble_rows.is_some()
            && self.quote.is_some()
            && self.flexible.is_some()
            && self.is_utf8.is_some()
            && self.delimiter_freq.is_some()
            // && self.has_header_row.is_some()
            // && self.avg_record_len.is_some()
            && self.delimiter_freq.is_some())
        {
            return Err(SnifferError::SniffingFailed(format!(
                "Failed to infer all metadata: {self:?}"
            )));
        }
        // safety: we just checked that all these are Some, so it's safe to unwrap
        Ok(Metadata {
            dialect: Dialect {
                delimiter: self.delimiter.unwrap(),
                // header: Header {
                //     num_preamble_rows: self.num_preamble_rows.unwrap(),
                //     has_header_row: self.has_header_row.unwrap(),
                // },
                quote: self.quote.clone().unwrap(),
                // flexible: self.flexible.unwrap(),
                // is_utf8: self.is_utf8.unwrap(),
            },
            // avg_record_len: self.avg_record_len.unwrap(),
            // num_fields: self.delimiter_freq.unwrap() + 1,
            // fields: self.fields.clone(),
            // types: self.types.clone(),
        })
    }

    // Infers quotes and delimiter from quoted (or possibly quoted) files. If quotes detected,
    // updates self.quote and self.delimiter. If quotes not detected, updates self.quote to
    // Quote::None. Only valid quote characters: " (double-quote), ' (single-quote), ` (back-tick).
    fn infer_quotes_delim<R: Read + Seek>(&mut self, reader: &mut R) -> Result<()> {
        if let (&Some(_), &Some(_)) = (&self.quote, &self.delimiter) {
            // nothing left to infer!
            return Ok(());
        }
        let quote_guesses = match self.quote {
            Some(Quote::Some(chr)) => vec![chr],
            Some(Quote::None) => {
                // this function only checks quoted (or possibly quoted) files, nothing left to
                // do if we know there are no quotes
                return Ok(());
            }
            None => vec![b'\'', b'"', b'`'],
        };
        let (quote_chr, (quote_cnt, delim_guess)) = quote_guesses.iter().try_fold(
            (b'"', (0, b'\0')),
            |acc, &chr| -> Result<(u8, (usize, u8))> {
                let mut sample_reader = take_sample_from_start(reader, self.get_sample_size())?;
                if let Some((cnt, delim_chr)) =
                    quote_count(&mut sample_reader, char::from(chr), self.delimiter)?
                {
                    Ok(if cnt > acc.1.0 {
                        (chr, (cnt, delim_chr))
                    } else {
                        acc
                    })
                } else {
                    Ok(acc)
                }
            },
        )?;
        if quote_cnt == 0 {
            self.quote = Some(Quote::None);
        } else {
            self.quote = Some(Quote::Some(quote_chr));
            self.delimiter = Some(delim_guess);
        };
        Ok(())
    }

    // Updates delimiter frequency, number of preamble rows, and flexible boolean.
    fn infer_preamble_known_delim<R: Read + Seek>(&mut self, reader: &mut R) -> Result<()> {
        // prerequisites for calling this function:
        if !(self.delimiter.is_some() && self.quote.is_some()) {
            // instead of assert, return an error
            // assert!(self.delimiter.is_some() && self.quote.is_some());
            return Err(SnifferError::SniffingFailed(
                "infer_preamble_known_delim called without delimiter and quote".into(),
            ));
        }
        // safety: unwraps for delimiter and quote are safe since we just checked above
        let (quote, delim) = (self.quote.clone().unwrap(), self.delimiter.unwrap());

        let sample_iter = take_sample_from_start(reader, self.get_sample_size())?;

        let mut chain = Chain::default();

        if let Quote::Some(character) = quote {
            // since we have a quote, we need to run this data through the csv_core::Reader (which
            // properly escapes quoted fields
            let mut csv_reader = csvc::ReaderBuilder::new()
                .delimiter(delim)
                .quote(character)
                .build();

            let mut output = vec![];
            let mut ends = vec![];
            for line in sample_iter {
                let line = line?;
                if line.len() > output.len() {
                    output.resize(line.len(), 0);
                }
                if line.len() > ends.len() {
                    ends.resize(line.len(), 0);
                }
                let (result, _, _, n_ends) =
                    csv_reader.read_record(line.as_bytes(), &mut output, &mut ends);
                // check to make sure record was read correctly
                match result {
                    csvc::ReadRecordResult::OutputFull | csvc::ReadRecordResult::OutputEndsFull => {
                        return Err(SnifferError::SniffingFailed(format!(
                            "failure to read quoted CSV record: {result:?}"
                        )));
                    }
                    _ => {} // non-error results, do nothing
                }
                // n_ends is the number of barries between fields, so it's the same as the number
                // of delimiters
                chain.add_observation(n_ends);
            }
        } else {
            for line in sample_iter {
                let line = line?;
                let freq = bytecount::count(line.as_bytes(), delim);
                chain.add_observation(freq);
            }
        }
        self.run_chains(vec![chain])
    }

    // Updates delimiter, delimiter frequency, number of preamble rows, and flexible boolean.
    fn infer_delim_preamble<R: Read + Seek>(&mut self, reader: &mut R) -> Result<()> {
        let sample_iter = take_sample_from_start(reader, self.get_sample_size())?;

        let mut chains = vec![Chain::default(); NUM_ASCII_CHARS];
        for line in sample_iter {
            let line = line?;
            let mut freqs = [0; NUM_ASCII_CHARS];
            for &chr in line.as_bytes() {
                if chr < NUM_ASCII_CHARS as u8 {
                    freqs[chr as usize] += 1;
                }
            }
            for (chr, &freq) in freqs.iter().enumerate() {
                chains[chr].add_observation(freq);
            }
        }

        self.run_chains(chains)
    }

    // Updates delimiter (if not already known), delimiter frequency, number of preamble rows, and
    // flexible boolean.
    fn run_chains(&mut self, mut chains: Vec<Chain>) -> Result<()> {
        // Find the 'best' delimiter: choose strict (non-flexible) delimiters over flexible ones,
        // and choose the one that had the highest probability markov chain in the end.
        //
        // In the case where delim is already known, 'best_delim' will be incorrect (since it won't
        // correspond with position in a vector of Chains), but we'll just ignore it when
        // constructing our return value later. 'best_state' and 'path' are necessary, though, to
        // compute the preamble rows.
        let (best_delim, delim_freq, best_state, _, _) = chains.iter_mut().enumerate().fold(
            (b',', 0, STATE_UNSTEADY, vec![], 0.0),
            |acc, (i, ref mut chain)| {
                let (_, _, best_state, _, best_state_prob) = acc;
                let ViterbiResults {
                    max_delim_freq,
                    path,
                } = chain.viterbi();
                if path.is_empty() {
                    return acc;
                }
                let (final_state, final_viter) = path[path.len() - 1];
                if final_state < best_state
                    || (final_state == best_state && final_viter.prob > best_state_prob)
                {
                    (i as u8, max_delim_freq, final_state, path, final_viter.prob)
                } else {
                    acc
                }
            },
        );
        self.flexible = Some(match best_state {
            STATE_STEADYSTRICT => false,
            STATE_STEADYFLEX => true,
            _ => {
                return Err(SnifferError::SniffingFailed(
                    "unable to find valid delimiter".to_string(),
                ));
            }
        });

        // Find the number of preamble rows (the number of rows during which the state fluctuated
        // before getting to the final state).
        // let mut num_preamble_rows = 0;
        // since path has an extra state as the beginning, skip one
        // for &(state, _) in path.iter().skip(2) {
        //     if state == best_state {
        //         break;
        //     }
        //     num_preamble_rows += 1;
        // }
        // if num_preamble_rows > 0 {
        //     num_preamble_rows += 1;
        // }
        if self.delimiter.is_none() {
            self.delimiter = Some(best_delim);
        }
        self.delimiter_freq = Some(delim_freq);
        // self.num_preamble_rows = Some(num_preamble_rows);
        Ok(())
    }

    // fn infer_types<R: Read + Seek>(&mut self, reader: &mut R) -> Result<()> {
    //     // prerequisites for calling this function:
    //     if self.delimiter_freq.is_none() {
    //         // instead of assert, return error
    //         // assert!(self.delimiter_freq.is_some());
    //         return Err(SnifferError::SniffingFailed(
    //             "delimiter frequency not known".to_string(),
    //         ));
    //     }
    //     // safety: unwrap is safe as we just checked that delimiter_freq is Some
    //     let field_count = self.delimiter_freq.unwrap() + 1;

    //     let mut csv_reader = self.create_csv_reader(reader)?;
    //     let mut records_iter = csv_reader.byte_records();
    //     let mut n_bytes = 0;
    //     let mut n_records = 0;
    //     let sample_size = self.get_sample_size();

    //     // Infer types for the top row. We'll save this set of types to check against the types
    //     // of the remaining rows to see if this is part of the data or a separate header row.
    //     let header_row_types = match records_iter.next() {
    //         Some(record) => {
    //             let byte_record = record?;
    //             let str_record = StringRecord::from_byte_record_lossy(byte_record);
    //             n_records += 1;
    //             n_bytes += count_bytes(&str_record);
    //             infer_record_types(&str_record)
    //         }
    //         None => {
    //             return Err(SnifferError::SniffingFailed(
    //                 "CSV empty (after preamble)".into(),
    //             ));
    //         }
    //     };
    //     let mut row_types = vec![TypeGuesses::all(); field_count];

    //     for record in records_iter {
    //         let record = record?;
    //         for (i, field) in record.iter().enumerate() {
    //             let str_field = String::from_utf8_lossy(field).to_string();
    //             row_types[i] &= infer_types(&str_field);
    //         }
    //         n_records += 1;
    //         n_bytes += record.as_slice().len();
    //         // break if we pass sample size limits
    //         match sample_size {
    //             SampleSize::Records(recs) => {
    //                 if n_records > recs {
    //                     break;
    //                 }
    //             }
    //             SampleSize::Bytes(bytes) => {
    //                 if n_bytes > bytes {
    //                     break;
    //                 }
    //             }
    //             SampleSize::All => {}
    //         }
    //     }
    //     if n_records == 1 {
    //         // there's only one row in the whole data file (the top row already parsed),
    //         // so we're going to assume it's a data row, not a header row.
    //         self.has_header_row = Some(false);
    //         self.types = get_best_types(&header_row_types);
    //         self.avg_record_len = Some(n_bytes);
    //         return Ok(());
    //     }

    //     if header_row_types
    //         .iter()
    //         .zip(&row_types)
    //         .any(|(header, data)| !data.allows(*header))
    //     {
    //         self.has_header_row = Some(true);
    //         // get field names in header
    //         for field in csv_reader.byte_headers()? {
    //             self.fields.push(String::from_utf8_lossy(field).to_string());
    //         }
    //     } else {
    //         self.has_header_row = Some(false);
    //     }

    //     self.types = get_best_types(&row_types);
    //     self.avg_record_len = Some(n_bytes / n_records);
    //     Ok(())
    // }

    // fn create_csv_reader<'a, R: Read + Seek>(
    //     &self,
    //     mut reader: &'a mut R,
    // ) -> Result<Reader<&'a mut R>> {
    //     reader.seek(SeekFrom::Start(0))?;
    //     if let Some(num_preamble_rows) = self.num_preamble_rows {
    //         snip_preamble(&mut reader, num_preamble_rows)?;
    //     }

    //     let mut builder = csv::ReaderBuilder::new();
    //     if let Some(delim) = self.delimiter {
    //         builder.delimiter(delim);
    //     }
    //     if let Some(has_header_row) = self.has_header_row {
    //         builder.has_headers(has_header_row);
    //     }
    //     match self.quote {
    //         Some(Quote::Some(chr)) => {
    //             builder.quoting(true);
    //             builder.quote(chr);
    //         }
    //         Some(Quote::None) => {
    //             builder.quoting(false);
    //         }
    //         _ => {}
    //     }
    //     if let Some(flexible) = self.flexible {
    //         builder.flexible(flexible);
    //     }

    //     Ok(builder.from_reader(reader))
    // }
}

fn quote_count<R: Read>(
    sample_iter: &mut SampleIter<R>,
    character: char,
    delim: Option<u8>,
) -> Result<Option<(usize, u8)>> {
    // Build a regex that matches a quoted CSV cell,
    // optionally followed by a delimiter.
    // If delim is None, we try to capture a candidate delimiter.
    let pattern = delim.map_or_else(
        || {
            // When delim is not provided, capture candidate delimiters in a group.
            format!(
                r"{character}(?P<field>(?:[^{character}]|{character}{character})*){character}(?:\s*(?P<delim>[^\w\n{character}])\s*)?"
            )
        },
        |delim| {
            // When delim is provided, enforce its presence if it appears.
            format!(
                r"{q}(?P<field>(?:[^{q}]|{q}{q})*){q}(?:\s*{d}\s*)?",
                q = character,
                d = delim as char
            )
        },
    );
    // Safety: unwrap is safe here because we control the regex pattern.
    let re = Regex::new(&pattern).unwrap();

    let mut delim_count_map: HashMap<String, usize> = HashMap::new();
    let mut count = 0;
    for line in sample_iter {
        let line = line?;
        // Iterate through all quoted cell matches in the line.
        for cap in re.captures_iter(&line) {
            count += 1;
            // If delim was not provided, count candidate delimiter occurrences.
            if delim.is_none() {
                if let Some(d) = cap.name("delim") {
                    *delim_count_map.entry(d.as_str().to_string()).or_insert(0) += 1;
                }
            }
        }
    }
    if count == 0 {
        return Ok(None);
    }

    // If a delimiter was provided, just return it.
    if let Some(delim) = delim {
        return Ok(Some((count, delim)));
    }

    // Otherwise, select the candidate delimiter that was matched most frequently.
    let (delim_count, delim) = delim_count_map
        .iter()
        .fold((0, b'\0'), |acc, (delim, &d_count)| {
            if delim.len() != 1 {
                (0, b'\0')
            } else if d_count > acc.0 {
                (d_count, delim.as_bytes()[0])
            } else {
                acc
            }
        });

    if delim_count == 0 {
        return Err(SnifferError::SniffingFailed(
            "invalid regex match: no delimiter found".into(),
        ));
    }
    Ok(Some((count, delim)))
}
