use std::process::ExitCode;

use rsepuh::cmds;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match cmds::run(args) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("rsepuh: {e}");
            ExitCode::from(1)
        }
    }
}
