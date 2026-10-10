#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let parsed = espejismo_core::mux::native::validate_frame_bytes_for_fuzz(data);

    if let Ok(Some((kind, stream_id, payload_len))) = parsed {
        // A valid frame must remain valid when trailing bytes are appended, and its
        // declared payload must be incomplete at every shorter frame boundary.
        let frame_end = 9 + payload_len;
        let exact_frame = &data[..frame_end];
        assert_eq!(
            espejismo_core::mux::native::validate_frame_bytes_for_fuzz(exact_frame).unwrap(),
            Some((kind, stream_id, payload_len))
        );
        let mut with_trailing_bytes = exact_frame.to_vec();
        with_trailing_bytes.extend_from_slice(&data[frame_end..]);
        assert_eq!(
            espejismo_core::mux::native::validate_frame_bytes_for_fuzz(&with_trailing_bytes)
                .unwrap(),
            Some((kind, stream_id, payload_len))
        );
        // Exercise every partial header length as well as a truncated payload.
        // This makes mutations around individual header fields observable to
        // libFuzzer instead of only checking the empty and almost-complete cases.
        for end in 0..9 {
            assert_eq!(
                espejismo_core::mux::native::validate_frame_bytes_for_fuzz(&data[..end]).unwrap(),
                None
            );
        }
        assert_eq!(
            espejismo_core::mux::native::validate_frame_bytes_for_fuzz(
                &data[..frame_end.saturating_sub(1)]
            )
            .unwrap(),
            None
        );
    }
});
