#![allow(dead_code)]

use super::error::PjsuaError;
use super::pjsua_memory_pool::PjsuaMemoryPool;
use super::pjsua_sink_buffer_media_port::{
    PjsuaSinkBufferMediaPort, PjsuaSinkBufferMediaPortAdded,
};
use super::pjsua_softphone_api;
use std::marker::PhantomData;
use std::sync::Mutex;

use std::rc::Rc;

pub(crate) struct ConfBrigdgeHandle {
    _not_send_sync: std::marker::PhantomData<*const ()>,
    _private: (),
    _pjsua_instance_handle: Rc<pjsua_softphone_api::PjsuaInstanceHandle>,
}

impl ConfBrigdgeHandle {
    pub fn get_instance(
        pjsua_instance_handle: Rc<pjsua_softphone_api::PjsuaInstanceHandle>,
    ) -> Option<ConfBrigdgeHandle> {
        static INSTANCE_CRATED: Mutex<bool> = Mutex::new(false);

        if let Ok(mut instance_guard) = INSTANCE_CRATED.try_lock() {
            let val = *instance_guard;
            return match val {
                false => {
                    *instance_guard = true;
                    Some(ConfBrigdgeHandle {
                        _private: (),
                        _not_send_sync: PhantomData,
                        _pjsua_instance_handle: pjsua_instance_handle,
                    })
                }
                true => None,
            };
        }

        None
    }

    pub fn add_sink<'a>(
        &'a self,
        pjsua_sink_buffer_media_port: PjsuaSinkBufferMediaPort<'a>,
        mem_pool: &PjsuaMemoryPool,
    ) -> Result<PjsuaSinkBufferMediaPortAdded<'a>, PjsuaError> {
        let added_port =
            PjsuaSinkBufferMediaPortAdded::new(pjsua_sink_buffer_media_port, mem_pool, self);

        added_port
    }
}
