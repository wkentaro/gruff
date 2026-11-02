use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let unused_canary = 1;
    let arguments = gruff::Arguments::parse();

    match gruff::run(arguments) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("gruff failed\n  Cause: {error}");
            ExitCode::from(2)
        }
    }
}
