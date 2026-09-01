use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::{
    CreateMutexW, DETACHED_PROCESS, OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE,
};
use windows::core::HSTRING;

use crate::application::Application;
use crate::collector::list_applications;
use crate::storage::{
    create_usage_file_if_missing, read_usage, save_usage, stop_request_path, tracker_pid_path,
};

fn update_visible_applications(
    saved_applications: &mut HashMap<String, Application>,
    applications: &HashMap<String, Application>,
) {
    for application in applications.values() {
        match saved_applications.entry(application.executable.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(application.clone());
            }

            Entry::Occupied(mut entry) => {
                let saved_application = entry.get_mut();
                saved_application.usage.total_time += Duration::from_secs(1);

                if saved_application.display_name.is_empty() {
                    saved_application.display_name = application.display_name.clone();
                }
            }
        }
    }
}

fn acquire_tracker_mutex() -> Option<HANDLE> {
    let name = HSTRING::from("Local\\AppUsageTrackerMutex");

    unsafe {
        let mutex = CreateMutexW(None, false, &name).expect("Couldn't create tracker mutex");

        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(mutex);
            return None;
        }
        Some(mutex)
    }
}

fn is_tracker_mutex_exists() -> bool {
    let name = HSTRING::from("Local\\AppUsageTrackerMutex");
    unsafe {
        match OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, &name) {
            Ok(handle) => {
                let _ = CloseHandle(handle);
                true
            }
            Err(_) => false,
        }
    }
}

pub(crate) fn bismillah() {
    if is_tracker_mutex_exists() {
        println!("App is already Running!");
        return;
    }

    let executable = std::env::current_exe().expect("Couldn't ifnd the tracker executable");

    let _child = Command::new(executable)
        .arg("spawn-child")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(DETACHED_PROCESS.0)
        .spawn()
        .expect("Couldn't start the tracker");
}

pub(crate) fn run_tracker() {
    let Some(_tracker_mutex) = acquire_tracker_mutex() else {
        return;
    };

    let _ = fs::remove_file(stop_request_path());
    fs::write(tracker_pid_path(), std::process::id().to_string())
        .expect("Couldn't write tracker.pid");

    let applications = list_applications();
    println!("{applications:#?}");
    create_usage_file_if_missing(&applications);

    let mut saved_applications = read_usage();
    let mut last_save = Instant::now();

    loop {
        let applications = list_applications();
        update_visible_applications(&mut saved_applications, &applications);

        if stop_request_path().exists() {
            save_usage(&saved_applications);
            let _ = fs::remove_file(stop_request_path());
            let _ = fs::remove_file(tracker_pid_path());
            println!("Tracker stopped and usage.json was saved.");
            break;
        }

        if last_save.elapsed() >= Duration::from_secs(60) {
            save_usage(&saved_applications);
            last_save = Instant::now();
        }

        thread::sleep(Duration::from_secs(1));
    }
}

pub(crate) fn kill_tracker() {
    if !is_tracker_mutex_exists() {
        println!("App is not running.");
        return;
    }

    fs::write(stop_request_path(), "stop").expect("Couldn't request tracker shutdown");
    println!("Stop requested. The tracker will save and exit within about one second.");
}
