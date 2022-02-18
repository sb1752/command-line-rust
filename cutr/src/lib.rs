use crate::Extract::*;
use clap::{App, Arg};
use csv::{ReaderBuilder, StringRecord, WriterBuilder};
use regex::Regex;
use std::{
    error::Error,
    fs::File,
    io::{self, BufRead, BufReader},
    num::NonZeroUsize,
    ops::Range,
};

type MyResult<T> = Result<T, Box<dyn Error>>;
type PositionList = Vec<Range<usize>>;

#[derive(Debug)]
pub enum Extract {
    Fields(PositionList),
    Bytes(PositionList),
    Chars(PositionList),
}

#[derive(Debug)]
pub struct Config {
    files: Vec<String>,
    delimiter: u8,
    extract: Extract,
}

pub fn get_args() -> MyResult<Config> {
    let matches = App::new("cutr")
        .version("0.1.0")
        .author("Shaan Batra")
        .about("Rust cut")
        .arg(
            Arg::with_name("files")
                .value_name("FILE")
                .help("Input file(s)")
                .multiple(true)
                .default_value("-"),
        )
        .arg(
            Arg::with_name("delimiter")
                .value_name("DELIMITER")
                .help("Field delimiter")
                .short("d")
                .long("delim")
                .takes_value(true)
                .default_value("	"),
        )
        .arg(
            Arg::with_name("bytes")
                .value_name("BYTES")
                .help("Selected bytes")
                .short("b")
                .long("bytes")
                .takes_value(true)
                .conflicts_with_all(&["chars", "fields"]),
        )
        .arg(
            Arg::with_name("chars")
                .value_name("CHARS")
                .help("Selected characters")
                .short("c")
                .long("chars")
                .takes_value(true)
                .conflicts_with_all(&["bytes", "fields"]),
        )
        .arg(
            Arg::with_name("fields")
                .value_name("FIELDS")
                .help("Selected fields")
                .short("f")
                .long("fields")
                .takes_value(true)
                .conflicts_with_all(&["bytes", "chars"]),
        )
        .get_matches();

    let delimiter = matches.value_of("delimiter").unwrap();
    let delim_bytes = delimiter.as_bytes();
    if delim_bytes.len() != 1 {
        return Err(From::from(format!(
            "--delim \"{}\" must be a single byte",
            delimiter
        )));
    }

    let fields = matches.value_of("fields").map(parse_pos).transpose()?;
    let bytes = matches.value_of("bytes").map(parse_pos).transpose()?;
    let chars = matches.value_of("chars").map(parse_pos).transpose()?;

    let extract = if let Some(field_pos) = fields {
        Fields(field_pos)
    } else if let Some(byte_pos) = bytes {
        Bytes(byte_pos)
    } else if let Some(char_pos) = chars {
        Chars(char_pos)
    } else {
        return Err(From::from("Must have --fields, --bytes, or --chars"));
    };

    Ok(Config {
        files: matches.values_of_lossy("files").unwrap(),
        delimiter: *delim_bytes.first().unwrap(),
        extract,
    })
}

pub fn run(config: Config) -> MyResult<()> {
    for filename in &config.files {
        match open(filename) {
            Err(err) => eprintln!("{}: {}", filename, err),
            Ok(file) => match &config.extract {
                Fields(field_pos) => {
                    let mut rdr = ReaderBuilder::new()
                        .has_headers(false)
                        .delimiter(config.delimiter)
                        .from_reader(file);

                    let mut wtr = WriterBuilder::new()
                        .delimiter(config.delimiter)
                        .from_writer(io::stdout());

                    for record in rdr.records() {
                        let record = record?;
                        wtr.write_record(extract_fields(&record, field_pos))?;
                    }
                }

                Bytes(byte_pos) => {
                    for line in file.lines() {
                        println!("{}", extract_bytes(&line?, byte_pos));
                    }
                }

                Chars(char_pos) => {
                    for line in file.lines() {
                        println!("{}", extract_chars(&line?, char_pos));
                    }
                }
            },
        }
    }
    Ok(())
}

