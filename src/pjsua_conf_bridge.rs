#![allow(dead_code)]

use super::error::PjsuaError;
use super::pjsua_memory_pool::PjsuaMemoryPool;
use super::pjsua_softphone_api;
use std::marker::PhantomData;
use std::sync::Mutex;

use super::pjmedia_port_audio_sink::*;

use std::rc::Rc;

pub(crate) struct ConfBridgeHandle {
    _not_send_sync: std::marker::PhantomData<*const ()>,
    _private: (),
    _pjsua_instance_handle: Rc<pjsua_softphone_api::PjsuaInstanceHandle>,
}

pub trait SinkMediaPortConnected: AsMut<pjsua::pjmedia_port> {}

use super::pjsua_call::{PjsuaCallHandle, PjsuaCallSetup};

impl ConfBridgeHandle {
    pub fn get_instance(
        pjsua_instance_handle: Rc<pjsua_softphone_api::PjsuaInstanceHandle>,
    ) -> Option<ConfBridgeHandle> {
        static INSTANCE_CRATED: Mutex<bool> = Mutex::new(false);

        if let Ok(mut instance_guard) = INSTANCE_CRATED.try_lock() {
            let val = *instance_guard;
            return match val {
                false => {
                    *instance_guard = true;
                    Some(ConfBridgeHandle {
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

    //currently hard coded, later to be used with trait
    pub(crate) async fn setup_sink_media<'a>(
        &'a self,
        custom_media_port: CustomSinkMediaPort<'a>,
        pjsua_call: &PjsuaCallHandle<'a>,
        mem_pool: &'a PjsuaMemoryPool,
    ) -> Result<CustomSinkMediaPortAdded<'a>, PjsuaError> {
        let added_port = custom_media_port.add(mem_pool, self)?;

        Ok(added_port)
    }
}
