use std::{
    io::{Read as _, stdin},
    path::PathBuf,
};

use beancount_rs::{model::Directive, parser::Statement};
use clap::Parser;

#[derive(Parser)]
struct Cli {
    files: Vec<PathBuf>,
}

fn main() -> Result<(), ()> {
    let cli = Cli::parse();

    if cli.files.is_empty() {
        let mut input = String::new();
        stdin().read_to_string(&mut input).unwrap();
        process_file(&input)
    } else {
        for f in &cli.files {
            let input = std::fs::read_to_string(f).unwrap();
            process_file(&input)?;
        }
        Ok(())
    }
}

fn process_file(input: &str) -> Result<(), ()> {
    match beancount_rs::parser::statements(input) {
        Ok((_, statements)) => {
            let directives = statements
                .into_iter()
                .filter_map(|s| {
                    if let Statement::Directive(d) = s {
                        Some(Directive::try_from(d))
                    } else {
                        None
                    }
                })
                .collect::<Result<Vec<Directive>, _>>()
                .unwrap();
            eprintln!("{directives:#?}");
            Ok(())
        }
        Err(err) => {
            eprintln!("{err}");
            Err(())
        }
    }
}
