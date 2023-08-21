use std::{ffi::CString, mem::MaybeUninit, os::raw::c_int, ptr};

use super::error::{get_error as get_pjsua_error, Error as PjsuaError};

const CSTRING_NEW_FAILED: &str = "CString::new failed!";

struct PjsuaConfig {
    pjsua_config: pjsua::pjsua_config,
}

impl PjsuaConfig {
    fn new() -> Self {
        let mut cfg = unsafe { MaybeUninit::<pjsua::pjsua_config>::zeroed().assume_init() };
        unsafe {
            pjsua::pjsua_config_default(&mut cfg);
        };

        PjsuaConfig { pjsua_config: cfg }
    }
}

struct AccountConfig {
    account_cfg: pjsua::pjsua_acc_config,
}

impl AccountConfig {
    fn new(username: &str, password: &str, domain: &str) -> Result<Self, super::error::Error> {
        let id = CString::new(&*format!("sip:{}@{}", username, domain)).expect(CSTRING_NEW_FAILED);
        let uri = CString::new(&*format!("sip:{}", domain)).expect(CSTRING_NEW_FAILED);

        let all_realm = CString::new("*").expect(CSTRING_NEW_FAILED);

        let digest = CString::new("digest").expect(CSTRING_NEW_FAILED);

        let password = CString::new(password).expect(CSTRING_NEW_FAILED);

        unsafe {
            use pjsua::pj_str;

            let mut acc_cfg = MaybeUninit::<pjsua::pjsua_acc_config>::zeroed().assume_init();

            acc_cfg.id = pj_str(id.as_ptr() as *mut i8);
            acc_cfg.reg_uri = pj_str(uri.as_ptr() as *mut i8);
            acc_cfg.cred_count = 1;
            acc_cfg.cred_info[0].realm = pj_str(all_realm.as_ptr() as *mut i8);
            acc_cfg.cred_info[0].scheme = pj_str(digest.as_ptr() as *mut i8);
            acc_cfg.cred_info[0].username = pj_str(username.as_ptr() as *mut i8);

            acc_cfg.cred_info[0].data_type =
                pjsua::pjsip_cred_data_type_PJSIP_CRED_DATA_PLAIN_PASSWD as i32;

            acc_cfg.cred_info[0].data = pj_str(password.as_ptr() as *mut i8);

            let mut acc_id = pjsua::pjsua_acc_id::default();

            let status = pjsua::pjsua_acc_add(
                &mut acc_cfg,
                pjsua::pj_constants__PJ_TRUE as i32,
                &mut acc_id,
            );

            match get_pjsua_error(status) {
                Some(err) => Err(err),
                None => Ok(AccountConfig {
                    account_cfg: acc_cfg,
                }),
            }
        }
    }
}
