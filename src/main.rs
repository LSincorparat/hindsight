use anyhow::Result;
use clap::Parser;
use hindsight::scanner::scan_repos;
use hindsight::analyzer::{analyze_repo, DayStats};
use std::collections::HashMap;
use std::path::PathBuf;

mod tui;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory to scan
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Number of days to look back
    #[arg(short, long, default_value_t = 365)]
    days: i64,

    /// Max depth for recursive search
    #[arg(long, default_value_t = 3)]
    depth: usize,

    /// List output as TSV (Date, Commits, Lines, Author)
    #[arg(long, default_value_t = false)]
    list: bool,

    /// Filter by authors (comma separated)
    #[arg(long)]
    authors: Option<String>,

    /// Export TSV to file (also prints to stdout)
    #[arg(long)]
    export_tsv: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let root_path = args.path.canonicalize()?;
    let repos = scan_repos(&root_path, args.depth);

    if repos.is_empty() {
        println!("No git repositories found in {:?}", root_path);
        return Ok(());
    }

    // Parse authors filter
    let author_filter: Option<Vec<String>> = args.authors.map(|s| {
        s.split(',')
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect()
    });

    // Accumulate stats: (Date, ProjectName, Author) -> Stats
    let mut total_stats: HashMap<(chrono::NaiveDate, String, String), DayStats> = HashMap::new();
    
    let is_list_mode = args.list || args.export_tsv.is_some();

    if !is_list_mode {
        println!("Analyzing {} repositories...", repos.len());
    }

    for repo_path in repos {
        let project_name = repo_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        match analyze_repo(&repo_path, args.days) {
            Ok(repo_stats) => {
                for ((date, author), stats) in repo_stats {
                     // Filter by author if specified
                     if let Some(ref allowed) = author_filter {
                         if !allowed.contains(&author) {
                             continue;
                         }
                     }

                     let key = (date, project_name.clone(), author);
                     let entry = total_stats.entry(key).or_default();
                     entry.commits += stats.commits;
                     entry.lines_changed += stats.lines_changed;
                }
            }
            Err(e) => {
                if !is_list_mode {
                   eprintln!("Warning: Failed to analyze {:?}: {}", repo_path, e);
                }
            }
        }
    }

    if is_list_mode {
        // Prepare TSV buffer for file export
        use std::fmt::Write;
        let mut tsv_buffer = String::new();
        writeln!(&mut tsv_buffer, "Date\tProject\tCommits\tLines\tAuthor").unwrap();

        // Prepare Table for stdout
        let mut table = comfy_table::Table::new();
        table.set_header(vec!["Date", "Project", "Commits", "Lines", "Author"]);
        table.load_preset(comfy_table::presets::UTF8_FULL);

        // Sort keys for consistent output
        let mut keys: Vec<_> = total_stats.keys().collect();
        keys.sort();
        
        for key in keys {
            let stats = &total_stats[key];
            if stats.commits > 0 || stats.lines_changed > 0 {
                // Add to TSV buffer
                writeln!(&mut tsv_buffer, "{}\t{}\t{}\t{}\t{}", key.0, key.1, stats.commits, stats.lines_changed, key.2).unwrap();
                
                // Add to Table
                table.add_row(vec![
                    key.0.to_string(),
                    key.1.clone(),
                    stats.commits.to_string(),
                    stats.lines_changed.to_string(),
                    key.2.clone(),
                ]);
            }
        }

        // Print table to stdout
        println!("{table}");

        // Write TSV to file if requested
        if let Some(path) = args.export_tsv {
            std::fs::write(&path, &tsv_buffer)?;
            println!("Exported TSV to {:?}", path);
        }

    } else {
        // Flatten for TUI: Date -> Stats (Summing all authors/projects)
        let mut tui_stats: HashMap<chrono::NaiveDate, DayStats> = HashMap::new();
        for ((date, _, _), stats) in total_stats {
            let entry = tui_stats.entry(date).or_default();
            entry.commits += stats.commits;
            entry.lines_changed += stats.lines_changed;
        }
        
        tui::run(tui_stats, args.days)?;
    }

    Ok(())
}
