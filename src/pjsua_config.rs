use std::{ffi::CString, mem::MaybeUninit, os::raw::c_int, ptr};

struct PjsuaConfig {
    pjsua_config: MaybeUninit<pjsua::pjsua_config>,
}

impl PjsuaConfig {
    fn new(username: &str, password: &str, domain: &str) -> Self {
        let mut cfg = MaybeUninit::<pjsua::pjsua_config>::uninit();
        unsafe {
            pjsua::pjsua_config_default(cfg.as_mut_ptr());

            PjsuaConfig { pjsua_config: cfg }
        }
    }
}
