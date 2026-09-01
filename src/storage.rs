use std::collections::HashMap;
use std::fs::{self, File};
use std::path::PathBuf;

use crate::application::Application;

fn application_data_dir() -> PathBuf {
    let directory = dirs::data_local_dir()
        .expect("Couldn't find the local application data directory")
        .join("app-usage-tracker");

    fs::create_dir_all(&directory).expect("Couldn't create the application data directory");

    directory
}

fn usage_file_path() -> PathBuf {
    application_data_dir().join("usage.json")
}

pub(crate) fn tracker_pid_path() -> PathBuf {
    application_data_dir().join("tracker.pid")
}

pub(crate) fn stop_request_path() -> PathBuf {
    application_data_dir().join("stop.request")
}

pub(crate) fn read_usage() -> HashMap<String, Application> {
    let file = File::open(usage_file_path()).expect("Couldn't open usage.json");

    serde_json::from_reader(file).expect("Couldn't read usage.json")
}

pub(crate) fn save_usage(applications: &HashMap<String, Application>) {
    let file = File::create(usage_file_path()).expect("Couldn't open usage.json for writing");
    serde_json::to_writer_pretty(file, applications).expect("Couldn't write usage.json");
}

pub(crate) fn create_usage_file_if_missing(applications: &HashMap<String, Application>) {
    let file = match File::options()
        .write(true)
        .create_new(true)
        .open(usage_file_path())
    {
        Ok(file) => file,

        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            println!("usage.json already exists; leaving it unchanged");
            return;
        }
        Err(error) => {
            panic!("Couldnt create usage.json:{error}")
        }
    };

    serde_json::to_writer_pretty(file, applications).expect("Couldn't write usage.json");
}
