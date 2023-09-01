use super::pjsua_call::PjsuaCall;
use super::pjsua_conf_bridge::ConfBrigdgeHandle;
use super::pjsua_memory_pool::PjsuaMemoryPool;
use crate::error::ffi_assert_res;
use crate::error::get_error_as_result;
use crate::ffi_assert;
use std::ptr;
use std::vec;

use thingbuf::mpsc::{channel as thingbuf_channel, Receiver as ThingbufReceiver};

use super::error::PjsuaError;

extern "C" fn pjmedia_mem_capture_eof_cb2(
    port: *mut pjsua::pjmedia_port,
    user_data: *mut ::std::os::raw::c_void,
) {
    use cb_data::EofCb2UserData;

    eprintln!("entering pjmedia_mem_capture_eof_cb2...");

    let user_data = user_data as *mut EofCb2UserData;
    ffi_assert!(!user_data.is_null());

    let buffer_in = unsafe { &(*user_data).buffer };
    let in_size = unsafe { pjsua::pjmedia_mem_capture_get_size(port) };

    let buffer_out = buffer_in
        .iter()
        .take(in_size)
        .cloned()
        .collect::<Box<[u8]>>();
    let channel_tx = unsafe { &mut (*user_data).tx_channel };

    let res = channel_tx.try_send(buffer_out);
    ffi_assert_res(res);

    eprintln!("returning from pjmedia_mem_capture_eof_cb2...");
}

#[derive(Debug)]
pub struct PjsuaSinkBufferMediaPort<'a> {
    //this holds the buffer
    media_port: *mut pjsua::pjmedia_port,
    user_data_ptr: *mut cb_data::EofCb2UserData,
    rx_channel: ThingbufReceiver<Box<[u8]>>,
    _pjsua_pool: &'a PjsuaMemoryPool,
}

pub struct PjsuaSinkBufferMediaPortAdded<'a> {
    port_id: pjsua::pjsua_conf_port_id,
    _media_port: PjsuaSinkBufferMediaPort<'a>,
    brigde: &'a ConfBrigdgeHandle,
}

impl<'a> PjsuaSinkBufferMediaPortAdded<'a> {
    pub(crate) fn new(
        media_port: PjsuaSinkBufferMediaPort<'a>,
        mem_pool: &PjsuaMemoryPool,
        bridge: &'a ConfBrigdgeHandle,
    ) -> Result<PjsuaSinkBufferMediaPortAdded<'a>, PjsuaError> {
        let mut port_id = pjsua::pjsua_conf_port_id::default();

        unsafe {
            pjsua::pjsua_conf_add_port(
                mem_pool.raw_handle(),
                media_port.raw_handle(),
                &mut port_id,
            );
        }
        Ok(PjsuaSinkBufferMediaPortAdded {
            port_id,
            _media_port: media_port,
            brigde: bridge,
        })
    }

    pub(crate) fn connect(
        self,
        call: &'a PjsuaCall,
    ) -> Result<PjsuaSinkBufferMediaPortConnected<'a>, PjsuaError> {
        PjsuaSinkBufferMediaPortConnected::new(self, call)
    }

    pub(crate) fn port_id(&self) -> pjsua::pjsua_conf_port_id {
        self.port_id
    }
}

impl<'a> Drop for PjsuaSinkBufferMediaPortAdded<'a> {
    fn drop(&mut self) {
        unsafe {
            pjsua::pjsua_conf_remove_port(self.port_id);
        }
    }
}

pub struct PjsuaSinkBufferMediaPortConnected<'a> {
    added_media_port: PjsuaSinkBufferMediaPortAdded<'a>,
    call: &'a PjsuaCall<'a>,
}

impl<'a> PjsuaSinkBufferMediaPortConnected<'a> {
    pub(crate) fn new(
        added_media_port: PjsuaSinkBufferMediaPortAdded<'a>,
        call: &'a PjsuaCall,
    ) -> Result<PjsuaSinkBufferMediaPortConnected<'a>, PjsuaError> {
        unsafe {
            let status =
                pjsua::pjsua_conf_connect(call.get_conf_port_id(), added_media_port.port_id());

            get_error_as_result(status)?;
        }

        Ok(PjsuaSinkBufferMediaPortConnected {
            call,
            added_media_port,
        })
    }
}

impl<'a> Drop for PjsuaSinkBufferMediaPortConnected<'a> {
    fn drop(&mut self) {
        unsafe {
            pjsua::pjsua_conf_disconnect(
                self.call.get_conf_port_id(),
                self.added_media_port.port_id(),
            );
        }
    }
}

fn static_size_buffer(buffer_size: usize) -> Box<[u8]> {
    let buffer = vec![0u8; buffer_size];

    buffer.into_boxed_slice()
}

impl<'a> PjsuaSinkBufferMediaPort<'a> {
    pub fn new(
        buffer_size: usize,
        sample_rate: usize,
        channels_count: usize,
        samples_per_frame: usize,
        pjsua_pool: &'a PjsuaMemoryPool,
    ) -> Result<PjsuaSinkBufferMediaPort<'a>, PjsuaError> {
        let mut buffer: Box<[u8]> = static_size_buffer(buffer_size);

        let media_port = unsafe {
            let mut media_port = ptr::null_mut();

            let status = pjsua::pjmedia_mem_capture_create(
                pjsua_pool.raw_handle(),
                buffer.as_mut_ptr() as *mut _,
                buffer.len(),
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

        let (tx_channel, rx_channel) = thingbuf_channel(100);
        let mut user_data = Box::new(cb_data::EofCb2UserData { buffer, tx_channel });

        let _ = unsafe {
            let status = pjsua::pjmedia_mem_capture_set_eof_cb2(
                media_port,
                user_data.as_mut() as *mut _ as *mut _,
                Some(pjmedia_mem_capture_eof_cb2),
            );

            get_error_as_result(status)?
        };

        Ok(PjsuaSinkBufferMediaPort {
            media_port,
            user_data_ptr: Box::into_raw(user_data),
            rx_channel,
            _pjsua_pool: pjsua_pool,
        })
    }

    pub(crate) fn raw_handle(&self) -> *mut pjsua::pjmedia_port {
        self.media_port
    }
}

impl<'a> Drop for PjsuaSinkBufferMediaPort<'a> {
    fn drop(&mut self) {
        ffi_assert!(!self.media_port.is_null());

        let status = unsafe { pjsua::pjmedia_port_destroy(self.media_port) };
        let status = get_error_as_result(status);

        ffi_assert!(status.is_ok());

        ffi_assert!(!self.user_data_ptr.is_null());

        let _ = unsafe { Box::from_raw(self.user_data_ptr) };
    }
}

pub(super) mod cb_data {
    use thingbuf::mpsc::Sender as ThingbufSender;

    pub(super) struct EofCb2UserData {
        pub buffer: Box<[u8]>,
        pub tx_channel: ThingbufSender<Box<[u8]>>,
    }
}
