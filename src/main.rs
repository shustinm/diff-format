use anyhow::{Context, Result};
use clap::Parser;
use env_logger;
use git2::Delta;
use git2::Repository;
use log::{debug, info};
use std::path::PathBuf;
use regex::Regex;

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

fn main() -> Result<()> {
    env_logger::init();
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

    diff.foreach(
        &mut |file, _| {
            info!(
                "Analyzing '{}'",
                file.new_file().path().unwrap().to_str().unwrap()
            );
            true
        },
        None, // Ignore binary files
        Some(&mut |file, hunk| match file.status() {
            Delta::Modified => {
                debug!(
                    "Changes in lines {}..{}",
                    hunk.new_start(),
                    hunk.new_start() + hunk.new_lines()
                );
                true
            }
            _ => true,
        }),
        None, // Extrapolating line information from hunks is enough, no need for line callback
    )
    .context("Issue when iterating over diff")?;

    let regex_pattern = r#":(\d+)"#;
    let regex = Regex::new(regex_pattern).expect("Failed to create regex");
    let input_string = "cluster_config.py:481:4: C0116: Missing function or method docstring (missing-function-docstring)";

    if let Some(captures) = regex.captures(input_string) {
        if let Some(number_match) = captures.get(1) {
            let number = number_match.as_str().parse::<u32>().unwrap();
            println!("Found number: {}", number);
        }
    }

    Ok(())
}
