use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};
use windows::core::PWSTR;

struct Process {
    title: String,
    executable: String,
    pid: u32,
    // process_handle: HANDLE,
}
struct RunningProcess {
    pid: u32,
    executable: String,
}
fn is_user_app(process_name: &str) -> bool {
    let ignored = [
        "svchost.exe",
        "System",
        "Registry",
        "csrss.exe",
        "wininit.exe",
        "services.exe",
        "lsass.exe",
        "dwm.exe",
        "fontdrvhost.exe",
        "RuntimeBroker.exe",
    ];

    !ignored.contains(&process_name)
}

fn list_running_processes() -> Vec<RunningProcess> {
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                println!("Failed to create process snapshot: {}", error);
                return Vec::new();
            }
        };

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if let Err(error) = Process32FirstW(snapshot, &mut entry) {
            println!("Failed to read first process: {}", error);
            let _ = CloseHandle(snapshot);
            return Vec::new();
        }
        let mut processes = Vec::new();

        loop {
            let length = entry
                .szExeFile
                .iter()
                .position(|&character| character == 0)
                .unwrap_or(entry.szExeFile.len());
            let executable = String::from_utf16_lossy(&entry.szExeFile[..length]);

            if is_user_app(&executable) {
                processes.push(RunningProcess {
                    pid: entry.th32ProcessID,
                    executable,
                });
            }

            if Process32NextW(snapshot, &mut entry).is_err() {
                break;
            }
        }
        let _ = CloseHandle(snapshot);
        processes
    }
}

fn extract_process_info(title: &String, process_id: u32) -> Result<Process, ()> {
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
            return Err(());
        }

        let name = String::from_utf16_lossy(&process_name[..length as usize]);

        Ok(Process {
            title: title.to_string(),
            executable: name,
            pid: process_id,
        })
    }
}
fn main() {
    let mut previous_process = String::new();
    let mut total_process = 0;

    println!("********* ************** ********** ");
    println!();
    for process in list_running_processes() {
        total_process += 1;
        println!("PID: {} | Process: {}", process.pid, process.executable);
    }
    println!();
    println!("Total processes: {}", total_process);
    println!("********* ************** ********** ");

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

            let process = match extract_process_info(&title, process_id) {
                Ok(proc) => proc,
                Err(_) => {
                    thread::sleep(Duration::from_secs(5));
                    continue;
                }
            };
            if process.executable != previous_process {
                println!("-------------------------");
                println!("Window: {}", process.title);
                println!("PID: {}", process.pid);
                println!("Process: {}", process.executable);
                println!("-------------------------");

                previous_process = process.executable.clone();
            }

            if thread_id == 0 {
                println!("Faile to get the process ID");
            }
            thread::sleep(Duration::from_secs(5));
        }
    }
}
