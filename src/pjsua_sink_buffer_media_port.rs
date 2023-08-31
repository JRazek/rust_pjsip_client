use super::pjsua_memory_pool::PjsuaMemoryPool;
use crate::ffi_assert;
use std::ptr;

use super::error::PjsuaError;

use super::error::{get_error_as_option, get_error_as_result};

extern "C" fn pjmedia_mem_capture_set_eof_cb2(
    _port: *mut pjsua::pjmedia_port,
    _user_data: *mut ::std::os::raw::c_void,
) {
}

#[derive(Debug)]
pub struct PjsuaSinkBufferMediaPort<'a> {
    //this holds the buffer
    media_port: *mut pjsua::pjmedia_port,
    buffer_ptr: *mut Vec<u8>,
    _pjsua_pool: &'a PjsuaMemoryPool,
}

impl<'a> PjsuaSinkBufferMediaPort<'a> {
    pub fn new(
        buffer_size: usize,
        sample_rate: usize,
        channels_count: usize,
        samples_per_frame: usize,
        pjsua_pool: &'a mut PjsuaMemoryPool,
    ) -> Result<PjsuaSinkBufferMediaPort<'a>, PjsuaError> {
        let mut buffer: Box<Vec<u8>> = Box::new(Vec::with_capacity(buffer_size));

        let media_port = unsafe {
            let buffer_raw_bytes = buffer.as_mut_ptr();

            let mut media_port = ptr::null_mut();

            let status = pjsua::pjmedia_mem_capture_create(
                pjsua_pool.as_mut(),
                buffer_raw_bytes as *mut _,
                buffer_size as u64,
                sample_rate as u32,
                channels_count as u32,
                samples_per_frame as u32,
                16 as u32,
                0,
                &mut media_port,
            );

            get_error_as_result(status)?;

            media_port
        };

        ffi_assert!(!media_port.is_null());

        let buffer_ptr = Box::into_raw(buffer);

        let _ = unsafe {
            let status = pjsua::pjmedia_mem_capture_set_eof_cb2(
                media_port,
                buffer_ptr as *mut _,
                Some(pjmedia_mem_capture_set_eof_cb2),
            );

            get_error_as_result(status)?
        };

        Ok(PjsuaSinkBufferMediaPort {
            media_port,
            buffer_ptr,
            _pjsua_pool: pjsua_pool,
        })
    }
}

impl<'a> Drop for PjsuaSinkBufferMediaPort<'a> {
    fn drop(&mut self) {
        ffi_assert!(!self.media_port.is_null());

        let status = unsafe { pjsua::pjmedia_port_destroy(self.media_port) };
        let status = get_error_as_result(status);

        ffi_assert!(status.is_ok());

        ffi_assert!(!self.buffer_ptr.is_null());

        let _ = unsafe { Box::from_raw(self.buffer_ptr) };
    }
}

use super::pjsua_call::PjsuaSinkMediaPort;

impl<'a> PjsuaSinkMediaPort for PjsuaSinkBufferMediaPort<'a> {
    fn as_pjmedia_port(&mut self) -> *mut pjsua::pjmedia_port {
        self.media_port as *mut _
    }
}
