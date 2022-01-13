mod git_history;
mod preprocessor;

use crate::preprocessor::GitInfoPreprocessor;
use anyhow::Result;
use clap::{App, Arg, ArgMatches, SubCommand};
use mdbook::preprocess::{CmdPreprocessor, Preprocessor};
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
    let preprocessor = GitInfoPreprocessor::new();

    if let Some(sub_args) = matches.subcommand_matches("supports") {
        handle_supports(preprocessor, sub_args);
    } else if let Err(e) = handle_preprocessing(preprocessor) {
        eprintln!("{:?}", e);
        process::exit(1);
    }
}

/// Pre-processor starter, taken straight out of the mdbook book
fn handle_preprocessing(pre: impl Preprocessor) -> Result<()> {
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
fn handle_supports(pre: impl Preprocessor, sub_args: &ArgMatches) -> ! {
    let renderer = sub_args.value_of("renderer").expect("Required argument");
    let supported = pre.supports_renderer(renderer);

    if supported {
        process::exit(0);
    } else {
        process::exit(1);
    }
}
