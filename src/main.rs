use git2::Repository;


fn main() {
    let repo = match Repository::open(".") {
        Ok(repo) => repo,
        Err(e) => panic!("Unable to find repo: {}", e),
    };

    println!("Hello repo");
    let gitref = repo.head().unwrap();
    println!("Current head: {}", gitref.name().unwrap())
}
