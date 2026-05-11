#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = espejismo_core::mux::native::validate_frame_bytes_for_fuzz(data);
});
