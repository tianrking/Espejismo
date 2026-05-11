#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = espejismo_core::ingress::socks5::parse_udp_packet(data);
});
