use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs::{self, File};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::{
    BOOL, CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, HWND, LPARAM,
};
use windows::Win32::System::Threading::{
    CreateMutexW, DETACHED_PROCESS, OpenMutexW, OpenProcess, PROCESS_NAME_WIN32,
    PROCESS_QUERY_INFORMATION, PROCESS_TERMINATE, QueryFullProcessImageNameW,
    SYNCHRONIZATION_SYNCHRONIZE, TerminateProcess,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};
use windows::core::{HSTRING, PWSTR};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Application {
    title: String,
    name: String,
    executable: String,
    pid: u32,
    usage: AppUsage,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AppUsage {
    total_time: Duration,
}

// funcs
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

fn tracker_pid_path() -> PathBuf {
    application_data_dir().join("tracker.pid")
}

fn migrate_legacy_usage_file() {
    let new_path = usage_file_path();
    let old_path = Path::new("usage.json");

    if !new_path.exists() && old_path.exists() {
        fs::copy(old_path, &new_path).expect("Couldn't migrate usage.json");
        println!("Migrated usage data to {}", new_path.display());
    }
}

fn read_usage() -> HashMap<String, Application> {
    let file = File::open(usage_file_path()).expect("Couldn't open usage.json");

    serde_json::from_reader(file).expect("Couldn't read usage.json")
}

fn append_to_the_file(applications: &HashMap<String, Application>) {
    let file = File::open(usage_file_path()).expect("Couldn't open usage.json");
    let mut saved_applications: HashMap<String, Application> =
        serde_json::from_reader(file).expect("Couldn't read applications from usage.json");

    for application in applications.values() {
        match saved_applications.entry(application.executable.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(application.clone());
            }

            Entry::Occupied(mut entry) => {
                let saved_application = entry.get_mut();
                saved_application.usage.total_time += Duration::from_secs(30);
            }
        }
    }
    overwrite_existing(&saved_applications, true);
}

fn overwrite_existing(app: &HashMap<String, Application>, create_new: bool) {
    if create_new {
        let file = File::create(usage_file_path()).expect("Couldn't open usage.sjon for writing");
        serde_json::to_writer_pretty(file, &app).expect("Couldn't write usage.json");
        return;
    }

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

    let res = serde_json::to_writer_pretty(file, &app);
    println!("output: {:?}", res);
}

fn extract_name_from_path(executable: &String) -> String {
    let app_name: String = Path::new(&executable)
        .file_name()
        .map(|program_name| program_name.to_string_lossy().into_owned())
        .unwrap_or_default();
    app_name
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

fn is_windows_system_path(executable: &str) -> bool {
    let Some(windows_directory) = std::env::var_os("WINDIR") else {
        return false;
    };

    let windows_directory = windows_directory
        .to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase();
    let executable = executable.replace('/', "\\").to_lowercase();

    executable.starts_with(&format!("{windows_directory}\\"))
}

fn is_valid_application(app: &Application) -> bool {
    let title = app.title.trim().to_lowercase();

    let executable_name = Path::new(&app.executable)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_lowercase();

    if title.is_empty() || executable_name.is_empty() {
        return false;
    }

    // Explicit overrides belong at the top so a user choice wins over heuristics.
    const ALLOWED_EXECUTABLES: &[&str] = &[];
    const BLOCKED_EXECUTABLES: &[&str] = &[];

    if ALLOWED_EXECUTABLES.contains(&executable_name.as_str()) {
        return true;
    }

    if BLOCKED_EXECUTABLES.contains(&executable_name.as_str()) {
        return false;
    }

    const BLOCKED_TITLES: &[&str] = &[
        "program manager",
        "windows input experience",
        "nvidia geforce overlay",
        "powertoys quick access (preview)",
        "settings",
    ];

    if BLOCKED_TITLES.contains(&title.as_str()) {
        return false;
    }

    const HELPER_TITLE_MARKERS: &[&str] =
        &["gracefulshutdownwindow", "uiaccesshelperwindow", "crashpad"];

    if HELPER_TITLE_MARKERS
        .iter()
        .any(|marker| title.contains(marker))
    {
        return false;
    }

    const HELPER_EXECUTABLES: &[&str] = &["crashpad_handler.exe", "squirrel.exe", "update.exe"];

    if HELPER_EXECUTABLES.contains(&executable_name.as_str()) {
        return false;
    }

    const WINDOWS_SYSTEM_COMPONENTS: &[&str] = &[
        "lockapp.exe",
        "searchhost.exe",
        "shellexperiencehost.exe",
        "startmenuexperiencehost.exe",
        "systemsettings.exe",
        "textinputhost.exe",
    ];

    if is_windows_system_path(&app.executable)
        && WINDOWS_SYSTEM_COMPONENTS.contains(&executable_name.as_str())
    {
        return false;
    }

    true
}

unsafe extern "system" fn collect_application(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let applications_ptr = lparam.0 as *mut HashMap<String, Application>;

        let applications = &mut *applications_ptr;

        if !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1);
        }

        let mut buffer = [0u16; 512];
        let length = GetWindowTextW(hwnd, &mut buffer);

        if length == 0 {
            return BOOL(1);
        }

        let title = String::from_utf16_lossy(&buffer[..length as usize]);

        let mut pid = 0u32;
        if GetWindowThreadProcessId(hwnd, Some(&mut pid)) == 0 {
            return BOOL(1);
        }

        let executable = match get_executable_path(pid) {
            Ok(path) => path,
            Err(_) => return BOOL(1),
        };

        let application = Application {
            title: title.clone(),
            name: extract_name_from_path(&executable),
            executable: executable.clone(),
            pid,
            usage: AppUsage {
                total_time: Duration::ZERO,
            },
        };

        if !is_valid_application(&application) {
            return BOOL(1);
        }

        applications.entry(executable).or_insert(application);

        BOOL(1)
    }
}

