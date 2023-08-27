use std::mem::ManuallyDrop;
use std::sync::Mutex;
use std::{marker::PhantomData, ptr};

use crate::error::{get_error_as_option, get_error_as_result};
use crate::{pjsua_account_config, pjsua_config, pjsua_types, transport};
use delegate::delegate;

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
    accounts: Vec<pjsua_account_config::AccountConfigAdded>,
    log_config: pjsua_config::LogConfig,
    pjsua_config: pjsua_config::PjsuaConfig,
    handle: PjsuaInstanceHandle,
}

pub struct NotStarted;
pub struct Started;

pub struct PjsuaInstanceInitTransportConfigured<State = NotStarted> {
    pjsua_instance_init: PjsuaInstanceInit,
    transport: transport::PjsuaTransport,

    _state: PhantomData<State>,
}

impl PjsuaInstanceInitTransportConfigured<NotStarted> {
    delegate! {
        to self.pjsua_instance_init {
            pub fn add_account(&mut self, account: pjsua_account_config::AccountConfig);
        }
    }
}

impl PjsuaInstanceInitTransportConfigured<NotStarted> {
    pub fn start(self) -> PjsuaInstanceInitTransportConfigured<Started> {
        unsafe {
            pjsua::pjsua_start();
        }

        PjsuaInstanceInitTransportConfigured {
            pjsua_instance_init: self.pjsua_instance_init,
            transport: self.transport,
            _state: PhantomData,
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

    pub async fn accept_call(&self) -> () {}
}

impl PjsuaInstanceInit {
    pub fn add_account(&mut self, account: pjsua_account_config::AccountConfig) {
        let account_added = account.add(&self);
        self.accounts.push(account_added);
    }

    pub fn set_transport(
        self,
        mut transport: transport::PjsuaTransport,
    ) -> PjsuaInstanceInitTransportConfigured<NotStarted> {
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
                _state: PhantomData,
            };

            instance_transport_set
        }
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
            accounts: Vec::new(),
            pjsua_config,
            log_config,
        }
    }
}
