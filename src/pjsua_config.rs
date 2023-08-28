use crate::{
    ffi_assert,
    pjsua_account_config::cb_user_data::{AccountConfigUserData, OnIncomingCallSendData},
};

use std::mem::MaybeUninit;

pub unsafe extern "C" fn on_incoming_call(
    acc_id: pjsua::pjsua_acc_id,
    call_id: pjsua::pjsua_call_id,
    rx_data: *mut pjsua::pjsip_rx_data,
) {
    ffi_assert!(!rx_data.is_null(), "rx_data musn't be null!");

    let rx_data = rx_data.as_mut().unwrap();
    let transport_info = rx_data.tp_info;
    let _transport = transport_info.transport.as_ref().unwrap();

    let account_user_data = pjsua::pjsua_acc_get_user_data(acc_id) as *mut AccountConfigUserData;

    ffi_assert!(
        !account_user_data.is_null(),
        "callback user data musn't be null!"
    );

    //user data should be valid, allocated ptr here due to ffi_assert!
    //also, since pjsua_acc_del is called on Drop in AccountConfigAdded, where this buffer is allocated,
    //also not that this value is not stored in reference/box due to aliasing invariants of Rust.
    let account_user_data = account_user_data as *const AccountConfigUserData;

    ffi_assert!(
        !account_user_data.is_null(),
        "on_incoming_call_tx channel is closed!"
    );

    let incoming_call_tx = &(*account_user_data).on_incoming_call_tx;
    let send_data: OnIncomingCallSendData = (acc_id, call_id);
    incoming_call_tx
        .blocking_send(send_data)
        .expect("channel should not be closed at that point!");
}

pub struct PjsuaConfig {
    pjsua_config: Box<pjsua::pjsua_config>,
}

impl PjsuaConfig {
    pub fn new() -> Self {
        unsafe {
            let mut pjsua_config =
                Box::new(MaybeUninit::<pjsua::pjsua_config>::zeroed().assume_init());

            pjsua::pjsua_config_default(pjsua_config.as_mut());

            pjsua_config.cb.on_incoming_call = Some(on_incoming_call);

            PjsuaConfig { pjsua_config }
        }
    }
}

pub struct LogConfig {
    logging_cfg: Box<pjsua::pjsua_logging_config>,
}

impl Default for LogConfig {
    fn default() -> Self {
        unsafe {
            let mut log_cfg =
                Box::new(MaybeUninit::<pjsua::pjsua_logging_config>::zeroed().assume_init());
            pjsua::pjsua_logging_config_default(log_cfg.as_mut());

            Self {
                logging_cfg: log_cfg,
            }
        }
    }
}

impl AsMut<pjsua::pjsua_logging_config> for LogConfig {
    fn as_mut(&mut self) -> &mut pjsua::pjsua_logging_config {
        self.logging_cfg.as_mut()
    }
}

impl AsMut<pjsua::pjsua_config> for PjsuaConfig {
    fn as_mut(&mut self) -> &mut pjsua::pjsua_config {
        &mut self.pjsua_config
    }
}
