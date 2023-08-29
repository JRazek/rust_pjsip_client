use std::sync::Mutex;
use std::{marker::PhantomData, ptr};

use super::pjsua_call::PjsuaIncomingCall;

use crate::{pjsua_account_config, pjsua_config, transport};
use delegate::delegate;
use std::sync::Arc;

use tokio::sync::mpsc;

use pjsua_account_config::cb_user_data::OnIncomingCallSendData;

struct PjsuaInstanceHandle {
    _not_send_sync: PhantomData<*const ()>,
    _private: (),
}

impl Drop for PjsuaInstanceHandle {
    fn drop(&mut self) {
        unsafe {
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
    pjsua_config: pjsua_config::PjsuaConfig,
    handle: PjsuaInstanceHandle,
}

pub struct PjsuaInstanceInitTransportConfigured {
    pjsua_instance_init: PjsuaInstanceInit,
    transport: transport::PjsuaTransport,
}

pub struct PjsuaInstanceStarted {
    //    _accounts: Vec<Arc<pjsua_account_config::AccountConfigAdded<'a>>>,
    //    new_calls_rx: mpsc::Receiver<OnIncomingCallSendData>,
    _log_config: pjsua_config::LogConfig,
    _pjsua_config: pjsua_config::PjsuaConfig,
    _transport: transport::PjsuaTransport,
    _handle: PjsuaInstanceHandle,
}

impl PjsuaInstanceInitTransportConfigured {
    pub fn start(self) -> PjsuaInstanceStarted {
        unsafe {
            pjsua::pjsua_start();
        }

        PjsuaInstanceStarted {
            _log_config: self.pjsua_instance_init.log_config,
            _pjsua_config: self.pjsua_instance_init.pjsua_config,
            _transport: self.transport,
            _handle: self.pjsua_instance_init.handle,
        }
    }
}

impl PjsuaInstanceUninit {
    pub fn get_instance() -> Option<PjsuaInstanceUninit> {
        PjsuaInstanceHandle::get_instance().map(|handle| PjsuaInstanceUninit { handle })
    }

    pub fn init(self, mut pjsua_config: pjsua_config::PjsuaConfig) -> PjsuaInstanceInit {
        unsafe {
            let mut log_cfg = pjsua_config::LogConfig::default();

            pjsua::pjsua_init(pjsua_config.as_mut(), log_cfg.as_mut(), ptr::null());

            PjsuaInstanceInit::from(self, pjsua_config, log_cfg)
        }
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
    ) -> pjsua_account_config::AccountConfigAdded {
        let account_added = account.add_to_instance_init(&self);

        //note, this does not use FFI

        account_added
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
        pjsua_config: pjsua_config::PjsuaConfig,
        log_config: pjsua_config::LogConfig,
    ) -> Self {
        PjsuaInstanceInit {
            handle: instance.handle,
            pjsua_config,
            log_config,
        }
    }
}
