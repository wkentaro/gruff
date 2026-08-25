use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let arguments = ruffhouse::Arguments::parse();

    match ruffhouse::run(arguments) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("ruffhouse failed\n  Cause: {error}");
            ExitCode::from(2)
        }
    }
}
