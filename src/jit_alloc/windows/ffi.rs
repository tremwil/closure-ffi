#![allow(
    unused,
    non_camel_case_types,
    non_snake_case,
    clippy::upper_case_acronyms
)]

use core::ffi::c_void;

pub type BOOL = i32;
pub type WCHAR = u16;
pub type WORD = u16;
pub type DWORD = u32;
pub type SIZE_T = usize;
pub type DWORD_PTR = usize;

pub type LPVOID = *mut c_void;
pub type LPCVOID = *const c_void;
pub type LPCWSTR = *const WCHAR;

pub type HANDLE = *mut c_void;

pub const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;

pub const PAGE_EXECUTE_READWRITE: DWORD = 0x00000040;

pub const FILE_MAP_READ: DWORD = 0x00000004;
pub const FILE_MAP_WRITE: DWORD = 0x00000002;
pub const FILE_MAP_EXECUTE: DWORD = 0x00000020;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SYSTEM_INFO {
    pub wProcessorArchitecture: WORD,
    pub wReserved: WORD,
    pub dwPageSize: DWORD,
    pub lpMinimumApplicationAddress: LPVOID,
    pub lpMaximumApplicationAddress: LPVOID,
    pub dwActiveProcessorMask: DWORD_PTR,
    pub dwNumberOfProcessors: DWORD,
    pub dwProcessorType: DWORD,
    pub dwAllocationGranularity: DWORD,
    pub wProcessorLevel: WORD,
    pub wProcessorRevision: WORD,
}

pub type LPSYSTEM_INFO = *mut SYSTEM_INFO;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SECURITY_ATTRIBUTES {
    nLength: DWORD,
    lpSecurityDescriptor: LPVOID,
    bInheritHandle: BOOL,
}

pub type LPSECURITY_ATTRIBUTES = *mut SECURITY_ATTRIBUTES;

unsafe extern "system" {
    pub unsafe fn GetLastError() -> DWORD;

    pub unsafe fn GetSystemInfo(lpSystemInfo: LPSYSTEM_INFO);

    pub unsafe fn CreateFileMappingW(
        hFile: HANDLE,
        lpFileMappingAttributes: LPSECURITY_ATTRIBUTES,
        flProtect: DWORD,
        dwMaximumSizeHigh: DWORD,
        dwMaximumSizeLow: DWORD,
        lpName: LPCWSTR,
    ) -> HANDLE;

    pub unsafe fn MapViewOfFile(
        hFileMappingObject: HANDLE,
        dwDesiredAccess: DWORD,
        dwFileOffsetHigh: DWORD,
        dwFileOffsetLow: DWORD,
        dwNumberOfBytesToMap: SIZE_T,
    ) -> LPVOID;

    pub unsafe fn UnmapViewOfFile(lpBaseAddress: LPCVOID) -> BOOL;

    pub unsafe fn CloseHandle(hObject: HANDLE) -> BOOL;
}
