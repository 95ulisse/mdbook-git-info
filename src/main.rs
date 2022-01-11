use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{App, Arg, ArgMatches, SubCommand};
use mdbook::book::{Book, BookItem, Chapter};
use mdbook::preprocess::{CmdPreprocessor, Preprocessor, PreprocessorContext};
use std::collections::HashSet;
use std::io;
use std::process;

fn make_app() -> App<'static, 'static> {
    App::new("mdbook-git-info")
        .about("A mdbook preprocessor which extracts metadata from Git and adds it to the chapters of the book")
        .subcommand(
            SubCommand::with_name("supports")
                .arg(Arg::with_name("renderer").required(true))
                .about("Check whether a renderer is supported by this preprocessor")
        )
}

fn main() {
    let matches = make_app().get_matches();

    // Users will want to construct their own preprocessor here
    let preprocessor = GitInfo::new();

    if let Some(sub_args) = matches.subcommand_matches("supports") {
        handle_supports(&preprocessor, sub_args);
    } else if let Err(e) = handle_preprocessing(&preprocessor) {
        eprintln!("{}", e);
        process::exit(1);
    }
}

/// Pre-processor starter, taken straight out of the mdbook book
fn handle_preprocessing(pre: &dyn Preprocessor) -> Result<()> {
    let (ctx, book) = CmdPreprocessor::parse_input(io::stdin())?;

    if ctx.mdbook_version != mdbook::MDBOOK_VERSION {
        // We should probably use the `semver` crate to check compatibility
        // here...
        eprintln!(
            "Warning: The {} plugin was built against version {} of mdbook, \
             but we're being called from version {}",
            pre.name(),
            mdbook::MDBOOK_VERSION,
            ctx.mdbook_version
        );
    }

    let processed_book = pre.run(&ctx, book)?;
    serde_json::to_writer(io::stdout(), &processed_book)?;

    Ok(())
}

/// Check to see if we support the processor, taken straight out of the mdbook book
fn handle_supports(pre: &dyn Preprocessor, sub_args: &ArgMatches) -> ! {
    let renderer = sub_args.value_of("renderer").expect("Required argument");
    let supported = pre.supports_renderer(&renderer);

    if supported {
        process::exit(0);
    } else {
        process::exit(1);
    }
}

pub struct GitInfo;

impl GitInfo {
    pub fn new() -> GitInfo {
        GitInfo
    }
}

impl Preprocessor for GitInfo {
    fn name(&self) -> &str {
        "git-info"
    }

    fn run(&self, _ctx: &PreprocessorContext, mut book: Book) -> Result<Book> {
        book.for_each_mut(|book| {
            if let BookItem::Chapter(chapter) = book {
                if let Err(e) = enrich_chapter(chapter) {
                    panic!("mdbook-git-info error: {:?}", e);
                }
            }
        });

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

fn enrich_chapter(chapter: &mut Chapter) -> Result<()> {
    use std::process::{Command, Stdio};

    // Launch git to extract info
    let child = match Command::new("git")
        .arg("log")
        .arg("--pretty=\"%an%x09%aI\"")
        .arg("--")
        .arg(format!(
            "src/{}",
            chapter.source_path.as_ref().unwrap().display()
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "failed to launch git, not adding git-info to chapter {}: {:?}",
                chapter.source_path.as_ref().unwrap().display(),
                e
            );
            return Err(e.into());
        }
    };

    let output = child.wait_with_output().expect("can launch git");

    // check for failure
    if !output.status.success() {
        eprintln!("git failed, exit code: {:?}", output.status.code());

        eprintln!("git STDOUT:");
        eprintln!("{}", String::from_utf8_lossy(output.stdout.as_ref()));
        eprintln!("git STDERR:");
        eprintln!("{}", String::from_utf8_lossy(output.stdout.as_ref()));
        eprintln!("/git output");

        return Err(anyhow::anyhow!("Git invocation failed"));
    }

    // Parse the git output
    let log = String::from_utf8(output.stdout)
        .expect("valid utf-8")
        .lines()
        .map(|line| line.trim_matches('"').split('\t').collect::<GitLogEntry>())
        .collect::<Vec<_>>();

    // Aggregate the logs
    let last_commit = log.first();
    let first_commit = log.last();
    let mut other_contributors = log
        .iter()
        .skip(1)
        .take(log.len().saturating_sub(2))
        .filter_map(|entry| match last_commit {
            Some(last_commit) if last_commit.author == entry.author => None,
            _ => Some(entry.author.as_str()),
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    other_contributors.sort();

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
        | Created on | Last edit on | By | Other contributors |\n\
        | --- | --- | --- | --- |\n\
        | **{}** | **{}** | **{}** | {} |\n",
        first_commit
            .map(|c| c.timestamp.format("%d %b %Y").to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        last_commit
            .map(|c| c.timestamp.format("%d %b %Y").to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        last_commit.map(|c| c.author.as_str()).unwrap_or("n/a"),
        other_contributors.join("<br>")
    ));

    Ok(())
}

struct GitLogEntry {
    author: String,
    timestamp: DateTime<Utc>,
}

impl<'a> FromIterator<&'a str> for GitLogEntry {
    fn from_iter<T: IntoIterator<Item = &'a str>>(iter: T) -> Self {
        let mut it = iter.into_iter();
        let author = it.next().unwrap().to_string();
        let timestamp = it.next().unwrap();

        GitLogEntry {
            author,
            timestamp: DateTime::parse_from_rfc3339(timestamp)
                .unwrap()
                .with_timezone(&Utc),
        }
    }
}