fn open(filename: &str) -> MyResult<Box<dyn BufRead>> {
    match filename {
        "-" => Ok(Box::new(BufReader::new(io::stdin()))),
        _ => Ok(Box::new(BufReader::new(File::open(filename)?))),
    }
}

fn parse_index(input: &str) -> Result<usize, String> {
    let value_error = || format!("illegal list value: \"{}\"", input);

    input
        .starts_with("+")
        .then(|| Err(value_error()))
        .unwrap_or_else(|| {
            input
                .parse::<NonZeroUsize>()
                .map(|n| usize::from(n) - 1)
                .map_err(|_| value_error())
        })
}

fn parse_pos(range: &str) -> MyResult<PositionList> {
    let range_re = Regex::new(r"^(\d+)-(\d+)$").unwrap();

    range
        .split(",")
        .into_iter()
        .map(|val| {
            parse_index(val).map(|n| n..n + 1).or_else(|e| {
                range_re.captures(val).ok_or(e).and_then(|captures| {
                    let n1 = parse_index(&captures[1])?;
                    let n2 = parse_index(&captures[2])?;
                    if n1 >= n2 {
                        return Err(format!(
                            "First number in range ({}) must be lower than second number ({})",
                            n1 + 1,
                            n2 + 1
                        ));
                    }
                    Ok(n1..n2 + 1)
                })
            })
        })
        .collect::<Result<_, _>>()
        .map_err(From::from)
}

fn extract_chars(line: &str, char_pos: &[Range<usize>]) -> String {
    let chars: Vec<_> = line.chars().collect();

    // filter_map returns only values for which closure returns Some(value)
    char_pos
        .iter()
        .cloned()
        .flat_map(|range| range.filter_map(|i| chars.get(i)))
        .collect()
}

fn extract_bytes(line: &str, byte_pos: &[Range<usize>]) -> String {
    let bytes = line.as_bytes();
    let selected: Vec<_> = byte_pos
        .iter()
        .cloned()
        .flat_map(|range| range.filter_map(|i| bytes.get(i)).copied())
        .collect();

    String::from_utf8_lossy(&selected).into_owned()
}

fn extract_fields(record: &StringRecord, field_pos: &[Range<usize>]) -> Vec<String> {
    field_pos
        .iter()
        .cloned()
        .flat_map(|range| range.filter_map(|i| record.get(i)).map(From::from))
        .collect()
}

#[cfg(test)]
mod unit_tests {
    use super::{extract_bytes, extract_chars, extract_fields, parse_pos};
    use csv::StringRecord;

    #[test]
    fn test_parse_pos() {
        // The empty string is an error
        assert!(parse_pos("").is_err());

        // Zero is an error
        let res = parse_pos("0");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "illegal list value: \"0\"");

