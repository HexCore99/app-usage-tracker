# LEARN.md

## Windows API Reference

- Crate: `windows = "0.58"`
- API style: wide-character Win32 functions ending in `W`
- Source files:
  - `src/collector.rs`
  - `src/tracker.rs`
  - `src/startup.rs`

## 1. Window APIs

### `EnumWindows`

- Call: `EnumWindows(Some(collect_application), LPARAM(applications_ptr as isize))`
- Does:
  - Enumerates every top-level desktop window.
  - Calls `collect_application` once for each window.
- Parameters:
  - `lpEnumFunc`:
    - Callback invoked for each top-level window.
    - Project value: `Some(collect_application)`.
  - `lParam`:
    - Custom pointer-sized value forwarded to every callback call.
    - Project value: raw pointer to `HashMap<String, Application>`.
- Returns:
  - `Ok(())`: enumeration finished.
  - `Err(error)`: enumeration failed or was stopped.
- Project use:
  - Builds the current map of visible applications.

### `collect_application` callback

- Signature: `unsafe extern "system" fn collect_application(hwnd: HWND, lparam: LPARAM) -> BOOL`
- Does:
  - Receives one window from `EnumWindows`.
  - Collects its title, process ID, executable path, and display name.
- Parameters:
  - `hwnd`:
    - Handle identifying the current window.
  - `lparam`:
    - Custom value originally passed to `EnumWindows`.
    - Converted back to `*mut HashMap<String, Application>`.
- Returns:
  - `BOOL(1)`: continue enumeration.
  - `BOOL(0)`: stop enumeration.

### `IsWindowVisible`

- Call: `IsWindowVisible(hwnd)`
- Does:
  - Checks whether Windows marks a window as visible.
- Parameters:
  - `hWnd`:
    - Handle of the window being checked.
- Returns:
  - Nonzero `BOOL`: visible.
  - Zero `BOOL`: not visible.
- Project use:
  - Rejects hidden windows.
- Important:
  - Visible does not mean foreground, focused, or actively used.
  - A minimized window may still be marked visible.

### `GetWindowTextW`

- Call: `GetWindowTextW(hwnd, &mut buffer)`
- Does:
  - Copies a window title into a UTF-16 buffer.
- Parameters:
  - `hWnd`:
    - Handle of the target window.
  - `lpString`:
    - Mutable UTF-16 output buffer.
    - Project value: `[0u16; 512]`.
    - The Rust slice supplies the buffer capacity to the wrapper.
- Returns:
  - Positive number: UTF-16 code units copied, excluding the null terminator.
  - Zero: no title or failure.
- Project use:
  - Converts `&buffer[..length as usize]` with `String::from_utf16_lossy`.

### `GetWindowThreadProcessId`

- Call: `GetWindowThreadProcessId(hwnd, Some(&mut pid))`
- Does:
  - Finds the thread and process that own a window.
- Parameters:
  - `hWnd`:
    - Handle of the target window.
  - `lpdwProcessId`:
    - Optional mutable output location for the process ID.
    - Project value: `Some(&mut pid)`.
- Returns:
  - Nonzero: ID of the thread that created the window.
  - Zero: failure.
- Project use:
  - Supplies the process ID used by `OpenProcess`.

## 2. Process APIs

### `OpenProcess`

- Call: `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id)`
- Does:
  - Opens an existing process.
- Parameters:
  - `dwDesiredAccess`:
    - Access rights requested for the process handle.
    - Project value: `PROCESS_QUERY_LIMITED_INFORMATION`.
  - `bInheritHandle`:
    - Controls whether child processes inherit the returned handle.
    - Project value: `false`.
  - `dwProcessId`:
    - Numeric ID of the process to open.
- Returns:
  - `Ok(HANDLE)`: process opened.
  - `Err(error)`: access denied, process ended, or another failure.
- Project use:
  - Opens the process that owns a discovered window.

### `PROCESS_QUERY_LIMITED_INFORMATION`

- Type:
  - Process access-right constant.
- Means:
  - Requests limited process information.
  - Allows executable-path queries without requesting broader memory access.

