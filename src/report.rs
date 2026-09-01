use std::time::Duration;

use crate::application::Application;
use crate::storage::read_usage;

fn create_bar(current: Duration, maximum: Duration) -> String {
    if maximum.is_zero() {
        return String::new();
    }

    let bar_length = (current.as_secs() * 30 / maximum.as_secs()) as usize;

    "█".repeat(bar_length)
}

pub(crate) fn show_usage() {
    let applications_by_executable = read_usage();

    let mut applications: Vec<Application> = applications_by_executable.values().cloned().collect();
    applications.sort_by(|a, b| b.usage.total_time.cmp(&a.usage.total_time));
    let Some(first_application) = applications.first() else {
        println!("No applications found.");
        return;
    };
    let maximum_usage = first_application.usage.total_time;

    for application in &applications {
        let bar = create_bar(application.usage.total_time, maximum_usage);

        let total_seconds = application.usage.total_time.as_secs();
        let hour = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;

        println!(
            "{:<40} | {:<30} {:02}h {:02}m\n",
            application.display_name, bar, hour, minutes
        );
    }
}
