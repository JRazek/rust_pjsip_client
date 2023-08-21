use pjsua::pj_str;
use std::{ffi::CString, mem::MaybeUninit, ops::DerefMut, os::raw::c_int, ptr};

use super::error::{get_error_as_option as get_pjsua_error, Error as PjsuaError};

const CSTRING_NEW_FAILED: &str = "CString::new failed!";

pub struct PjsuaConfig {
    pjsua_config: pjsua::pjsua_config,
}

impl PjsuaConfig {
    pub fn new() -> Self {
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

fn set_cred_info(
    cred_info: &mut pjsua::pjsip_cred_info,
    username: &str,
    password: &str,
    domain: &str,
) {
    let all_realm = CString::new("*").expect(CSTRING_NEW_FAILED);

    let digest = CString::new("digest").expect(CSTRING_NEW_FAILED);

    let password = CString::new(password).expect(CSTRING_NEW_FAILED);

    unsafe {
        cred_info.realm = pj_str(all_realm.as_ptr() as *mut i8);
        cred_info.scheme = pj_str(digest.as_ptr() as *mut i8);
        cred_info.username = pj_str(username.as_ptr() as *mut i8);

        cred_info.data_type = pjsua::pjsip_cred_data_type_PJSIP_CRED_DATA_PLAIN_PASSWD as i32;

        cred_info.data = pj_str(password.as_ptr() as *mut i8);
    }
}

impl Into<pjsua::pjsua_config> for PjsuaConfig {
    fn into(self) -> pjsua::pjsua_config {
        self.pjsua_config
    }
}

impl AccountConfig {
    pub fn new(username: &str, password: &str, domain: &str) -> Result<Self, super::error::Error> {
        let id = CString::new(&*format!("sip:{}@{}", username, domain)).expect(CSTRING_NEW_FAILED);
        let uri = CString::new(&*format!("sip:{}", domain)).expect(CSTRING_NEW_FAILED);

        unsafe {
            let mut acc_cfg = MaybeUninit::<pjsua::pjsua_acc_config>::zeroed().assume_init();

            acc_cfg.id = pj_str(id.as_ptr() as *mut i8);
            acc_cfg.reg_uri = pj_str(uri.as_ptr() as *mut i8);
            acc_cfg.cred_count = 1;

            set_cred_info(&mut acc_cfg.cred_info[0], username, password, domain);

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

impl Into<pjsua::pjsua_acc_config> for AccountConfig {
    fn into(self) -> pjsua::pjsua_acc_config {
        self.account_cfg
    }
}