### `QueryFullProcessImageNameW`

- Call:

```rust
QueryFullProcessImageNameW(
    process_handle,
    PROCESS_NAME_WIN32,
    PWSTR(process_name.as_mut_ptr()),
    &mut length,
)
```

- Does:
  - Writes the full executable path of an opened process into a UTF-16 buffer.
- Parameters:
  - `hProcess`:
    - Process handle returned by `OpenProcess`.
  - `dwFlags`:
    - Selects the path format.
    - Project value: `PROCESS_NAME_WIN32`.
  - `lpExeName`:
    - Mutable UTF-16 output buffer.
    - Project value: `PWSTR(process_name.as_mut_ptr())`.
  - `lpdwSize`:
    - Input: output-buffer capacity in UTF-16 code units.
    - Output: number of code units written, excluding the null terminator.
- Returns:
  - `Ok(())`: path written.
  - `Err(error)`: query failed.
- Project use:
  - Gets the executable path used as the application map key.

### `PROCESS_NAME_WIN32`

- Type:
  - `QueryFullProcessImageNameW` flag.
- Means:
  - Requests a regular Win32 path.
  - Example: `C:\Program Files\App\app.exe`.
  - Avoids the native device-path format.

### `CloseHandle`

- Call: `CloseHandle(handle)`
- Does:
  - Releases an open Windows kernel-object handle.
- Parameters:
  - `hObject`:
    - Handle to close.
- Returns:
  - `Ok(())`: handle closed.
  - `Err(error)`: invalid handle or close failure.
- Project use:
  - Closes process handles after path queries.
  - Closes handles used only to check for the tracker mutex.
  - Closes the newly created mutex handle when its name already exists.
- Important:
  - A process ID is only a number and does not need closing.
  - A `HANDLE` is an OS resource and must be closed when no longer needed.

## 3. File Version APIs

### `GetFileVersionInfoSizeW`

- Call: `GetFileVersionInfoSizeW(&path, Some(&mut handle))`
- Does:
  - Calculates the buffer size required for an executable's version-information block.
- Parameters:
  - `lptstrFilename`:
    - UTF-16 executable path.
    - Project value: `HSTRING`.
  - `lpdwHandle`:
    - Ignored legacy output parameter.
    - Project value: `Some(&mut handle)`.
- Returns:
  - Positive `u32`: required size in bytes.
  - Zero: no version information or failure.
- Project use:
  - Allocates `vec![0u8; size as usize]`.

### `GetFileVersionInfoW`

- Call: `GetFileVersionInfoW(&path, 0, size, buffer.as_mut_ptr() as *mut c_void)`
- Does:
  - Copies an executable's version-information resource into a caller-owned buffer.
- Parameters:
  - `lptstrFilename`:
    - UTF-16 executable path.
  - `dwHandle`:
    - Ignored legacy parameter.
    - Project value: `0`.
  - `dwLen`:
    - Destination-buffer size in bytes.
    - Project value: result from `GetFileVersionInfoSizeW`.
  - `lpData`:
    - Raw mutable pointer to the destination byte buffer.
- Returns:
  - `Ok(())`: version block copied.
  - `Err(error)`: operation failed.
- Project use:
  - Supplies the data queried by `VerQueryValueW`.

### `VerQueryValueW`

- Calls:
  - `\VarFileInfo\Translation`
  - `\StringFileInfo\LLLLCCCC\ProductName`
  - `\StringFileInfo\LLLLCCCC\FileDescription`
- Does:
  - Returns a pointer to a selected value inside a version-information block.
- Parameters:
  - `pBlock`:
    - Pointer to the buffer filled by `GetFileVersionInfoW`.
  - `lpSubBlock`:
    - UTF-16 query path selecting the requested value.
  - `lplpBuffer`:
    - Output pointer receiving the selected data address.
    - Points inside `pBlock`; it does not own new memory.
  - `puLen`:
    - Output length of the selected data.
    - `Translation`: project treats it as bytes and divides by four for `(u16, u16)` pairs.
    - String value: project treats it as a UTF-16 code-unit count.
