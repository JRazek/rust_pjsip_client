use crate::{
    error::ffi_assert_res,
    error::get_error_as_result,
    ffi_assert,
    pjsua_account_config::cb_user_data::{AccountConfigUserData, OnIncomingCallSendData},
    pjsua_call::cb_user_data::StateChangedUserData,
    pjsua_call::PjsipInvState,
};

use std::mem::MaybeUninit;

pub unsafe extern "C" fn on_incoming_call(
    acc_id: pjsua::pjsua_acc_id,
    call_id: pjsua::pjsua_call_id,
    rx_data: *mut pjsua::pjsip_rx_data,
) {
    eprintln!("on_incoming_call callback...");
    ffi_assert!(!rx_data.is_null(), "rx_data musn't be null!");

    let rx_data = rx_data.as_mut().unwrap();
    let transport_info = rx_data.tp_info;
    let _transport = transport_info.transport.as_ref().unwrap();

    let account_user_data = pjsua::pjsua_acc_get_user_data(acc_id) as *const AccountConfigUserData;

    //user data should be valid, allocated ptr here due to ffi_assert!
    //also, since pjsua_acc_del is called on Drop in AccountConfigAdded, where this buffer is allocated,
    //also not that this value is not stored in reference/box due to aliasing invariants of Rust.

    ffi_assert!(
        !account_user_data.is_null(),
        "on_incoming_call_tx channel is closed!"
    );

    let incoming_call_tx = &(*account_user_data).on_incoming_call_tx;
    let send_data: OnIncomingCallSendData = (acc_id, call_id);
    incoming_call_tx
        .try_send(send_data)
        .expect("channel should not be closed at that point!");

    eprintln!("on_incoming_call callback returned");
}

pub unsafe extern "C" fn on_call_state(
    call_id: pjsua::pjsua_call_id,
    _pjsip_event: *mut pjsua::pjsip_event,
) {
    //    ffi_assert!(!pjsip_event.is_null(), "pjsip_event musn't be null!");

    //state_changed_user_data may me null when the on_incoming_call is called, but no
    //OnIncomingCall instance is created.

    if let Some(state_changed_user_data) =
        (pjsua::pjsua_call_get_user_data(call_id) as *mut StateChangedUserData).as_mut()
    {
        let mut info = MaybeUninit::<pjsua::pjsua_call_info>::zeroed().assume_init();
        pjsua::pjsua_call_get_info(call_id, &mut info);

        let state: Result<PjsipInvState, ()> = info.state.try_into();

        eprintln!("on_call_state callback: {:?}", state);

        let state = ffi_assert_res(state);

        let res = (*state_changed_user_data)
            .on_state_changed_tx
            .try_send((call_id, state));

        ffi_assert_res(res);
    }
}

unsafe extern "C" fn on_media_event(event: *mut pjsua::pjmedia_event) {
    eprintln!("on_media_event callback. type: {:?}", (*event).type_);
}

unsafe extern "C" fn on_create_media_transport(
    call_id: pjsua::pjsua_call_id,
    media_idx: ::std::os::raw::c_uint,
    base_tp: *mut pjsua::pjmedia_transport,
    flags: ::std::os::raw::c_uint,
) -> *mut pjsua::pjmedia_transport {
    eprintln!(
        "on_create_media_transport callback. call_id: {:?}, media_idx: {:?}, flags: {:?}",
        call_id, media_idx, flags
    );

    base_tp
}

unsafe extern "C" fn on_call_media_state(call_id: pjsua::pjsua_call_id) {
    use super::pjsua_call;

    let state_changed_user_data =
        pjsua::pjsua_call_get_user_data(call_id) as *mut StateChangedUserData;

    let call_info = ffi_assert_res(pjsua_call::get_call_info(call_id));

    let media_status: pjsua_call::CallMediaStatus =
        ffi_assert_res(call_info.media_status.try_into());
    let call_state: pjsua_call::PjsipInvState = ffi_assert_res(call_info.state.try_into());

    eprintln!(
        "on_call_media_state: {:?} for call: {:?}",
        media_status, call_id
    );

    if pjsua_call::CallMediaStatus::Active == media_status
        && pjsua_call::PjsipInvState::Confirmed == call_state
    {
        if let Some(call_media_data_rx) = &mut (*state_changed_user_data).call_media_data_rx {
            match call_media_data_rx.try_recv() {
                Ok(call_media_data) => {
                    let call_conf_port = ffi_assert_res(pjsua_call::get_call_conf_port(call_id));

                    call_media_data
                        .sinks_slots
                        .iter()
                        .for_each(|added_port_slot| {
                            eprintln!(
                                "Connecting {:?} to {:?}...",
                                call_conf_port, *added_port_slot
                            );

                            let status = get_error_as_result(pjsua::pjsua_conf_connect(
                                call_conf_port,
                                added_port_slot.sink_slot,
                            ));

                            ffi_assert_res(status);

                            eprintln!(
                                "connected conf bridge slots {:?} -> {:?}",
                                call_conf_port, *added_port_slot
                            )
                        });
                }
                _ => {}
            };
        }
    }
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
            pjsua_config.cb.on_media_event = Some(on_media_event);
            pjsua_config.cb.on_call_media_state = Some(on_call_media_state);
            pjsua_config.cb.on_create_media_transport = Some(on_create_media_transport);

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

            log_cfg.console_level = 4;
            log_cfg.level = 4;

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

pub struct MediaConfig {
    media_cfg: Box<pjsua::pjsua_media_config>,
}

impl Default for MediaConfig {
    fn default() -> Self {
        unsafe {
            let mut media_cfg =
                Box::new(MaybeUninit::<pjsua::pjsua_media_config>::zeroed().assume_init());

            pjsua::pjsua_media_config_default(media_cfg.as_mut());

            media_cfg.no_vad = 1;
            media_cfg.ec_tail_len = 0;
            media_cfg.snd_auto_close_time = 0;

            Self { media_cfg }
        }
    }
}

impl AsMut<pjsua::pjsua_media_config> for MediaConfig {
    fn as_mut(&mut self) -> &mut pjsua::pjsua_media_config {
        self.media_cfg.as_mut()
    }
}
