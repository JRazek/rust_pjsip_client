#![allow(dead_code)]

use super::error::PjsuaError;
use super::pjsua_memory_pool::PjsuaMemoryPool;
use super::pjsua_softphone_api;
use std::marker::PhantomData;
use std::sync::Mutex;

use std::rc::Rc;

pub(crate) struct ConfBrigdgeHandle {
    _not_send_sync: std::marker::PhantomData<*const ()>,
    _private: (),
    _pjsua_instance_handle: Rc<pjsua_softphone_api::PjsuaInstanceHandle>,
}

pub trait SinkMediaPort<'a, C: SinkMediaPortConnected + 'a, A: SinkMediaPortAdded<'a, C> + 'a> {
    fn add(
        self,
        mem_pool: &'a PjsuaMemoryPool,
        conf_bridge_handle: &'a ConfBrigdgeHandle,
    ) -> Result<A, PjsuaError>;
}

pub trait SinkMediaPortAdded<'a, C: SinkMediaPortConnected + 'a>:
    AsMut<pjsua::pjmedia_port>
{
    fn connect_call(
        &mut self,
        mem_pool: &PjsuaMemoryPool,
        conf_bridge_handle: &ConfBrigdgeHandle,
    ) -> Result<C, PjsuaError>;
}

pub trait SinkMediaPortConnected: AsMut<pjsua::pjmedia_port> {}

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

    //    pub fn add_sink<'a, A: SinkMediaPortAdded + 'a, S: SinkMediaPort<'a, A> + 'a>(
    //        &'a self,
    //        pjsua_sink_buffer_media_port: S,
    //        mem_pool: &PjsuaMemoryPool,
    //    ) -> Result<A, PjsuaError> {
    //        let added_port = pjsua_sink_buffer_media_port.add(mem_pool, self)?;
    //
    //        Ok(added_port)
    //    }
}
