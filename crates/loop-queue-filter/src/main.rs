#![forbid(unsafe_code)]

use std::io::{self, Read};

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--selftest-guard") {
        println!("queue-filter guard: PASS (Rust binary present)");
        return std::process::ExitCode::SUCCESS;
    }
    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        return std::process::ExitCode::from(1);
    }
    let output = loop_queue_filter::run(&input, &args, &loop_queue_filter::Runtime::from_process());
    print!("{}", output.stdout);
    eprint!("{}", output.stderr);
    std::process::ExitCode::from(output.code as u8)
}
