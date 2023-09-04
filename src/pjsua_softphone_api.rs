use super::error::PjsuaError;
use std::marker::PhantomData;
use std::sync::Mutex;

use super::error::get_error_as_result;
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
            let status = get_error_as_result(pjsua::pjsua_destroy());

            eprintln!("Dropping PjsuaInstanceHandle status: {:?}", status);
        }
    }
}

impl PjsuaInstanceHandle {
    pub fn get_instance() -> Result<PjsuaInstanceHandle, PjsuaError> {
        static INSTANCE_CRATED: Mutex<bool> = Mutex::new(false);

        if let Ok(mut instance_guard) = INSTANCE_CRATED.try_lock() {
            let val = *instance_guard;
            if let false = val {
                *instance_guard = true;
                unsafe {
                    get_error_as_result(pjsua::pjsua_create())?;
                }
                return Ok(PjsuaInstanceHandle {
                    _private: (),
                    _not_send_sync: PhantomData,
                });
            };
        }

        Err(PjsuaError {
            code: -1,
            message: "Pjsua instance already created".to_string(),
        })
    }
}

pub struct PjsuaInstanceUninit {
    handle: PjsuaInstanceHandle,
}

//keep in mind the order of fields.
//The order of fields is important for the drop order.
//PjsuaInstanceInit MUST be dropped as the last, as it uninitializes pjsua completely.
pub struct PjsuaInstanceInit {
    log_config: pjsua_config::LogConfig,
    _media_config: pjsua_config::MediaConfig,
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
    pub fn start(self) -> Result<PjsuaInstanceStarted, PjsuaError> {
        unsafe {
            get_error_as_result(pjsua::pjsua_start())?;
        }

        let handle = Rc::new(self.pjsua_instance_init.handle);
        let bridge = ConfBrigdgeHandle::get_instance(handle.clone()).unwrap();

        let instance_started = PjsuaInstanceStarted {
            _log_config: self.pjsua_instance_init.log_config,
            _pjsua_config: self.pjsua_instance_init.pjsua_config,
            _transport: self.transport,
            _handle: handle,
            bridge,
        };

        Ok(instance_started)
    }
}

impl PjsuaInstanceUninit {
    pub fn get_instance() -> Result<PjsuaInstanceUninit, PjsuaError> {
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
    ) -> Result<PjsuaInstanceInitTransportConfigured, PjsuaError> {
        unsafe {
            let mut transport_id: pjsua::pjsua_transport_id = 0;

            get_error_as_result(pjsua::pjsua_transport_create(
                pjsua::pjsip_transport_type_e_PJSIP_TRANSPORT_UDP,
                transport.as_mut(),
                &mut transport_id,
            ))?;

            let instance_transport_set = PjsuaInstanceInitTransportConfigured {
                pjsua_instance_init: self,
                transport,
            };

            Ok(instance_transport_set)
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
                _media_config: media_config,
            })
        }
    }
}
