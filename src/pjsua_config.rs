use super::pjsua_types::PjsipRxData;
use super::transport::PjsuaTransport;
use pjsua::pj_strcpy2;
use std::{ffi::CString, mem::MaybeUninit, ops::DerefMut, os::raw::c_int, ptr};

use super::error::{get_error_as_option as get_pjsua_error, Error as PjsuaError};

pub unsafe extern "C" fn on_incoming_call(
    acc_id: pjsua::pjsua_acc_id,
    _call_id: pjsua::pjsua_call_id,
    rx_data: *mut pjsua::pjsip_rx_data,
) {
    let mut acc_info: pjsua::pjsua_acc_info = MaybeUninit::zeroed().assume_init();
    pjsua::pjsua_acc_get_info(acc_id, &mut acc_info);

    let rx_data = PjsipRxData::try_from(*rx_data).expect("PjsipRxData::try_from failed!");

    let rx_data: pjsua::pjsip_rx_data = rx_data.into();

    let ctype = *rx_data.msg_info.ctype;

    let name = std::slice::from_raw_parts(ctype.name.ptr as *mut u8, ctype.name.slen as usize);
    let content_type = std::str::from_utf8(name).expect("str::from_utf8 failed!");

    eprintln!("{:?}: {}", content_type, ctype.type_);
}

type RawCallbackType = unsafe extern "C" fn(
    acc_id: pjsua::pjsua_acc_id,
    call_id: pjsua::pjsua_call_id,
    rx_data: *mut pjsua::pjsip_rx_data,
);

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
