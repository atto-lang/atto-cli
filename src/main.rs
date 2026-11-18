use atto_cli::{execute_run, parse_run_args};
use std::process::ExitCode;

fn usage() {
    eprintln!(
        "usage: atto run [--single-threaded | --max-threads-per-space=1 | \
         --mode=single-threaded] [--backend=wasm] \
         --runner=/absolute/path/to/run-guest.mjs \
         --sha256=<guest-sha256> /absolute/guest.wasm\n\
         The current Rust CLI admits only pinned scalar Wasm artifacts; \
         .atto source and native backend execution are not wired."
    );
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        usage();
        return ExitCode::SUCCESS;
    }
    match args.first().map(String::as_str) {
        Some("run") => {
            let request = match parse_run_args(&args[1..]) {
                Ok(r) => r,
                Err(reason) => {
                    eprintln!("{reason}");
                    usage();
                    return ExitCode::from(2);
                }
            };
            match execute_run(&request) {
                Ok(0) => ExitCode::SUCCESS,
                Ok(code) => {
                    eprintln!("E_GUEST_EXIT: pinned scalar Wasm host exited with status {code}");
                    ExitCode::from(1)
                }
                Err(reason) => {
                    eprintln!("{reason}");
                    ExitCode::from(2)
                }
            }
        }
        _ => {
            eprintln!("E_UNKNOWN_COMMAND: this bootstrap currently implements run only");
            usage();
            ExitCode::from(2)
        }
    }
}
