pub mod pjmedia_port_audio_sink;
pub mod pjmedia_port_audio_stream;
pub(super) mod pjmedia_api;


pub(super) fn next_num() -> u32 {
    use std::sync::atomic::AtomicU32;
    static mut COUNTER: AtomicU32 = AtomicU32::new(0);

    unsafe { COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst) }
}

