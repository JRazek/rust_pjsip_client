use std::mem::MaybeUninit;

#[derive(Debug, Clone)]
pub struct PjsuaTransport {
    transport_config: pjsua::pjsua_transport_config,
}

impl PjsuaTransport {
    pub fn new(port: Option<u16>) -> Self {
        let mut transport_config =
            unsafe { MaybeUninit::<pjsua::pjsua_transport_config>::zeroed().assume_init() };

        unsafe {
            pjsua::pjsua_transport_config_default(&mut transport_config);
        };

        transport_config.port = port.unwrap_or(0) as u32;

        PjsuaTransport { transport_config }
    }
}

impl AsMut<pjsua::pjsua_transport_config> for PjsuaTransport {
    fn as_mut(&mut self) -> &mut pjsua::pjsua_transport_config {
        &mut self.transport_config
    }
}
