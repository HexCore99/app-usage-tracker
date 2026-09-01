use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::time::Duration;

use windows::Win32::Foundation::{BOOL, CloseHandle, HWND, LPARAM};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};
use windows::core::{HSTRING, PWSTR};

use crate::application::{AppUsage, Application};

fn get_product_name(executable: &str) -> Option<String> {
    unsafe {
        let path = HSTRING::from(executable);
        let mut handle = 0u32;
        let size = GetFileVersionInfoSizeW(&path, Some(&mut handle));
        if size == 0 {
            return None;
        }

        let mut buffer = vec![0u8; size as usize];
        if GetFileVersionInfoW(&path, 0, size, buffer.as_mut_ptr() as *mut c_void).is_err() {
            return None;
        }

        // Find which language/codepage the file's strings are stored under.
        let mut translation_ptr: *mut c_void = std::ptr::null_mut();
        let mut translation_len = 0u32;
        if !VerQueryValueW(
            buffer.as_ptr() as *const c_void,
            &HSTRING::from("\\VarFileInfo\\Translation"),
            &mut translation_ptr,
            &mut translation_len,
        )
        .as_bool()
            || translation_ptr.is_null()
        {
            return None;
        }

        let langs = std::slice::from_raw_parts(
            translation_ptr as *const (u16, u16),
            translation_len as usize / 4,
        );
        let (lang, codepage) = *langs.first()?;

        for field in ["ProductName", "FileDescription"] {
            let query = format!("\\StringFileInfo\\{lang:04x}{codepage:04x}\\{field}");
            let mut value_ptr: *mut c_void = std::ptr::null_mut();
            let mut value_len = 0u32;

            if VerQueryValueW(
                buffer.as_ptr() as *const c_void,
                &HSTRING::from(query.as_str()),
                &mut value_ptr,
                &mut value_len,
            )
            .as_bool()
                && !value_ptr.is_null()
                && value_len > 0
            {
                let wide =
                    std::slice::from_raw_parts(value_ptr as *const u16, value_len as usize - 1);
                let name = String::from_utf16_lossy(wide).trim().to_string();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }

        None
    }
}

fn extract_name_from_path(executable: &String) -> String {
    Path::new(executable)
        .file_name()
        .map(|program_name| program_name.to_string_lossy().into_owned())
        .unwrap_or_default()
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

        let display_name =
            get_product_name(&executable).unwrap_or_else(|| extract_name_from_path(&executable));

        let application = Application {
            title: title.clone(),
            name: extract_name_from_path(&executable),
            display_name,
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

pub(crate) fn list_applications() -> HashMap<String, Application> {
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
        let process_handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id)
        {
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
