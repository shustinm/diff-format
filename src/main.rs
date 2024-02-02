use git2::Repository;
use git2::Delta;
use clap::Parser;
use std::path::PathBuf;
use anyhow::{Context, Result};


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
    let args = Args::parse();

    let repo = Repository::open(args.path).context("Can't open repository")?;
    let head = repo.head().context("Can't retreive repo head")?;
    let gitref = repo.resolve_reference_from_short_name(&args.gitref)?;
    println!("Repo head is {:?}", gitref.shorthand().unwrap());
    let head_tree = head.peel_to_tree().context(format!("Unable to peel head: {:?}", head.name()))?;
    let diff = repo.diff_tree_to_workdir_with_index(Some(&head_tree), None)?;

    diff.foreach(
        &mut |file, _| {
            println!("Analyzing '{}'", file.new_file().path().unwrap().to_str().unwrap());
            true
        },
        None,  // Ignore binary files
        Some(&mut |file, hunk| {
            match file.status() {
                Delta::Modified => {
                    println!("\tChanges in lines {}...{}", hunk.new_start(), hunk.new_lines());
                    true
                }
                _ => true
            }
        }),
        None,  // Extrapolating line information from hunks is enough, no need for line callback
    ).context("Issue when iterating over diff")?;


    Ok(())
}
