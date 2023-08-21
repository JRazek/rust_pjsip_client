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
