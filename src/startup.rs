use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, KEY_WRITE},
};

pub fn add_to_startup() {
    if is_startup_registered() {
        println!("App already registered to the Startup");
        return;
    }
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let startup_key = hkcu
        .open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            KEY_WRITE,
        )
        .expect("Failed to open registry");

    let exe_path = std::env::current_exe().expect("Cannot get executable path");

    startup_key
        .set_value("AppUsageTracker", &exe_path.to_string_lossy().to_string())
        .expect("Failed to add startup entry");

    println!("Added AppUsageTracker to Windows startup");
}

pub fn is_startup_registered() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let startup_key = match hkcu.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run") {
        Ok(key) => key,
        Err(_) => return false,
    };
    startup_key
        .get_value::<String, _>("AppUsageTracker")
        .is_ok()
}

pub fn remove_from_startup() {
    if !is_startup_registered() {
        println!("Nothing to remove");
        return;
    }
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let startup_key = hkcu
        .open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            KEY_WRITE,
        )
        .unwrap();
    let _ = startup_key.delete_value("AppUsageTracker");

    println!("Removed AppUsageTracker from Windows startup");
}
