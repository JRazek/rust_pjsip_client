use super::error::{get_error_as_result as get_pjsua_error, Error as PjsuaError};
use std::{mem::MaybeUninit, os::raw::c_int};

struct PjsuaTransport {
    transport_id: i32,
    transport_config: pjsua::pjsua_transport_config,
}

impl PjsuaTransport {
    pub fn new(port: Option<u16>) -> Result<Self, PjsuaError> {
        let mut transport_config =
            unsafe { MaybeUninit::<pjsua::pjsua_transport_config>::zeroed().assume_init() };

        transport_config.port = port.unwrap_or(0) as u32;

        let mut transport_id: c_int = 0;

        unsafe {
            pjsua::pjsua_transport_config_default(&mut transport_config);

            get_pjsua_error(pjsua::pjsua_transport_create(
                pjsua::pjsip_transport_type_e_PJSIP_TRANSPORT_UDP as u32,
                &mut transport_config,
                &mut transport_id,
            ))?;
        };

        Ok(PjsuaTransport {
            transport_id,
            transport_config,
        })
    }
}
