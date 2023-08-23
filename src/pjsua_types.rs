use super::error::Error as PjsuaError;

pub struct PjsipRxData {
    rx_data: pjsua::pjsip_rx_data,
}

impl TryFrom<pjsua::pjsip_rx_data> for PjsipRxData {
    type Error = PjsuaError;

    fn try_from(rx_data: pjsua::pjsip_rx_data) -> Result<Self, Self::Error> {
        eprintln!("rx_data: {:?}", rx_data.tp_info);
        Ok(PjsipRxData { rx_data })
    }
}

impl Into<pjsua::pjsip_rx_data> for PjsipRxData {
    fn into(self) -> pjsua::pjsip_rx_data {
        self.rx_data
    }
}

pub struct PjsuaString(pub String);

impl PjsuaString {
    fn as_view(&mut self) -> pjsua::pj_str_t {
        let ptr = self.0.as_ptr() as *mut ::std::os::raw::c_char;
        let len = self.0.len() as u64;
        let slen = len as pjsua::pj_ssize_t;
        let view = pjsua::pj_str_t { ptr, slen };
        view
    }
}
