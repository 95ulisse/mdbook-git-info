use crate::git_history;
use anyhow::{Context, Result};
use mdbook::book::{Book, Chapter};
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use mdbook::BookItem;
use std::collections::HashSet;

/// Preprocessor for mdBook that extracts info from the git metadata of each chapter of the book.
pub struct GitInfoPreprocessor;

impl GitInfoPreprocessor {
    pub fn new() -> GitInfoPreprocessor {
        GitInfoPreprocessor
    }
}

impl Preprocessor for GitInfoPreprocessor {
    fn name(&self) -> &str {
        "git-info"
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book> {
        // Visit each chapter of the book and accumulate and stop at the first error
        let mut error = None;
        book.for_each_mut(|book| {
            if error.is_some() {
                return;
            }

            if let BookItem::Chapter(chapter) = book {
                if let Err(e) = enrich_chapter(ctx, chapter) {
                    error = Some(e.context(format!("Chapter name: {}", chapter.name)));
                }
            }
        });

        error.map_or_else(|| Ok(book), Err)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

fn enrich_chapter(ctx: &PreprocessorContext, chapter: &mut Chapter) -> Result<()> {
    let history = git_history::extract(ctx.root.join(chapter.source_path.as_ref().unwrap()))
        .context("Cannot extract git history")?;

    // Aggregate the logs
    let last_commit = history.first();
    let first_commit = history.last();
    let mut other_contributors = history
        .iter()
        .skip(1)
        .take(history.len().saturating_sub(2))
        .filter_map(|entry| match last_commit {
            Some(last_commit) if last_commit.author == entry.author => None,
            _ => Some(entry.author.as_str()),
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    other_contributors.sort_unstable();

    // Build the output
    chapter.content.push_str(&format!(
        "\n\
        \n\
        <br>\n\
        \n\
        ---\n\
        \n\
        <br>\n\
        \n\
        | Created on | Created by | Last edit on | Last edit by | Other contributors |\n\
        | :---: | :---: | :---: | :---: | --- |\n\
        | **{}** | **{}** | **{}** | **{}** | {} |\n",
        first_commit
            .map(|c| c.timestamp.format("%d %b %Y").to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        first_commit.map(|c| c.author.as_str()).unwrap_or("n/a"),
        last_commit
            .map(|c| c.timestamp.format("%d %b %Y").to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        last_commit.map(|c| c.author.as_str()).unwrap_or("n/a"),
        other_contributors.join("<br>")
    ));

    Ok(())
}
