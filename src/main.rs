use std::collections::HashMap;
use std::path::Path;
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{BOOL, CloseHandle, HWND, LPARAM};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};
use windows::core::PWSTR;

#[derive(Debug)]
struct Application {
    title: String,
    executable: String,
    pid: u32,
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
        let applications_ptr = lparam.0 as *mut HashMap<u32, Application>;
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
            title,
            executable,
            pid,
        };

        if !is_valid_application(&application) {
            return BOOL(1);
        }

        applications.entry(pid).or_insert(application);

        BOOL(1)
    }
}

fn list_applications() -> HashMap<u32, Application> {
    let mut applications: HashMap<u32, Application> = HashMap::new();
    let applications_ptr = &mut applications as *mut HashMap<u32, Application>;
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
                println!("Faile to open process, {}", error);
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
fn main() {
    let mut previous_process = String::new();
    let applications = list_applications();
    println!("{applications:#?}");

    loop {
        unsafe {
            // get the process
            let hwnd = GetForegroundWindow();
            let mut process_id = 0;
            let thread_id = GetWindowThreadProcessId(hwnd, Some(&mut process_id));

            // extract the window name
            let mut buffer = [0u16; 512];
            let length = GetWindowTextW(hwnd, &mut buffer);
            let title = String::from_utf16_lossy(&buffer[..length as usize]);

            let executable = match get_executable_path(process_id) {
                Ok(path) => path,
                Err(_) => {
                    thread::sleep(Duration::from_secs(5));
                    continue;
                }
            };

            let application = Application {
                title,
                executable,
                pid: process_id,
            };

            if application.executable != previous_process {
                println!("-------------------------");
                println!("Window: {}", application.title);
                println!("PID: {}", application.pid);
                println!("Process: {}", application.executable);
                println!("-------------------------");

                previous_process = application.executable.clone();
            }

            if thread_id == 0 {
                println!("Faile to get the process ID");
            }
            thread::sleep(Duration::from_secs(5));
        }
    }
}