- Returns:
  - Nonzero `BOOL`: value found.
  - Zero `BOOL`: value unavailable or query failed.
- Project use:
  - Reads language and codepage pairs.
  - Reads `ProductName`.
  - Falls back to `FileDescription`.
- Important:
  - The version-information buffer must stay alive while returned pointers are used.
  - Returned pointers must be checked for null before dereferencing.

## 4. Mutex and Error APIs

### `CreateMutexW`

- Call: `CreateMutexW(None, false, &name)`
- Does:
  - Creates a named mutex object.
  - Opens the existing object when the same name already exists.
- Parameters:
  - `lpMutexAttributes`:
    - Optional security attributes.
    - Project value: `None`.
    - Result: default security and a non-inheritable handle.
  - `bInitialOwner`:
    - Controls whether the creating thread immediately owns the mutex.
    - Project value: `false`.
  - `lpName`:
    - Optional UTF-16 object name.
    - Project value: `Local\AppUsageTrackerMutex`.
- Returns:
  - `Ok(HANDLE)`: created or existing mutex opened.
  - `Err(error)`: object could not be created or opened.
- Project use:
  - Keeps one named mutex object alive for the tracker.
  - Uses object existence as the single-instance signal.

### `GetLastError`

- Call: `GetLastError()`
- Does:
  - Reads the calling thread's most recent Win32 error code.
- Parameters:
  - None.
- Returns:
  - `WIN32_ERROR`.
- Project use:
  - Called immediately after `CreateMutexW`.
  - Compared with `ERROR_ALREADY_EXISTS`.

### `ERROR_ALREADY_EXISTS`

- Type:
  - Win32 error-code constant.
- Means:
  - `CreateMutexW` returned a handle to an existing named mutex.
- Project use:
  - Indicates that another tracker instance is already running.

### `OpenMutexW`

- Call: `OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, &name)`
- Does:
  - Opens an existing named mutex.
- Parameters:
  - `dwDesiredAccess`:
    - Access rights requested for the mutex handle.
    - Project value: `SYNCHRONIZATION_SYNCHRONIZE`.
  - `bInheritHandle`:
    - Controls whether child processes inherit the handle.
    - Project value: `false`.
  - `lpName`:
    - UTF-16 name of the existing mutex.
- Returns:
  - `Ok(HANDLE)`: named mutex exists and was opened.
  - `Err(error)`: mutex does not exist or could not be opened.
- Project use:
  - Checks whether the tracker is running.
  - Immediately closes the check-only handle.

### `SYNCHRONIZATION_SYNCHRONIZE`

- Type:
  - Synchronization access-right constant.
- Means:
  - Requests permission to synchronize with the mutex object.
- Project use:
  - Supplies the access mask for `OpenMutexW`.

## 5. Detached Process Flag

### `DETACHED_PROCESS`

- Usage: `.creation_flags(DETACHED_PROCESS.0)`
- Does:
  - Passes the Windows `DETACHED_PROCESS` creation flag when spawning the child.
  - Prevents the child from inheriting the parent's console.
- Parameters:
  - `creation_flags(flags)`:
    - `flags` is a `u32` bitmask passed to Windows process creation.
    - `DETACHED_PROCESS.0` extracts the flag's numeric value.
- Project use:
  - Starts the internal `spawn-child` worker in the background.
- Related settings:
  - `stdin(Stdio::null())`
  - `stdout(Stdio::null())`
  - `stderr(Stdio::null())`
- Important:
  - `Command::spawn` and `creation_flags` are Rust standard-library interfaces.
  - `DETACHED_PROCESS` is the Windows process-creation flag they pass through.

## 6. Windows Types Used

### `HWND`

- Means:
  - Handle identifying a Windows window.
- Used by:
  - `EnumWindows`
  - `IsWindowVisible`
  - `GetWindowTextW`
  - `GetWindowThreadProcessId`

### `HANDLE`

- Means:
  - Generic handle to a Windows kernel object.
- Project objects:
  - Process handles.
  - Named mutex handles.
- Cleanup:
  - Close with `CloseHandle`.

