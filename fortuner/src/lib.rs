use clap::{App, Arg};
use rand::{rngs::StdRng, seq::SliceRandom, thread_rng, SeedableRng};
use regex::{Regex, RegexBuilder};
use std::{
    error::Error,
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};
use walkdir::WalkDir;

type MyResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
pub struct Config {
    sources: Vec<String>,
    pattern: Option<Regex>,
    seed: Option<u64>,
}

#[derive(Debug)]
pub struct Fortune {
    source: String,
    text: String,
}

pub fn get_args() -> MyResult<Config> {
    let matches = App::new("fortuner")
        .version("0.1.0")
        .author("Shaan Batra")
        .about("Rust fortune")
        .arg(
            Arg::with_name("sources")
                .value_name("FILE")
                .help("Input files or directories")
                .multiple(true)
                .required(true),
        )
        .arg(
            Arg::with_name("pattern")
                .value_name("PATTERN")
                .help("Pattern")
                .short("m")
                .long("pattern"),
        )
        .arg(
            Arg::with_name("seed")
                .value_name("SEED")
                .help("Random seed")
                .short("s")
                .long("seed"),
        )
        .arg(
            Arg::with_name("insensitive")
                .help("Case-insensitive pattern matching")
                .short("i")
                .long("insensitive")
                .takes_value(false),
        )
        .get_matches();

    let sources = matches.values_of_lossy("sources").unwrap();
    let pattern = matches
        .value_of("pattern")
        .map(|pattern| {
            RegexBuilder::new(pattern)
                .case_insensitive(matches.is_present("insensitive"))
                .build()
                .map_err(|_| format!("Invalid --pattern \"{}\"", pattern))
        })
        .transpose()?;
    let seed = matches
        .value_of("seed")
        .map(|num| {
            num.parse()
                .map_err(|_| format!("\"{}\" not a valid integer", num))
        })
        .transpose()?;

    Ok(Config {
        sources,
        pattern,
        seed,
    })
}

pub fn run(config: Config) -> MyResult<()> {
    let files = find_files(&config.sources)?;
    let fortunes = read_fortunes(&files)?;
    let mut prev_source = None;
    if let Some(pattern) = config.pattern {
        for fortune in fortunes.iter().filter(|f| pattern.is_match(&f.text)) {
            if prev_source.as_ref().map_or(true, |s| s != &fortune.source) {
                eprintln!("({})\n%", fortune.source);
                prev_source = Some(fortune.source.clone());
            }
            println!("{}\n%", fortune.text);
        }
    } else {
        println!(
            "{}",
            pick_fortune(&fortunes, config.seed)
                .or_else(|| Some("No fortunes found".to_string()))
                .unwrap()
        );
    }

    Ok(())
}

fn find_files(paths: &[String]) -> MyResult<Vec<PathBuf>> {
    let mut pathbufs = Vec::new();

    for path in paths {
        for entry in WalkDir::new(path) {
            let entry = entry?;
            if entry.file_type().is_file() {
                pathbufs.push(entry.into_path());
            }
        }
    }

    pathbufs.sort();
    pathbufs.dedup();

    Ok(pathbufs)
}

fn read_fortunes(paths: &[PathBuf]) -> MyResult<Vec<Fortune>> {
    let mut fortunes = vec![];
    let mut buf = vec![];

    for path in paths {
        let basename = path.file_name().unwrap().to_string_lossy().into_owned();
        let file = File::open(path)
            .map_err(|e| format!("{}: {}", path.to_string_lossy().into_owned(), e))?;

        for line in BufReader::new(file).lines().filter_map(Result::ok) {
            if line == "%" {
                if !buf.is_empty() {
                    // skips empty lines
                    fortunes.push(Fortune {
                        source: basename.clone(),
                        text: buf.join("\n"),
                    });
                }
                buf.clear();
            } else {
                buf.push(line.to_string());
            }
        }
    }

    Ok(fortunes)
}

fn pick_fortune(fortunes: &[Fortune], seed: Option<u64>) -> Option<String> {
    let mut rng = match seed {
        None => StdRng::from_rng(thread_rng()).unwrap(),
        Some(num) => StdRng::seed_from_u64(num),
    };
    fortunes.choose(&mut rng).map(|f| f.text.clone())
}

#[cfg(test)]
mod unit_tests {
    use super::{find_files, pick_fortune, read_fortunes, Fortune};
    use std::path::PathBuf;

    #[test]
    fn test_find_files() {
        // Verify that the function finds a file known to exist
        let res = find_files(&["./tests/inputs/jokes".to_string()]);
        assert!(res.is_ok());

        let files = res.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            files.get(0).unwrap().to_string_lossy(),
            "./tests/inputs/jokes"
        );

        // Fails to find bad file
        let res = find_files(&["/path/does/not/exist".to_string()]);
        assert!(res.is_err());

        // Finds all the input files, excludes ".dat"
        let res = find_files(&["./tests/inputs".to_string()]);
        assert!(res.is_ok());

        // Check number and order of files
        let files = res.unwrap();
        println!("{:?}", files);
        assert_eq!(files.len(), 5);
        let first = files.get(0).unwrap().display().to_string();
        assert!(first.contains("ascii-art"));
        let last = files.last().unwrap().display().to_string();
        assert!(last.contains("quotes"));

        // Test for multiple sources, path must be unique and sorted
        let res = find_files(&[
            "./tests/inputs/jokes".to_string(),
            "./tests/inputs/ascii-art".to_string(),
            "./tests/inputs/jokes".to_string(),
        ]);
        assert!(res.is_ok());
        let files = res.unwrap();
        assert_eq!(files.len(), 2);
        if let Some(filename) = files.first().unwrap().file_name() {
            assert_eq!(filename.to_string_lossy(), "ascii-art".to_string())
        }
        if let Some(filename) = files.last().unwrap().file_name() {
            assert_eq!(filename.to_string_lossy(), "jokes".to_string())
        }
    }

    #[test]
    fn test_read_fortunes() {
        // One input file
        let res = read_fortunes(&[PathBuf::from("./tests/inputs/jokes")]);
        assert!(res.is_ok());
        if let Ok(fortunes) = res {
            // Correct number and sorting
            assert_eq!(fortunes.len(), 6);
            assert_eq!(
                fortunes.first().unwrap().text,
                "Q. What do you call a head of lettuce in a shirt and tie?\n\
                A. Collared greens."
            );
            assert_eq!(
                fortunes.last().unwrap().text,
                "Q: What do you call a deer wearing an eye patch?\n\
            	A: A bad idea (bad-eye deer)."
            );
        }

        // Multiple input files
        let res = read_fortunes(&[
            PathBuf::from("./tests/inputs/jokes"),
            PathBuf::from("./tests/inputs/quotes"),
        ]);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().len(), 11);
    }

    #[test]
    fn test_pick_fortune() {
        // Create a slice of fortunes
        let fortunes = &[
            Fortune {
                source: "fortunes".to_string(),
                text: "You cannot achieve the impossible without \
    	attempting the absurd."
                    .to_string(),
            },
            Fortune {
                source: "fortunes".to_string(),
                text: "Assumption is the mother of all screw-ups.".to_string(),
            },
            Fortune {
                source: "fortunes".to_string(),
                text: "Neckties strangle clear thinking.".to_string(),
            },
        ];

        // Pick a fortune with a seed
        assert_eq!(
            pick_fortune(fortunes, Some(1)).unwrap(),
            "Neckties strangle clear thinking.".to_string()
        );
    }
}
