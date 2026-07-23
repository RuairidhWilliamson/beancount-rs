use std::{
    io::{Read as _, stdin},
    path::PathBuf,
};

use clap::Parser;

#[derive(Parser)]
struct Cli {
    files: Vec<PathBuf>,

    #[arg(long)]
    no_sort: bool,

    #[arg(long)]
    check: bool,

    #[arg(long)]
    remove_excess_newlines: bool,
}

fn main() -> Result<(), ()> {
    let cli = Cli::parse();

    let mut check_count = 0;

    if cli.files.is_empty() {
        let mut input = String::new();
        stdin().read_to_string(&mut input).unwrap();
        let statements = process_file(&input, &cli)?;
        for s in statements {
            print!("{s}");
        }
        println!();
    } else {
        for f in &cli.files {
            let input = std::fs::read_to_string(f).unwrap();
            let statements = process_file(&input, &cli)?;
            let output: String = statements.into_iter().map(|s| s.to_string()).collect();
            if cli.check {
                if output != input {
                    eprintln!("{}", f.display());
                    check_count += 1;
                }
                continue;
            }
            std::fs::write(f, &output).unwrap();
        }
    }
    if check_count > 0 {
        eprintln!("{check_count} file(s) need to be formatted");
        return Err(());
    }
    if cli.check {
        eprintln!("All files are up to date");
    }
    Ok(())
}

fn process_file<'src>(
    input: &'src str,
    cli: &Cli,
) -> Result<Vec<bean_rs::parser::Statement<'src>>, ()> {
    match bean_rs::parser::statements(input) {
        Ok((_, mut statements)) => {
            if cli.remove_excess_newlines {
                statements.retain(|s| !s.is_newline());
            }
            if !cli.no_sort {
                bean_rs::parser::sort_directive_runs(&mut statements);
            }
            Ok(statements)
        }
        Err(err) => {
            eprintln!("{err}");
            Err(())
        }
    }
}
