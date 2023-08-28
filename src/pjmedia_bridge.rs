use super::error::{get_error_as_option, get_error_as_result};
use super::pjsua_memory_pool::PjsuaMemoryPool;
use std::ptr;

pub struct PjmediaBridge<'a> {
    //stands for conference bridge
    pjmedia_bridge: *mut pjsua::pjmedia_conf,
    _pjsua_pool: &'a mut PjsuaMemoryPool,
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
            let mut pjmedia_bridge = ptr::null_mut();

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
            _pjsua_pool: pjsua_pool,
        })
    }
}

impl<'a> Drop for PjmediaBridge<'a> {
    fn drop(&mut self) {
        unsafe {
            pjsua::pjmedia_conf_destroy(self.pjmedia_bridge);
        }
    }
}
