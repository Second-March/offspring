//! Small helpers for bridging Rust strings into the COM ABI. All strings
//! in `IExplorerCommand` crossed as `PWSTR` are allocated with `CoTaskMemAlloc`
//! and freed by the caller (Explorer). The helper below centralises that.

use windows::core::*;
use windows::Win32::Foundation::E_OUTOFMEMORY;
use windows::Win32::System::Com::CoTaskMemAlloc;

/// Allocate a null-terminated UTF-16 copy of `s` on the COM task heap
/// and hand the pointer back as a `PWSTR`. Ownership transfers to
/// whichever COM caller received it.
///
/// Returns `Err(E_OUTOFMEMORY)` when the allocation fails. This used to
/// return a bare `PWSTR::null()`, and every caller wrapped the result in
/// `Ok(...)` — so Explorer was handed a null string alongside `S_OK` and
/// dereferenced it. Returning a `Result` makes the failure impossible to
/// paper over at the call site.
pub fn cotaskmem_wstr(s: &str) -> Result<PWSTR> {
    let mut wide: Vec<u16> = s.encode_utf16().collect();
    wide.push(0);
    let bytes = wide.len() * std::mem::size_of::<u16>();
    unsafe {
        let buf = CoTaskMemAlloc(bytes) as *mut u16;
        if buf.is_null() {
            return Err(E_OUTOFMEMORY.into());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), buf, wide.len());
        Ok(PWSTR(buf))
    }
}
