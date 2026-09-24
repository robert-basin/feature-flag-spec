use feature_flag_spec::parser::FlagReader;
use feature_flag_spec::printer;
use std::env;
use std::fs::File;
use std::io::{self, BufReader};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut path = None;
    let mut pretty = false;
    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--pretty" => pretty = true,
            other => path = Some(other.to_string()),
        }
    }

    let path = match path {
        Some(p) => p,
        None => {
            eprintln!("usage: flagspec [--pretty] <file>");
            return ExitCode::FAILURE;
        }
    };

    let file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}: {}", path, e);
            return ExitCode::FAILURE;
        }
    };

    let reader = FlagReader::new(BufReader::new(file));
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut count = 0usize;

    for flag in reader {
        match flag {
            Ok(flag) => {
                count += 1;
                if pretty {
                    if let Err(e) = printer::write_flag(&mut out, &flag) {
                        eprintln!("write error: {}", e);
                        return ExitCode::FAILURE;
                    }
                }
            }
            Err(e) => {
                eprintln!("{}: {}", path, e);
                return ExitCode::FAILURE;
            }
        }
    }

    if !pretty {
        println!("ok: {} flag(s)", count);
    }
    ExitCode::SUCCESS
}
