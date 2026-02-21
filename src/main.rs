use clap::Parser;
use git2::Repository;
use std::path::Path;
use std::process;

#[derive(Parser)]
#[command(name = "blamer")]
#[command(about = "A tool for working with files in git repositories", long_about = None)]
struct Cli {
    /// Path to the file
    filename: String,
}

fn main() {
    let cli = Cli::parse();

    let file_path = Path::new(&cli.filename);

    // Check if file exists
    if !file_path.exists() {
        eprintln!("Error: File '{}' does not exist", cli.filename);
        process::exit(1);
    }

    // Get the absolute path and parent directory
    let abs_path = match file_path.canonicalize() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: Could not resolve file path: {}", e);
            process::exit(1);
        }
    };

    // Try to find a git repository containing this file
    let repo_result = Repository::discover(&abs_path);

    match repo_result {
        Ok(_repo) => {
            println!("File '{}' is part of a git repository", cli.filename);
        }
        Err(e) => {
            eprintln!("Error: File '{}' is not part of a git repository: {}", cli.filename, e);
            process::exit(1);
        }
    }
}
