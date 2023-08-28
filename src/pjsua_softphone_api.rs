use std::sync::Mutex;
use std::{marker::PhantomData, ptr};

use crate::error::{get_error_as_option, get_error_as_result};
use crate::{pjsua_account_config, pjsua_config, pjsua_types, transport};
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
    accounts: Vec<Arc<pjsua_account_config::AccountConfigAdded>>,
    account_incoming_calls_rxs: Vec<mpsc::Receiver<OnIncomingCallSendData>>,
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
            pub async fn add_account(&mut self, account: (pjsua_account_config::AccountConfig, pjsua_account_config::IncomingCallReceiver)) -> ();
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

impl PjsuaInstanceInitTransportConfigured<Started> {
    pub async fn next_call(&mut self) {}
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
    pub async fn add_account(
        &mut self,
        account: (
            pjsua_account_config::AccountConfig,
            pjsua_account_config::IncomingCallReceiver,
        ),
    ) {
        let (account, mut incoming_call_receiver) = account;

        let account_added = Arc::new(account.add(&self));
        self.accounts.push(account_added.clone());

        let (tx, rx) = mpsc::channel(1);
        self.account_incoming_calls_rxs.push(rx);

        tokio::spawn(async move {
            loop {
                let account_future = incoming_call_receiver.next_call();
                let incoming_call = account_future.await;
                if let Err(_) = tx.send(incoming_call).await {
                    return;
                }
            }
        });
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
            account_incoming_calls_rxs: Vec::new(),
            pjsua_config,
            log_config,
        }
    }
}
