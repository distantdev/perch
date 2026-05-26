use std::ffi::c_void;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};

const PROCESS_BASIC_INFORMATION: u32 = 0;

#[repr(C)]
struct ProcessBasicInformation {
    _reserved1: *mut c_void,
    peb_base_address: *mut c_void,
    _reserved2: [*mut c_void; 2],
    _unique_process_id: usize,
    _reserved3: *mut c_void,
}

#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *mut u16,
}

const PEB_PROCESS_PARAMETERS_OFFSET: usize = 0x20;
const PROCESS_PARAMETERS_CURRENT_DIRECTORY_OFFSET: usize = 0x38;

pub fn read_process_cwd(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid).ok()?;

        let result = read_process_cwd_inner(handle);
        let _ = CloseHandle(handle);
        result
    }
}

unsafe fn read_process_cwd_inner(handle: HANDLE) -> Option<String> {
    let mut info = ProcessBasicInformation {
        _reserved1: std::ptr::null_mut(),
        peb_base_address: std::ptr::null_mut(),
        _reserved2: [std::ptr::null_mut(); 2],
        _unique_process_id: 0,
        _reserved3: std::ptr::null_mut(),
    };

    let status = NtQueryInformationProcess(
        handle,
        PROCESS_BASIC_INFORMATION,
        &mut info as *mut _ as *mut c_void,
        std::mem::size_of::<ProcessBasicInformation>() as u32,
        std::ptr::null_mut(),
    );
    if status != 0 || info.peb_base_address.is_null() {
        return None;
    }

    let peb = info.peb_base_address as usize;
    let mut process_parameters: usize = 0;
    read_usize(
        handle,
        peb + PEB_PROCESS_PARAMETERS_OFFSET,
        &mut process_parameters,
    )?;
    if process_parameters == 0 {
        return None;
    }

    let mut unicode = UnicodeString {
        length: 0,
        maximum_length: 0,
        buffer: std::ptr::null_mut(),
    };
    read_struct(
        handle,
        process_parameters + PROCESS_PARAMETERS_CURRENT_DIRECTORY_OFFSET,
        &mut unicode,
    )?;

    if unicode.length == 0 || unicode.buffer.is_null() {
        return None;
    }

    let wchar_count = unicode.length as usize / 2;
    let mut buffer = vec![0u16; wchar_count];
    ReadProcessMemory(
        handle,
        unicode.buffer as *const c_void,
        buffer.as_mut_ptr() as *mut c_void,
        unicode.length as usize,
        None,
    )
    .ok()?;

    let path = String::from_utf16(&buffer).ok()?;
    let path = path.trim_matches('\0').to_string();
    if path.is_empty() {
        return None;
    }

    Some(normalize_windows_path(&path))
}

fn normalize_windows_path(path: &str) -> String {
    let path = path.trim();
    let path = path
        .strip_prefix("\\\\?\\")
        .or_else(|| path.strip_prefix("\\\\?\\UNC\\"))
        .unwrap_or(path);
    std::path::Path::new(path).display().to_string()
}

unsafe fn read_usize(handle: HANDLE, address: usize, out: &mut usize) -> Option<()> {
    ReadProcessMemory(
        handle,
        address as *const c_void,
        out as *mut usize as *mut c_void,
        std::mem::size_of::<usize>(),
        None,
    )
    .ok()?;
    Some(())
}

unsafe fn read_struct<T>(handle: HANDLE, address: usize, out: &mut T) -> Option<()> {
    ReadProcessMemory(
        handle,
        address as *const c_void,
        out as *mut T as *mut c_void,
        std::mem::size_of::<T>(),
        None,
    )
    .ok()?;
    Some(())
}

#[link(name = "ntdll")]
extern "system" {
    fn NtQueryInformationProcess(
        process_handle: HANDLE,
        process_information_class: u32,
        process_information: *mut c_void,
        process_information_length: u32,
        return_length: *mut u32,
    ) -> i32;
}
