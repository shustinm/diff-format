use anyhow::{Context, Result};
use clap::Parser;
use env_logger;
use env_logger::Env;
use git2::Delta;
use git2::Diff;
use git2::DiffOptions;
use git2::Repository;
use log::{debug, info};
use regex::Regex;
use std::collections::HashMap;
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::process;

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
            (true, true) => return true,    // Number is within the current range
            (true, false) => low = mid + 1, // Number is greater than the current range, search in the right half
            (false, _) => high = mid, // Number is less than the current range, search in the left half
        }
    }

    false
}

fn get_diff<'a>(repo: &'a Repository, gitref: &str) -> Result<Diff<'a>> {
    let gitref = repo
        .revparse_single(gitref)
        .context("Unable to parse gitref")?;
    let gitref_tree = gitref.peel_to_tree().context("Gitref is not a tree")?;
    Ok(repo.diff_tree_to_workdir_with_index(
        Some(&gitref_tree),
        // Prevent errors on untouched lines by disabling context lines
        Some(DiffOptions::new().context_lines(0)),
    )?)
}

fn generate_hunkmap(diff: &Diff) -> Result<HashMap<String, Vec<HunkRange>>> {
    let mut hunkmap = HashMap::new();

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
                hunkmap
                    .entry(path.into())
                    .or_insert_with(Vec::new)
                    .push((hunk_edges.0, hunk_edges.1));
                true
            }
            _ => true,
        }),
        None, // Extrapolating line information from hunks is enough, no need for line callback
    )
    .context("Issue when iterating over diff")?;

    Ok(hunkmap)
}

fn remove_ansi_colors(text: &str) -> String {
    let re = Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    re.replace_all(text, "").to_string()
}

fn parse_lint_location(line: &str, regex: &Regex) -> Option<(String, u32)> {
    regex.captures(line).and_then(|captures| {
        let filename = captures.get(1)?.as_str().to_string();
        let line_num = captures.get(2)?.as_str().parse().ok()?;
        Some((filename, line_num))
    })
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("warn")).init();
    let args = Args::parse();

    let repo = Repository::open(args.path).context("Can't open repository")?;
    let diff = get_diff(&repo, &args.gitref)?;

    let file_hunks = generate_hunkmap(&diff)?;

    let python_regex = Regex::new(r#"(.+?):(\d+)"#).expect("Failed to create python regex");

    let mut failed = false;
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.expect("Could not read line from stdin");

        if let Some((filename, line_num)) =
            parse_lint_location(&remove_ansi_colors(&line), &python_regex)
        {
            if let Some(hunk_ranges) = file_hunks.get(&filename) {
                if is_number_in_sorted_ranges(hunk_ranges, line_num) {
                    println!("{}", line);
                    failed = true;
                }
            }
        }
    }
    if failed {
        process::exit(1);
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::parse_lint_location;
    use regex::Regex;

    #[test]
    fn test_python_regex() {
        let python_regex = Regex::new(r#"(.+?):(\d+)"#).expect("Failed to create python regex");
        let (filename, line) =
            parse_lint_location("pysrc/main.py:753:89: E501 Line too long", &python_regex).unwrap();
        assert_eq!(filename, "pysrc/main.py");
        assert_eq!(line, 753);
    }
}