### `BOOL`

- Means:
  - Win32 integer Boolean.
- Values:
  - Zero: false.
  - Nonzero: true.
- Rust conversion:
  - `.as_bool()`.

### `LPARAM`

- Means:
  - Pointer-sized signed value used to pass custom callback data.
- Project use:
  - Carries the application-map raw pointer through `EnumWindows`.

### `HSTRING`

- Means:
  - Owned Windows Runtime string containing UTF-16 text.
- Project use:
  - Executable paths.
  - Mutex names.
  - Version-information query paths.
- Construction:
  - `HSTRING::from(rust_string)`.

### `PWSTR`

- Means:
  - Mutable pointer to a null-terminated UTF-16 string buffer.
- Project use:
  - Wraps `process_name.as_mut_ptr()` for `QueryFullProcessImageNameW`.
- Important:
  - Does not allocate or own the buffer.
  - The Rust buffer must stay alive and have enough capacity.

## 7. Windows Registry Operations Through `winreg`

- Note:
  - These are `winreg` crate methods wrapping Windows Registry APIs.
  - They are not direct calls from the `windows` crate.

### `RegKey::predef(HKEY_CURRENT_USER)`

- Does:
  - Creates a wrapper around the predefined current-user registry hive.
- Parameters:
  - `HKEY_CURRENT_USER`:
    - Registry hive containing settings for the signed-in user.
- Project use:
  - Reads and changes startup registration without machine-wide HKLM access.

### `open_subkey(path)`

- Call: `hkcu.open_subkey("Software\Microsoft\Windows\CurrentVersion\Run")`
- Does:
  - Opens an existing registry key with default read access.
- Parameters:
  - `path`:
    - Registry path relative to `HKEY_CURRENT_USER`.
- Returns:
  - `Ok(RegKey)`: key opened.
  - `Err(error)`: key could not be opened.
- Project use:
  - Checks whether the startup value exists.

### `open_subkey_with_flags(path, flags)`

- Call: `hkcu.open_subkey_with_flags(RUN_KEY, KEY_WRITE)`
- Does:
  - Opens an existing registry key with explicit access rights.
- Parameters:
  - `path`:
    - Registry path relative to `HKEY_CURRENT_USER`.
  - `flags`:
    - Requested access mask.
    - Project value: `KEY_WRITE`.
- Returns:
  - `Ok(RegKey)`: key opened with requested access.
  - `Err(error)`: key could not be opened.
- Project use:
  - Opens the Run key before adding or deleting the startup value.

### `set_value(name, value)`

- Call: `startup_key.set_value("AppUsageTracker", &exe_path_string)`
- Does:
  - Creates or replaces a registry value.
- Parameters:
  - `name`:
    - Registry value name.
  - `value`:
    - Data stored in the value.
    - Project value: current executable path.
- Returns:
  - `Ok(())`: value written.
  - `Err(error)`: write failed.
- Project use:
  - Registers Usage Tracker to launch at sign-in.

### `get_value::<String, _>(name)`

- Call: `startup_key.get_value::<String, _>("AppUsageTracker")`
- Does:
  - Reads and converts a registry value into a Rust `String`.
- Parameters:
  - `name`:
    - Registry value name to read.
- Returns:
  - `Ok(String)`: value found and decoded.
  - `Err(error)`: value missing or invalid.
- Project use:
  - Determines whether startup is registered.

### `delete_value(name)`

- Call: `startup_key.delete_value("AppUsageTracker")`
- Does:
  - Deletes one registry value.
- Parameters:
  - `name`:
    - Registry value name to remove.
- Returns:
  - `Ok(())`: value deleted.
  - `Err(error)`: deletion failed.
- Project use:
  - Removes Usage Tracker from Windows startup.

### Registry constants

- `HKEY_CURRENT_USER`:
  - Per-user registry hive.
  - Avoids machine-wide startup registration.
- `KEY_WRITE`:
  - Access mask allowing registry values to be created, changed, or deleted.
- Run key:
  - `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
  - Windows executes values stored here when the current user signs in.
