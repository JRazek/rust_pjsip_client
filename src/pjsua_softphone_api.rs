use std::ptr;
use std::sync::Mutex;

use crate::pjsua_config;

//singleton
pub struct PjsuaInstance;

impl PjsuaInstance {
    pub fn get_instance() -> Option<PjsuaInstance> {
        static INSTANCE: Mutex<Option<PjsuaInstance>> = Mutex::new(Some(PjsuaInstance));

        if let Ok(mut instance_guard) = INSTANCE.try_lock() {
            if let Some(instance) = instance_guard.take() {
                unsafe {
                    pjsua::pjsua_create();
                }

                return Some(instance);
            }
        }

        None
    }

    pub fn init(pjsua_config: pjsua_config::PjsuaConfig) {
        let mut pjsua_config = pjsua_config.into();

        unsafe {
            pjsua::pjsua_init(&mut pjsua_config, ptr::null(), ptr::null());
        }
    }
}

impl Drop for PjsuaInstance {
    fn drop(&mut self) {
        unsafe {
            pjsua::pjsua_destroy();
        }
    }
}
