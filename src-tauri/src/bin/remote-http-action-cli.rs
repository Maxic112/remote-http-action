//! Console-subsystem companion of the main executable. Terminals wait for it and see its
//! output and exit code directly; it only forwards commands to the running application.

fn main() {
    std::process::exit(remote_http_action_lib::cli::run(std::env::args().collect()))
}
