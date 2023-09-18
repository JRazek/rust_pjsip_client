use crate::error::PjsuaError;

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

impl TryFrom<&pjsua::pjmedia_frame> for Frame {
    type Error = PjsuaError;

    fn try_from(frame_raw: &pjsua::pjmedia_frame) -> Result<Self, Self::Error> {
        let frame_data = unsafe {
            std::slice::from_raw_parts(frame_raw.buf as *const u8, frame_raw.size as usize)
        };

        let frame_data = Box::from_iter(frame_data.iter().cloned());
        let time = pj_timestamp_to_duration(frame_raw.timestamp)?;

        Ok(Frame {
            data: frame_data,
            time,
        })
    }
}

pub(crate) fn pj_timestamp_to_duration(
    timestamp: pjsua::pj_timestamp,
) -> Result<std::time::Duration, PjsuaError> {
    assert!(pjsua::PJ_HAS_INT64 != 0);

    let value = unsafe { timestamp.u64_ as u64 };

    let freq = unsafe {
        let mut freq: pjsua::pj_timestamp = std::mem::zeroed();
        get_error_as_result(pjsua::pj_get_timestamp_freq(&mut freq))?;

        freq.u64_ as u64
    };

    let duration = match freq {
        1000 => std::time::Duration::from_millis(value),
        1000000 => std::time::Duration::from_micros(value),
        1000000000 => std::time::Duration::from_nanos(value),
        _ => {
            return Err(PjsuaError {
                code: -1,
                message: "Unknown frequency in pj_timestamp!".to_string(),
            })
        }
    };

    Ok(duration)
}
