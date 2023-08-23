use super::pjsua_types::PjsipRxData;
use super::transport::PjsuaTransport;
use pjsua::pj_strcpy2;
use std::{ffi::CString, mem::MaybeUninit, ops::DerefMut, os::raw::c_int, ptr};

use super::error::{get_error_as_option as get_pjsua_error, Error as PjsuaError};

const CSTRING_NEW_FAILED: &str = "CString::new failed!";

pub unsafe extern "C" fn on_incoming_call(
    _acc_id: pjsua::pjsua_acc_id,
    _call_id: pjsua::pjsua_call_id,
    rx_data: *mut pjsua::pjsip_rx_data,
) {
    let rx_data = PjsipRxData::try_from(*rx_data).expect("PjsipRxData::try_from failed!");

    let rx_data: pjsua::pjsip_rx_data = rx_data.into();

    eprintln!("rx_data: {:?}", rx_data.tp_info);
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
