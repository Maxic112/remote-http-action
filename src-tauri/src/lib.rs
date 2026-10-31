pub mod actions;
pub mod cli;
pub mod core;
pub mod db;
pub mod error;
pub mod events;
mod gui;
mod headless;
pub mod http;
pub mod ipc;
pub mod model;
pub mod paths;
pub mod platform;
pub mod runner;
pub mod secrets;
pub mod settings;

/// Arguments understood by the application itself; anything else is treated as a CLI command.
const APP_FLAGS: &[&str] = &["--minimized", "--wait-pid", "--headless"];

pub fn main_entry() {
    let args: Vec<String> = std::env::args().collect();
    let is_cli = args.len() > 1 && !APP_FLAGS.contains(&args[1].as_str());
    if is_cli {
        #[cfg(windows)]
        platform::win::attach_parent_console();
        std::process::exit(cli::run(args));
    }

    // After an elevated relaunch, wait for the previous instance to release the
    // single-instance lock, the CLI pipe and the HTTP port.
    if let Some(pid) = args
        .iter()
        .position(|a| a == "--wait-pid")
        .and_then(|i| args.get(i + 1))
        .and_then(|p| p.parse::<u32>().ok())
    {
        #[cfg(windows)]
        platform::win::wait_for_pid(pid, 15_000);
        #[cfg(not(windows))]
        let _ = pid;
    }
    if args.iter().any(|a| a == "--headless") {
        #[cfg(windows)]
        platform::win::attach_parent_console();
        std::process::exit(headless::run());
    }
    gui::run(args.iter().any(|a| a == "--minimized"));
}
