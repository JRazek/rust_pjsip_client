use super::error::PjsuaError;
use super::pjsua_conf_bridge::ConfBrigdgeHandle;
use super::pjsua_memory_pool::PjsuaMemoryPool;
use crate::error::get_error_as_result;
use crate::pjsua_call::PjsuaCallSetup;

use super::pjsua_conf_bridge::*;

pub struct CustomMediaPort;

impl<'a> SinkMediaPort<'a, CustomMediaPortConnected<'a>, CustomMediaPortAdded<'a>>
    for CustomMediaPort
{
    fn add(
        self,
        mem_pool: &'a PjsuaMemoryPool,
        conf_bridge: &'a ConfBrigdgeHandle,
    ) -> Result<CustomMediaPortAdded<'a>, PjsuaError> {
        CustomMediaPortAdded::new(mem_pool, conf_bridge)
    }
}

pub struct CustomMediaPortAdded<'a> {
    conf_bridge: &'a ConfBrigdgeHandle,
}

impl<'a> SinkMediaPortAdded<'a, CustomMediaPortConnected<'a>> for CustomMediaPortAdded<'a> {}

unsafe extern "C" fn custom_port_put_frame(
    port: *mut pjsua::pjmedia_port,
    frame: *mut pjsua::pjmedia_frame,
) -> pjsua::pj_status_t {
    let custom_port = unsafe { &mut *(port as *mut CustomMediaPortAdded) };

    // Access the frame data and timestamp, then print
    let frame_data =
        unsafe { std::slice::from_raw_parts((*frame).buf as *const u8, (*frame).size as usize) };

    return 0; // or appropriate status
}

impl<'a> CustomMediaPortAdded<'a> {
    pub(crate) fn new(
        mem_pool: &'a PjsuaMemoryPool,
        conf_bridge: &'a ConfBrigdgeHandle,
    ) -> Result<Self, PjsuaError> {
        let mut base: Box<pjsua::pjmedia_port> = Box::new(unsafe { std::mem::zeroed() });
        let mut port_slot = pjsua::pjsua_conf_port_id::default();

        unsafe {
            pjsua::pjsua_conf_add_port(mem_pool.raw_handle(), base.as_mut(), &mut port_slot);
        }

        // Set the function pointers
        base.put_frame = Some(custom_port_put_frame);

        // Set other necessary fields of base...

        Ok(CustomMediaPortAdded { base, conf_bridge })
    }

    pub(crate) fn connect(pjsua_call: &PjsuaCallSetup) -> Result<Self, PjsuaError> {
        pjsua_call.connect_with_sink_media_port();
        todo!()
    }
}

impl<'a> Drop for CustomMediaPortAdded<'a> {
    fn drop(&mut self) {
        let status = unsafe { pjsua::pjmedia_port_destroy(self.base.as_mut()) };
        get_error_as_result(status).unwrap();
    }
}

pub struct CustomMediaPortConnected<'a> {
    added_media_port: CustomMediaPortAdded<'a>,
}
