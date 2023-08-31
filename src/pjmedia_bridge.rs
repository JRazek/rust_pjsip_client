use crate::error::PjsuaError;
use crate::pjsua_call;

use super::error::{get_error_as_option, get_error_as_result};
use super::pjsua_memory_pool::PjsuaMemoryPool;
use std::ptr;

pub struct PjmediaPort<'a> {
    pjmedia_port: *mut pjsua::pjmedia_port,
    slot: u32,
    pjmedia_bridge: &'a PjmediaBridge<'a>,
}

impl<'a> PjmediaPort<'a> {
    fn new(
        pjmedia_bridge: &'a PjmediaBridge<'a>,
        pjsua_pool: &'a mut PjsuaMemoryPool,
        port: *mut pjsua::pjmedia_port,
    ) -> Option<PjmediaPort<'a>> {
        let mut slot = 0;

        let status = unsafe {
            pjsua::pjmedia_conf_add_port(
                pjmedia_bridge.pjmedia_bridge,
                pjsua_pool.as_mut(),
                port,
                ptr::null_mut(),
                &mut slot,
            )
        };

        get_error_as_option(status)?;

        Some(PjmediaPort {
            pjmedia_port: port,
            slot,
            pjmedia_bridge,
        })
    }
}

impl<'a> Drop for PjmediaPort<'a> {
    fn drop(&mut self) {
        todo!()
    }
}

pub struct PjmediaBridge<'a> {
    //stands for conference bridge
    pjmedia_bridge: *mut pjsua::pjmedia_conf,
    pjsua_pool: &'a mut PjsuaMemoryPool,
}

impl<'a> PjmediaBridge<'a> {
    pub fn new(
        pjsua_pool: &'a mut PjsuaMemoryPool,
        max_slots: usize,
        sampling_rate: usize,
        channel_count: usize,
        samples_per_frame: usize,
        bits_per_sample: usize,
    ) -> Option<PjmediaBridge<'a>> {
        let pjmedia_bridge = unsafe {
            let pjmedia_bridge;

            let options = pjsua::pjmedia_conf_option_PJMEDIA_CONF_NO_MIC
                | pjsua::pjmedia_conf_option_PJMEDIA_CONF_NO_DEVICE;

            let mut bridge = ptr::null_mut();

            let status = pjsua::pjmedia_conf_create(
                pjsua_pool.as_mut(),
                max_slots as u32,
                sampling_rate as u32,
                channel_count as u32,
                samples_per_frame as u32,
                bits_per_sample as u32,
                options,
                &mut bridge,
            );

            get_error_as_option(status)?;

            match bridge.is_null() {
                true => None,
                false => {
                    pjmedia_bridge = bridge;
                    Some(pjmedia_bridge)
                }
            }
        }?;

        Some(PjmediaBridge {
            pjmedia_bridge,
            pjsua_pool,
        })
    }

    pub fn connect(
        &mut self,
        mut sink: impl pjsua_call::PjsuaSinkMediaPort,
        mut stream: impl pjsua_call::PjsuaStreamMediaPort,
    ) -> Result<(), PjsuaError> {
        let sink_slot = PjmediaPort::new(self, self.pjsua_pool, sink.as_pjmedia_port());

//        let mut sink_slot_id = 0;
//        unsafe {
//            let status = pjsua::pjmedia_conf_add_port(
//                self.pjmedia_bridge,
//                self.pjsua_pool.as_mut(),
//                sink.as_pjmedia_port(),
//                ptr::null_mut(),
//                &mut sink_slot_id,
//            );
//
//            get_error_as_result(status)?;
//        }
//
//        let mut stream_slot_id = 0;
//
//        unsafe {
//            let status = pjsua::pjmedia_conf_add_port(
//                self.pjmedia_bridge,
//                self.pjsua_pool.as_mut(),
//                stream.as_pjmedia_port(),
//                ptr::null_mut(),
//                &mut stream_slot_id,
//            );
//
//            get_error_as_result(status)?;
//        }

        Ok(())
    }
}

impl<'a> Drop for PjmediaBridge<'a> {
    fn drop(&mut self) {
        unsafe {
            pjsua::pjmedia_conf_destroy(self.pjmedia_bridge);
        }
    }
}
