use anyhow::{Result};
use chrono::{Duration, Local, NaiveDate};
use git2::{Repository, Time};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct DayStats {
    pub commits: usize,
    pub lines_changed: usize, // insertions + deletions
}

pub fn analyze_repo<P: AsRef<Path>>(
    path: P,
    days: i64,
) -> Result<HashMap<(NaiveDate, String), DayStats>> {
    let repo = Repository::open(path)?;
    
    // Check if repo is empty or head is missing
    if repo.head().is_err() {
        return Ok(HashMap::new()); // Empty or invalid repo, just skip
    }

    let mut revwalk = repo.revwalk()?;
    // Try pushing head, if it fails (e.g. unborn), just return empty
    if revwalk.push_head().is_err() {
        return Ok(HashMap::new());
    }
    
    revwalk.set_sorting(git2::Sort::TIME)?;

    let now = Local::now().date_naive();
    let start_date = now - Duration::days(days);
    
    let mut stats: HashMap<(NaiveDate, String), DayStats> = HashMap::new();

    for oid in revwalk {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        
        let time = commit.time();
        let date = time_to_date(time);

        if date < start_date {
            break; 
        }

        let author = commit.author().name().unwrap_or("Unknown").to_string();
        let key = (date, author);
        
        // Count commit
        stats.entry(key.clone()).or_default().commits += 1;

        // Calculate diff for lines changed
        let mut lines = 0;
        if commit.parent_count() > 0 {
            // Compare with first parent
            if let Ok(parent) = commit.parent(0) {
                 if let (Ok(p_tree), Ok(c_tree)) = (parent.tree(), commit.tree()) {
                    if let Ok(diff) = repo.diff_tree_to_tree(Some(&p_tree), Some(&c_tree), None) {
                        if let Ok(diff_stats) = diff.stats() {
                             lines = diff_stats.insertions() + diff_stats.deletions();
                        }
                    }
                 }
            }
        } else {
             // Initial commit
             if let Ok(tree) = commit.tree() {
                 if let Ok(diff) = repo.diff_tree_to_tree(None, Some(&tree), None) {
                     if let Ok(diff_stats) = diff.stats() {
                        lines = diff_stats.insertions() + diff_stats.deletions();
                     }
                 }
             }
        }
        stats.entry(key).or_default().lines_changed += lines;
    }

    Ok(stats)
}

fn time_to_date(time: Time) -> NaiveDate {
    let seconds = time.seconds();
    let dt = chrono::DateTime::from_timestamp(seconds, 0);
    if let Some(dt) = dt {
        dt.with_timezone(&Local).date_naive()
    } else {
        Local::now().date_naive()
    }
}
