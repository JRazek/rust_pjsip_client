use crate::error::PjsuaError;
use crate::pjsua_call;

use super::error::{get_error_as_option, get_error_as_result};
use super::pjsua_memory_pool::PjsuaMemoryPool;
use std::ptr;

struct PjmediaSlot {
    _pjmedia_port: *mut pjsua::pjmedia_port,
    slot: u32,
}

struct PjmediaSlotLink {
    pjmedia_slot_sink: PjmediaSlot,
    pjmedia_slot_stream: PjmediaSlot,
    bridge_handle: *mut pjsua::pjmedia_conf,
}

impl PjmediaSlotLink {
    fn new(
        pjmedia_slot_sink: PjmediaSlot,
        pjmedia_slot_stream: PjmediaSlot,
        pjmedia_bridge: &PjmediaBridge,
    ) -> Result<PjmediaSlotLink, PjsuaError> {
        unsafe {
            let status = pjsua::pjmedia_conf_connect_port(
                pjmedia_bridge.pjmedia_bridge,
                pjmedia_slot_sink.slot,
                pjmedia_slot_stream.slot,
                0,
            );

            get_error_as_result(status)?;
        }

        Ok(Self {
            pjmedia_slot_sink,
            pjmedia_slot_stream,
            bridge_handle: pjmedia_bridge.pjmedia_bridge,
        })
    }
}

impl Drop for PjmediaSlotLink {
    fn drop(&mut self) {
        unsafe {
            pjsua::pjmedia_conf_disconnect_port(
                self.bridge_handle,
                self.pjmedia_slot_sink.slot,
                self.pjmedia_slot_stream.slot,
            );
        }
    }
}

impl PjmediaSlot {
    fn new<'a>(
        pjmedia_bridge: &'a PjmediaBridge<'a>,
        pjsua_pool: &'a PjsuaMemoryPool,
        port: *mut pjsua::pjmedia_port,
    ) -> Result<PjmediaSlot, PjsuaError> {
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

        get_error_as_result(status)?;

        Ok(PjmediaSlot {
            _pjmedia_port: port,
            slot,
        })
    }
}

impl Drop for PjmediaSlot {
    fn drop(&mut self) {
        todo!()
    }
}

pub struct PjmediaBridge<'a> {
    //stands for conference bridge
    pjmedia_bridge: *mut pjsua::pjmedia_conf,
    pjsua_pool: &'a mut PjsuaMemoryPool,

    connections: Vec<PjmediaSlotLink>,
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
            connections: Vec::new(),
        })
    }

    pub fn connect(
        &mut self,
        mut sink: impl pjsua_call::PjsuaSinkMediaPort,
        mut stream: impl pjsua_call::PjsuaStreamMediaPort,
    ) -> Result<(), PjsuaError> {
        let sink_slot = PjmediaSlot::new(self, self.pjsua_pool, sink)?;
        let stream_slot = PjmediaSlot::new(self, self.pjsua_pool, stream)?;

        let link = PjmediaSlotLink::new(sink_slot, stream_slot, self)?;

        self.connections.push(link);

        Ok(())
    }
}

impl<'a> Drop for PjmediaBridge<'a> {
    fn drop(&mut self) {
        unsafe {
            self.connections.clear();
            pjsua::pjmedia_conf_destroy(self.pjmedia_bridge);
        }
    }
}
