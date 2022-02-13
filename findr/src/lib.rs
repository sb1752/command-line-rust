use crate::EntryType::*;
use clap::{App, Arg};
use regex::Regex;
use std::error::Error;
use walkdir::{DirEntry, WalkDir};

type MyResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, Eq, PartialEq)]
enum EntryType {
    Dir,
    File,
    Link,
}

#[derive(Debug)]
pub struct Config {
    paths: Vec<String>,
    names: Vec<Regex>,
    entry_types: Vec<EntryType>,
}

pub fn get_args() -> MyResult<Config> {
    let matches = App::new("findr")
        .version("0.1.0")
        .author("Shaan Batra")
        .about("Rust find")
        .arg(
            Arg::with_name("paths")
                .value_name("PATH")
                .help("Search paths")
                .multiple(true)
                .default_value("."),
        )
        .arg(
            Arg::with_name("name")
                .value_name("NAME")
                .help("Name")
                .short("n")
                .long("name")
                .multiple(true),
        )
        .arg(
            Arg::with_name("type")
                .value_name("TYPE")
                .help("Entry type")
                .short("t")
                .long("type")
                .multiple(true)
                .possible_values(&["f", "d", "l"]),
        )
        .get_matches();

    let paths = matches.values_of_lossy("paths").unwrap();

    let names = match matches.values_of_lossy("name") {
        None => vec![],
        // Collect with type allows us to gather the the vector of results into a single result of a vector containing Ok() values and an error capturing any failures
        // We can then use "?" to propagate the error by returning it to the caller function, instead of panicking with unwrap()
        Some(values) => values
            .into_iter()
            .map(|s| Regex::new(&s).map_err(|_| format!("Invalid --name \"{}\"", s)))
            .collect::<Result<Vec<_>, _>>()?,
    };

    let entry_types = match matches.values_of_lossy("type") {
        None => vec![],
        Some(values) => values
            .into_iter()
            .map({
                |s| match s.as_str() {
                    "f" => File,
                    "d" => Dir,
                    "l" => Link,
                    _ => unreachable!("Invalid type"),
                }
            })
            .collect(),
    };

    Ok(Config {
        paths,
        names,
        entry_types,
    })
}

pub fn run(config: Config) -> MyResult<()> {
    let type_filter = |entry: &DirEntry| {
        config.entry_types.is_empty()
            || config
                .entry_types
                .iter()
                .any(|entry_type| match entry_type {
                    Link => entry.path_is_symlink(),
                    Dir => entry.file_type().is_dir(),
                    File => entry.file_type().is_file(),
                })
    };

    let name_filter = |entry: &DirEntry| {
        config.names.is_empty()
            || config
                .names
                .iter()
                .any(|re| re.is_match(&entry.file_name().to_string_lossy()))
    };

    for path in config.paths {
        let entries = WalkDir::new(path)
            .into_iter()
            .filter_map(|e| match e {
                Err(e) => {
                    eprintln!("{}", e);
                    None
                }
                Ok(entry) => Some(entry), // doesn't replace with Some(value), just yields values for which the closure returns Some(value)
            })
            .filter(type_filter)
            .filter(name_filter)
            .map(|entry| entry.path().display().to_string())
            .collect::<Vec<_>>();

        println!("{}", entries.join("\n"));
    }
    Ok(())
}
