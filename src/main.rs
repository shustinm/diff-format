use anyhow::{Context, Result};
use clap::Parser;
use env_logger;
use env_logger::Env;
use git2::Delta;
use git2::Repository;
use log::{debug, info};
use regex::Regex;
use std::collections::HashMap;
use std::io::{self, BufRead};
use std::path::PathBuf;

type HunkRange = (u32, u32);

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to repository
    #[arg(short, long, default_value = ".")]
    path: PathBuf,

    #[arg(short, long, default_value = "origin")]
    remote: String,

    #[arg(short, long, default_value = "master")]
    gitref: String,
}

fn is_number_in_sorted_ranges(ranges: &[(u32, u32)], number: u32) -> bool {
    let mut low = 0;
    let mut high = ranges.len();

    while low < high {
        let mid = low + (high - low) / 2;
        match (number >= ranges[mid].0, number <= ranges[mid].1) {
            (true, true) => return true, // Number is within the current range
            (true, false) => low = mid + 1, // Number is greater than the current range, search in the right half
            (false, _) => high = mid, // Number is less than the current range, search in the left half
        }
    }

    false
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let args = Args::parse();

    let repo = Repository::open(args.path).context("Can't open repository")?;
    let gitref = repo.resolve_reference_from_short_name(&args.gitref)?;
    debug!(
        "Repo head is '{}'",
        gitref.name().expect("gitref is not UTF-8")
    );
    debug!("Repo head shortname '{}'", gitref.shorthand().unwrap());
    let gitref_tree = gitref.peel_to_tree().context("Unable to peel gitref")?;
    let diff = repo.diff_tree_to_workdir_with_index(Some(&gitref_tree), None)?;

    let mut file_hunks: HashMap<String, Vec<HunkRange>> = HashMap::new();

    diff.foreach(
        &mut |file, _| {
            let path = file.new_file().path().unwrap().to_str().unwrap();
            info!("Analyzing '{}'", path);
            true
        },
        None, // Ignore binary files
        Some(&mut |file, hunk| match file.status() {
            Delta::Modified => {
                let path = file.new_file().path().unwrap().to_str().unwrap();
                let hunk_edges = (hunk.new_start(), hunk.new_start() + hunk.new_lines());
                debug!("Changes in lines {}..{}", hunk_edges.0, hunk_edges.1);
                file_hunks
                    .entry(path.into())
                    .or_insert_with(|| Vec::new())
                    .push((hunk_edges.0, hunk_edges.1));
                true
            }
            _ => true,
        }),
        None, // Extrapolating line information from hunks is enough, no need for line callback
    )
    .context("Issue when iterating over diff")?;

    debug!("{:?}", file_hunks);

    let regex_pattern = r#"(.+?):(\d+)"#;
    let regex = Regex::new(regex_pattern).expect("Failed to create regex");

    let stdin = io::stdin();

    for line in stdin.lock().lines() {
        let line = line.expect("Could not read line from standard in");

        if let Some(captures) = regex.captures(&line) {
            let filename = captures.get(1).unwrap().as_str();
            let line_num_match = captures.get(2).unwrap().as_str();

            if let Some(hunk_ranges) = file_hunks.get(filename) {
                let line_num = line_num_match.parse::<u32>().unwrap();
                if is_number_in_sorted_ranges(hunk_ranges, line_num) {
                    println!("{}", line);
                }
            }
        }
    }

    Ok(())
}
