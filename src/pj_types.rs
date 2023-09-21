use crate::error::PjsuaError;
use crate::pjmedia_port_audio_sink::sample_duration;

use super::error::get_error_as_result;
use super::pjsua_memory_pool::{PjsuaMemoryPool, PoolBuffer};

pub struct PjString<'a> {
    pj_str: pjsua::pj_str_t,
    _pool_buffer: PoolBuffer<'a, u8>,
}

impl<'a> PjString<'a> {
    pub fn alloc(string: &str, mem_pool: &'a PjsuaMemoryPool) -> PjString<'a> {
        let mut pool_buffer = mem_pool.alloc::<u8>(string.len() + 1);

        let pj_str = pjsua::pj_str_t {
            ptr: pool_buffer.as_mut() as *mut _ as *mut std::os::raw::c_char,
            slen: pool_buffer.len() as pjsua::pj_ssize_t,
        };

        unsafe { std::ptr::copy(string.as_ptr(), pj_str.ptr as *mut u8, string.len()) };

        PjString {
            pj_str,
            _pool_buffer: pool_buffer,
        }
    }
}

impl<'a> AsRef<pjsua::pj_str_t> for PjString<'a> {
    fn as_ref(&self) -> &pjsua::pj_str_t {
        &self.pj_str
    }
}

impl<'a> AsMut<pjsua::pj_str_t> for PjString<'a> {
    fn as_mut(&mut self) -> &mut pjsua::pj_str_t {
        &mut self.pj_str
    }
}

pub struct Frame {
    pub data: Box<[u8]>,
    pub time: std::time::Duration,
}

impl Frame {
    pub(crate) fn new(
        frame_raw: &pjsua::pjmedia_frame,
        sample_rate: u32,
        channels_count: usize,
    ) -> Result<Self, PjsuaError> {
        type SampleType = u8;

        let buffer_size = frame_raw.size / std::mem::size_of::<SampleType>() as usize;

        let frame_data =
            unsafe { std::slice::from_raw_parts(frame_raw.buf as *const SampleType, buffer_size) };
        let frame_data = Box::from_iter(frame_data.iter().cloned());

        let timestamp0: pjsua::pj_timestamp = unsafe { std::mem::zeroed() };

        let samples_elapsed = unsafe { get_samples_diff(timestamp0, frame_raw.timestamp) };

        Ok(Frame {
            data: frame_data,
            time: samples_elapsed * sample_duration(sample_rate, channels_count),
        })
    }
}

pub(crate) unsafe fn get_samples_diff(
    timestamp1: pjsua::pj_timestamp,
    timestamp2: pjsua::pj_timestamp,
) -> u32 {
    let samples_diff = pjsua::pj_elapsed_nanosec(&timestamp1, &timestamp2);

    samples_diff
}
