#![no_std]
#![feature(alloc_error_handler)]

extern crate alloc;

use alloc::format;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[panic_handler]
#[no_mangle]
pub unsafe fn panic(_info: &::core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable();
}

#[alloc_error_handler]
#[no_mangle]
pub unsafe fn on_alloc_error(_: core::alloc::Layout) -> ! {
    core::arch::wasm32::unreachable();
}

extern "C" {
    fn input(register_id: u64);
    fn register_len(register_id: u64) -> u64;
    fn read_register(register_id: u64, ptr: u64);
    fn log_utf8(len: u64, ptr: u64);
}

#[no_mangle]
pub unsafe extern "C" fn cpu_ram_soak_test() {
    let mut buf = [0u8; 100 * 1024];
    let len = buf.len();
    let loop_limit = read_input() as usize;
    let mut counter = 0;
    for i in 0..loop_limit {
        let j = (i * 7 + len / 2) % len;
        let k = (i * 3) % len;
        let tmp = buf[k];
        buf[k] = buf[j];
        buf[j] = tmp;
        counter += 1;
    }
    let msg = format!("Done {} iterations!", counter);
    log_utf8(msg.len() as u64, msg.as_ptr() as u64);
}

unsafe fn read_input() -> u32 {
    const REGISTER_ID: u64 = 1;
    input(REGISTER_ID);
    let input_len = register_len(REGISTER_ID);
    assert_eq!(input_len, 4);
    let buf = [0u8; 4];
    read_register(REGISTER_ID, buf.as_ptr() as u64);
    u32::from_le_bytes(buf)
}
