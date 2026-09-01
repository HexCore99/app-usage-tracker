mod application;
mod collector;
mod report;
mod startup;
mod storage;
mod tracker;

use report::show_usage;
use startup::{add_to_startup, is_startup_registered, remove_from_startup};
use tracker::{bismillah, kill_tracker, run_tracker};

fn main() {
    if !is_startup_registered() {
        add_to_startup();
    }

    let command = std::env::args().nth(1);
    match command.as_deref() {
        Some("run") => bismillah(),
        Some("spawn-child") => run_tracker(),
        Some("kill") => kill_tracker(),
        Some("usage") => show_usage(),
        Some("enable-autostart") => add_to_startup(),
        Some("disable-autostart") => remove_from_startup(),
        Some("-h") | Some("--help") => {
            println!(
                "Usage: usage-tracker <command>

Commands:
  run               Start the usage tracker
  kill              Stop the running tracker
  usage             Show saved usage
  enable-autostart  Start automatically with Windows
  disable-autostart Disable Windows auto-start
  -h, --help        Show this help message"
            );
        }
        _ => {
            println!("Usage: usage-tracker <start|end|usage>");
        }
    }
}
