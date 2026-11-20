use atto_cli::{execute_run, parse_cli_args};
use flags2env::BundledFlags2Env;
use std::process::ExitCode;

fn main() -> ExitCode {
    // Invalid Unicode must be a controlled rejection, never an args() panic.
    let args: Vec<String> = match std::env::args_os().map(|s| s.into_string())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(args) => args,
        Err(_) => {
            eprintln!("E_ARGV_ENCODING: CLI arguments must be valid UTF-8");
            return ExitCode::from(2);
        }
    };
    let parser = BundledFlags2Env::new();
    if let Ok(true) = parser.is_help_requested(&args) {
        let schema = concat!(env!("CARGO_MANIFEST_DIR"), "/.cli-flags.toml");
        if parser.audit_config(Some(schema)).is_err() {
            eprintln!("E_CLI_SCHEMA: invalid bundled option schema");
            return ExitCode::from(2);
        }
        match parser.help_table_for_argv("atto", &args, 0, Some(schema)) {
            Ok(help) => {
                println!("{help}");
                return ExitCode::SUCCESS;
            }
            Err(_) => {
                eprintln!("E_CLI_HELP: unable to render help from the authoritative schema");
                return ExitCode::from(2);
            }
        }
    }
    let request = match parse_cli_args(&args[1..]) {
        Ok(request) => request,
        Err(reason) => {
            eprintln!("{reason}");
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