fn list_applications() -> HashMap<String, Application> {
    let mut applications: HashMap<String, Application> = HashMap::new();
    let applications_ptr = &mut applications as *mut HashMap<String, Application>; //borrwo the applications mutably and get the raw pointer to the first element
    unsafe {
        if let Err(error) =
            EnumWindows(Some(collect_application), LPARAM(applications_ptr as isize))
        {
            println!("EnumWindows failed: {}", error);
        }
    }

    applications
}

fn get_executable_path(process_id: u32) -> Result<String, ()> {
    // get the process handle
    unsafe {
        let process_handle = match OpenProcess(PROCESS_QUERY_INFORMATION, false, process_id) {
            Ok(handle) => handle,
            Err(error) => {
                println!("Failed to open process, {}", error);
                return Err(());
            }
        };

        // extract the process name
        let mut process_name = [0u16; 512];
        let mut length = process_name.len() as u32;
        if let Err(error) = QueryFullProcessImageNameW(
            process_handle,
            PROCESS_NAME_WIN32,
            PWSTR(process_name.as_mut_ptr()), //as_mut_ptr gives *mut u16 which is a raw pointer to the first element.
            &mut length,
        ) {
            println!("Failed to get process path: {}", error);
            let _ = CloseHandle(process_handle);
            return Err(());
        }

        let name = String::from_utf16_lossy(&process_name[..length as usize]);
        let _ = CloseHandle(process_handle);

        Ok(name)
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

fn bismillah() {
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

fn run_tracker() {
    let Some(_tracker_mutex) = acquire_tracker_mutex() else {
        return;
    };
    fs::write(tracker_pid_path(), std::process::id().to_string())
        .expect("Couldn't write tracker.pid");

    let applications = list_applications();
    println!("{applications:#?}");
    overwrite_existing(&applications, false);

    loop {
        let applications = list_applications();
        append_to_the_file(&applications);

        thread::sleep(Duration::from_secs(30));
    }
}
fn kill_tracker() {
    let pid_path = tracker_pid_path();
    let pid = fs::read_to_string(&pid_path).expect("App is not running");
    let pid: u32 = pid.parse().expect("Invalid PID");

    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, false, pid).expect("Cannot open tracker");

        TerminateProcess(handle, 0).expect("Failed to terminate");
        let _ = CloseHandle(handle);
    }
    let _ = fs::remove_file(pid_path).expect("tracker.pid file dont exists! :(");
}
fn create_bar(current: Duration, maximum: Duration) -> String {
    if maximum.is_zero() {
        return String::new();
    }

    let bar_length = (current.as_secs() * 30 / maximum.as_secs()) as usize;

    "█".repeat(bar_length)
}
fn show_usage() {
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
            "{:<24} | {:<30} {:02}h {:02}m\n",
            application.name, bar, hour, minutes
        );
    }
}

fn main() {
    migrate_legacy_usage_file();

    let command = std::env::args().nth(1);
    match command.as_deref() {
        Some("run") => bismillah(),
        Some("spawn-child") => run_tracker(),
        Some("kill") => kill_tracker(),
        Some("usage") => show_usage(),
        _ => {
            println!("Usage: usage-tracker <start|end|usage>");
        }
    }
}
