use super::error::PjsuaError;
use std::sync::Mutex;
use std::{marker::PhantomData, ptr};

use super::error::get_error_as_result;
use super::pjsua_call::PjsuaIncomingCall;
use super::pjsua_conf_bridge::ConfBrigdgeHandle;
use std::rc::Rc;

use crate::{pjsua_account_config, pjsua_config, transport};

pub(crate) struct PjsuaInstanceHandle {
    _not_send_sync: PhantomData<*const ()>,
    _private: (),
}

impl Drop for PjsuaInstanceHandle {
    fn drop(&mut self) {
        unsafe {
            eprintln!("Dropping PjsuaInstanceHandle");
            pjsua::pjsua_destroy();
        }
    }
}

impl PjsuaInstanceHandle {
    pub fn get_instance() -> Option<PjsuaInstanceHandle> {
        static INSTANCE_CRATED: Mutex<bool> = Mutex::new(false);

        if let Ok(mut instance_guard) = INSTANCE_CRATED.try_lock() {
            let val = *instance_guard;
            return match val {
                false => {
                    *instance_guard = true;
                    unsafe {
                        pjsua::pjsua_create();
                    }
                    Some(PjsuaInstanceHandle {
                        _private: (),
                        _not_send_sync: PhantomData,
                    })
                }
                true => None,
            };
        }

        None
    }
}

pub struct PjsuaInstanceUninit {
    handle: PjsuaInstanceHandle,
}

//keep in mind the order of fields.
//The order of fields is important for the drop order.
//PjsuaInstanceInit MUST be dropped as the last, as it uninitializes pjsua completely.
pub struct PjsuaInstanceInit {
    //    accounts: Vec<Arc<pjsua_account_config::AccountConfigAdded>>,
    //    account_incoming_calls_rxs: Vec<mpsc::Receiver<OnIncomingCallSendData>>,
    log_config: pjsua_config::LogConfig,
    media_config: pjsua_config::MediaConfig,
    pjsua_config: pjsua_config::PjsuaConfig,
    handle: PjsuaInstanceHandle,
}

pub struct PjsuaInstanceInitTransportConfigured {
    pjsua_instance_init: PjsuaInstanceInit,
    transport: transport::PjsuaTransport,
}

pub struct PjsuaInstanceStarted {
    _log_config: pjsua_config::LogConfig,
    _pjsua_config: pjsua_config::PjsuaConfig,
    _transport: transport::PjsuaTransport,
    _handle: Rc<PjsuaInstanceHandle>,
    bridge: ConfBrigdgeHandle,
}

impl PjsuaInstanceInitTransportConfigured {
    pub fn start(self) -> PjsuaInstanceStarted {
        unsafe {
            pjsua::pjsua_start();
        }

        let handle = Rc::new(self.pjsua_instance_init.handle);
        let bridge = ConfBrigdgeHandle::get_instance(handle.clone()).unwrap();

        PjsuaInstanceStarted {
            _log_config: self.pjsua_instance_init.log_config,
            _pjsua_config: self.pjsua_instance_init.pjsua_config,
            _transport: self.transport,
            _handle: handle,
            bridge,
        }
    }
}

impl PjsuaInstanceUninit {
    pub fn get_instance() -> Option<PjsuaInstanceUninit> {
        PjsuaInstanceHandle::get_instance().map(|handle| PjsuaInstanceUninit { handle })
    }

    pub fn init(
        self,
        pjsua_config: pjsua_config::PjsuaConfig,
    ) -> Result<PjsuaInstanceInit, PjsuaError> {
        let log_config = pjsua_config::LogConfig::default();
        let media_config = pjsua_config::MediaConfig::default();

        let instance_init = PjsuaInstanceInit::from(self, pjsua_config, log_config, media_config);

        instance_init
    }
}

impl PjsuaInstanceInit {
    pub fn set_transport(
        self,
        mut transport: transport::PjsuaTransport,
    ) -> PjsuaInstanceInitTransportConfigured {
        unsafe {
            let mut transport_id: pjsua::pjsua_transport_id = 0;

            pjsua::pjsua_transport_create(
                pjsua::pjsip_transport_type_e_PJSIP_TRANSPORT_UDP,
                transport.as_mut(),
                &mut transport_id,
            );

            let instance_transport_set = PjsuaInstanceInitTransportConfigured {
                pjsua_instance_init: self,
                transport,
            };

            instance_transport_set
        }
    }
}

impl PjsuaInstanceStarted {
    pub async fn add_account(
        &self,
        account: pjsua_account_config::AccountConfig,
    ) -> Result<pjsua_account_config::AccountConfigAdded, PjsuaError> {
        let account_added = account.add_to_instance_init(&self);

        //note, this does not use FFI

        account_added
    }

    pub(crate) fn get_bridge<'a>(&'a self) -> &'a ConfBrigdgeHandle {
        &self.bridge
    }
}

impl From<PjsuaInstanceHandle> for PjsuaInstanceUninit {
    fn from(handle: PjsuaInstanceHandle) -> Self {
        PjsuaInstanceUninit { handle }
    }
}

impl PjsuaInstanceInit {
    fn from(
        instance: PjsuaInstanceUninit,
        mut pjsua_config: pjsua_config::PjsuaConfig,
        mut log_config: pjsua_config::LogConfig,
        mut media_config: pjsua_config::MediaConfig,
    ) -> Result<Self, PjsuaError> {
        unsafe {
            get_error_as_result(pjsua::pjsua_init(
                pjsua_config.as_mut(),
                log_config.as_mut(),
                media_config.as_mut(),
            ))?;

            get_error_as_result(pjsua::pjsua_set_null_snd_dev())?;

            Ok(PjsuaInstanceInit {
                handle: instance.handle,
                pjsua_config,
                log_config,
                media_config,
            })
        }
    }
}
