use std::{ffi::CString, mem::MaybeUninit};

use crate::{pjsua_softphone_api, pjsua_types};

use pjsua::pj_str;

const CSTRING_NEW_FAILED: &str = "CString::new failed!";

pub struct AccountConfigAdded {
    account_id: pjsua::pjsua_acc_id,
    account_config: AccountConfig,
}

pub struct AccountConfig {
    account_config: Box<pjsua::pjsua_acc_config>,
    cred_info: Vec<CredInfo>,
    id_owned: CString,
    uri_owned: CString,
}

struct CredInfo {
    cred_info: Box<pjsua::pjsip_cred_info>,

    _all_realm_owned: CString,
    _scheme_owned: CString,
    _username_owned: CString,
    _password_owned: CString,
    _domain_owned: CString,
}

impl CredInfo {
    fn new(all_realm: &str, scheme: &str, username: &str, password: &str, domain: &str) -> Self {
        let mut cred_info =
            unsafe { Box::new(MaybeUninit::<pjsua::pjsip_cred_info>::zeroed().assume_init()) };

        let all_realm_owned = CString::new(all_realm).expect(CSTRING_NEW_FAILED);
        let scheme_owned = CString::new(scheme).expect(CSTRING_NEW_FAILED);
        let username_owned = CString::new(username).expect(CSTRING_NEW_FAILED);
        let password_owned = CString::new(password).expect(CSTRING_NEW_FAILED);
        let domain_owned = CString::new(domain).expect(CSTRING_NEW_FAILED);

        unsafe {
            cred_info.realm = pj_str(all_realm_owned.as_ptr() as *mut ::std::os::raw::c_char);
            cred_info.scheme = pj_str(scheme_owned.as_ptr() as *mut ::std::os::raw::c_char);
            cred_info.username = pj_str(username_owned.as_ptr() as *mut ::std::os::raw::c_char);
            cred_info.data = pj_str(password_owned.as_ptr() as *mut ::std::os::raw::c_char);
            cred_info.data_type = pjsua::pjsip_cred_data_type_PJSIP_CRED_DATA_PLAIN_PASSWD as i32;
        }

        Self {
            cred_info,
            _all_realm_owned: all_realm_owned,
            _scheme_owned: scheme_owned,
            _username_owned: username_owned,
            _password_owned: password_owned,
            _domain_owned: domain_owned,
        }
    }
}

impl AccountConfig {
    #[allow(dead_code)]
    pub fn new(username: &str, password: &str, domain: &str) -> Self {
        let mut account_config = unsafe {
            let mut account_config =
                Box::new(MaybeUninit::<pjsua::pjsua_acc_config>::zeroed().assume_init());

            pjsua::pjsua_acc_config_default(account_config.as_mut());

            account_config
        };
        //

        let id = CString::new(&*format!("sip:{}@{}", username, domain)).expect(CSTRING_NEW_FAILED);
        let uri = CString::new(&*format!("sip:{}", domain)).expect(CSTRING_NEW_FAILED);

        let pjsua_acc_cfg = account_config.as_mut();

        unsafe {
            pjsua_acc_cfg.id = pj_str(id.as_ptr() as *mut i8);
            pjsua_acc_cfg.reg_uri = pj_str(uri.as_ptr() as *mut i8);
        }

        pjsua_acc_cfg.cred_count = 1;

        let cred_info0 = CredInfo::new("*", "digest", username, password, domain);
        pjsua_acc_cfg.cred_info[0] = *cred_info0.cred_info;

        let mut account_config = Self {
            account_config,
            id_owned: id,
            uri_owned: uri,
            cred_info: vec![cred_info0],
        };

        account_config
    }

    pub fn add(mut self, _: &pjsua_softphone_api::PjsuaInstanceInit) -> AccountConfigAdded {
        use crate::error::get_error_as_result;

        let account_raw = self.as_mut();

        let mut id: pjsua::pjsua_acc_id = 0;

        unsafe {
            if let Err(status) = get_error_as_result(pjsua::pjsua_acc_add(
                account_raw,
                pjsua::pj_constants__PJ_TRUE as i32,
                &mut id,
            )) {
                panic!("pjsua_acc_add failed with status: {}", status.message);
            }
        }

        AccountConfigAdded {
            account_config: self,
            account_id: id,
        }
    }
}

impl Drop for AccountConfigAdded {
    fn drop(&mut self) {
        unsafe {
            pjsua::pjsua_acc_del(self.account_id);
        }
    }
}

impl AsMut<pjsua::pjsua_acc_config> for AccountConfigAdded {
    fn as_mut(&mut self) -> &mut pjsua::pjsua_acc_config {
        &mut self.account_config.account_config
    }
}

impl AsMut<pjsua::pjsua_acc_config> for AccountConfig {
    fn as_mut(&mut self) -> &mut pjsua::pjsua_acc_config {
        &mut self.account_config
    }
}
