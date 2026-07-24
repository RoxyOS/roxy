unsafe extern "C" {
    static mut __bss_start: u8;
    static mut __bss_end: u8;
}

pub(crate) fn clear_bss() {
    // SAFETY: The linker defines this writable, non-overlapping BSS range and
    // entry executes before any BSS-backed Rust state is read.
    unsafe {
        let start = core::ptr::addr_of_mut!(__bss_start);
        let length = core::ptr::addr_of_mut!(__bss_end)
            .offset_from(start)
            .cast_unsigned();
        core::ptr::write_bytes(start, 0, length);
    }
}
