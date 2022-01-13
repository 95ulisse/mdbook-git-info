use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::path::Path;
use std::process::{Command, Stdio};

/// A single entry of the git log.
pub struct GitHistoryEntry {
    pub author: String,
    pub timestamp: DateTime<Utc>,
}

/// Extracts the git history of the given file using `git log`.
pub fn extract(path: impl AsRef<Path>) -> Result<Vec<GitHistoryEntry>> {
    // Launch git to extract info
    let output = Command::new("git")
        .arg("log")
        .arg("--pretty=\"%an%x09%aI\"")
        .arg("--")
        .arg(path.as_ref())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| {
            format!("Failed to launch `git log`. Is git installed and available in $PATH?")
        })?
        .wait_with_output()
        .with_context(|| format!("Failed to wait on `git log`"))?;

    // Check the result of the invocation
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "Git log failed. Exit code: {}.\nSTDOUT: {}\nSTDERR: {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(output.stdout.as_slice()),
            String::from_utf8_lossy(output.stderr.as_slice())
        ));
    }

    // Parse the git output
    let log = String::from_utf8(output.stdout)
        .context("Invalid UTF-8 output from git")?
        .lines()
        .map(|line| {
            line.trim_matches('"')
                .split('\t')
                .collect::<GitHistoryEntry>()
        })
        .collect::<Vec<_>>();

    Ok(log)
}

impl<'a> FromIterator<&'a str> for GitHistoryEntry {
    fn from_iter<T: IntoIterator<Item = &'a str>>(iter: T) -> Self {
        let mut it = iter.into_iter();
        let author = it.next().unwrap().to_string();
        let timestamp = it.next().unwrap();

        GitHistoryEntry {
            author,
            timestamp: DateTime::parse_from_rfc3339(timestamp)
                .unwrap()
                .with_timezone(&Utc),
        }
    }
}
