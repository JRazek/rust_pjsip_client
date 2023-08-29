use crate::{
    ffi_assert,
    pjsua_account_config::cb_user_data::{AccountConfigUserData, OnIncomingCallSendData},
    pjsua_call::cb_user_data::StateChangedUserData,
};

use std::{ffi, mem::MaybeUninit};

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

pub unsafe extern "C" fn on_call_state(
    call_id: pjsua::pjsua_call_id,
    _pjsip_event: *mut pjsua::pjsip_event,
) {
    //    ffi_assert!(!pjsip_event.is_null(), "pjsip_event musn't be null!");

    let state_changed_user_data =
        pjsua::pjsua_call_get_user_data(call_id) as *mut StateChangedUserData;

    ffi_assert!(
        !state_changed_user_data.is_null(),
        "user data musn't be null!"
    );

    let mut info = MaybeUninit::<pjsua::pjsua_call_info>::zeroed().assume_init();
    pjsua::pjsua_call_get_info(call_id, &mut info);

    let state = info.state.try_into();

    ffi_assert!(state.is_ok(), "pjsua::pjsua_call_info::state is not valid!");

    let res = (*state_changed_user_data)
        .on_state_changed_tx
        .blocking_send((call_id, state.unwrap()));

    ffi_assert!(res.is_ok(), "on_state_changed_tx channel is closed!");
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
            pjsua_config.cb.on_call_state = Some(on_call_state);

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

            log_cfg.console_level = 1;

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