        let res = parse_pos("0-1");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "illegal list value: \"0\"");

        // A leading "+" is an error
        let res = parse_pos("+1");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "illegal list value: \"+1\"");

        let res = parse_pos("+1-2");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "illegal list value: \"+1-2\"");

        let res = parse_pos("1-+2");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "illegal list value: \"1-+2\"");

        // Any non-number is an error
        let res = parse_pos("a");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "illegal list value: \"a\"");

        let res = parse_pos("1,a");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "illegal list value: \"a\"");

        let res = parse_pos("1-a");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "illegal list value: \"1-a\"");

        let res = parse_pos("a-1");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "illegal list value: \"a-1\"");

        // Wonky ranges
        let res = parse_pos("-");
        assert!(res.is_err());

        let res = parse_pos(",");
        assert!(res.is_err());

        let res = parse_pos("1,");
        assert!(res.is_err());

        let res = parse_pos("1-");
        assert!(res.is_err());

        let res = parse_pos("1-1-1");
        assert!(res.is_err());

        let res = parse_pos("1-1-a");
        assert!(res.is_err());

        // First number must be less than second
        let res = parse_pos("1-1");
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "First number in range (1) must be lower than second number (1)"
        );

        let res = parse_pos("2-1");
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "First number in range (2) must be lower than second number (1)"
        );

        // All the following are acceptable
        let res = parse_pos("1");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec![0..1]);

        let res = parse_pos("01");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec![0..1]);

        let res = parse_pos("1,3");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec![0..1, 2..3]);

        let res = parse_pos("001,003");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec![0..1, 2..3]);

        let res = parse_pos("1-3");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec![0..3]);

        let res = parse_pos("0001-03");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec![0..3]);

        let res = parse_pos("1,7,3-5");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec![0..1, 6..7, 2..5]);

        let res = parse_pos("15,19-20");
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), vec![14..15, 18..20]);
    }

    #[test]
    fn test_extract_chars() {
        assert_eq!(extract_chars("", &[0..1]), "".to_string());
        assert_eq!(extract_chars("ábc", &[0..1]), "á".to_string());
        assert_eq!(extract_chars("ábc", &[0..1, 2..3]), "ác".to_string());
        assert_eq!(extract_chars("ábc", &[0..3]), "ábc".to_string());
        assert_eq!(extract_chars("ábc", &[2..3, 1..2]), "cb".to_string());
        assert_eq!(extract_chars("ábc", &[0..1, 1..2, 4..5]), "áb".to_string());
    }

    #[test]
    fn test_extract_bytes() {
        assert_eq!(extract_bytes("ábc", &[0..1]), "�".to_string());
        assert_eq!(extract_bytes("ábc", &[0..2]), "á".to_string());
        assert_eq!(extract_bytes("ábc", &[0..3]), "áb".to_string());
        assert_eq!(extract_bytes("ábc", &[0..4]), "ábc".to_string());
        assert_eq!(extract_bytes("ábc", &[3..4, 2..3]), "cb".to_string());
        assert_eq!(extract_bytes("ábc", &[0..2, 5..6]), "á".to_string());
    }

    #[test]
    fn test_extract_fields() {
        let rec = StringRecord::from(vec!["Captain", "Sham", "12345"]);
        assert_eq!(extract_fields(&rec, &[0..1]), &["Captain"]);
        assert_eq!(extract_fields(&rec, &[1..2]), &["Sham"]);
        assert_eq!(extract_fields(&rec, &[0..1, 2..3]), &["Captain", "12345"]);
        assert_eq!(extract_fields(&rec, &[0..1, 3..4]), &["Captain"]);
        assert_eq!(extract_fields(&rec, &[1..2, 0..1]), &["Sham", "Captain"]);
    }
}

//---------------------------------------------------------
// Old, unused solution
//---------------------------------------------------------
fn parse_pos_old(input: &str) -> MyResult<PositionList> {
    let mut ranges = Vec::new();
    let plus_char = "+".chars().next().unwrap();

    for range_str in input.split(",") {
        if range_str
            .chars()
            .any(|char| char == plus_char || char.is_alphabetic())
        {
            return Err(format!("illegal list value \"{}\"", range_str).into());
        }

        let range_tuple = match range_str.find("-") {
            None => {
                let int = parse_positive_int(range_str)?;
                (int, int)
            }
            Some(byte) => {
                let str_tuple = range_str.split_at(byte);
                let start = parse_positive_int(str_tuple.0)?;
                let end = parse_positive_int(&str_tuple.1[1..])?;

                if start >= end {
                    return Err(format!(
                        "First number in range ({}) must be lower than second number ({})",
                        start, end
                    )
                    .into());
                }

                (start, end)
            }
        };

        ranges.push(Range {
            start: range_tuple.0 - 1,
            end: range_tuple.1,
        });
    }

    Ok(ranges)
}

//---------------------------------------------------------
// Old, unused solution
//---------------------------------------------------------
fn parse_positive_int(text: &str) -> MyResult<usize> {
    match text.parse() {
        Ok(int) if int > 0 => Ok(int),
        _ => Err(format!("illegal list value \"{}\"", text).into()),
    }
}
