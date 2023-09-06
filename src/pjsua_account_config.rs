use std::{ffi::CString, mem::MaybeUninit};

use crate::error::get_error_as_result;
use crate::{ffi_assert, pjsua_call, pjsua_softphone_api};

use pjsua::pj_str;

const CSTRING_NEW_FAILED: &str = "CString::new failed!";

use tokio::sync::mpsc;

use super::error::PjsuaError;

pub struct AccountConfigAdded<'a> {
    account_id: pjsua::pjsua_acc_id,
    account_config: Box<pjsua::pjsua_acc_config>,
    on_incoming_call_rx: IncomingCallReceiver,
    _cred_info: Vec<CredInfo>,
    _id_owned: CString,
    _uri_owned: CString,
    _pjsua_instance_started: &'a pjsua_softphone_api::PjsuaInstanceStarted,
}

pub struct IncomingCallReceiver {
    on_incoming_call_rx: mpsc::Receiver<cb_user_data::OnIncomingCallSendData>,
}

impl IncomingCallReceiver {
    pub async fn next_call(&mut self) -> cb_user_data::OnIncomingCallSendData {
        self.on_incoming_call_rx
            .recv()
            .await
            .expect("channel should not be closed at that point!")
    }
}

pub struct AccountConfig {
    account_config: Box<pjsua::pjsua_acc_config>,
    on_incoming_call_rx: IncomingCallReceiver,
    _cred_info: Vec<CredInfo>,
    _id_owned: CString,
    _uri_owned: CString,
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
    pub fn new(username: &str, password: &str, domain: &str) -> Self {
        let (mut account_config, on_incoming_call_rx) = unsafe {
            let mut account_config =
                Box::new(MaybeUninit::<pjsua::pjsua_acc_config>::zeroed().assume_init());

            pjsua::pjsua_acc_config_default(account_config.as_mut());

            let (on_incoming_call_tx, on_incoming_call_rx) = mpsc::channel(10);

            let on_incoming_call_tx = Box::new(cb_user_data::AccountConfigUserData {
                on_incoming_call_tx,
            });

            account_config.user_data =
                Box::into_raw(on_incoming_call_tx) as *mut ::std::os::raw::c_void;

            assert!(!account_config.user_data.is_null());

            (account_config, on_incoming_call_rx)
        };

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

        let on_incoming_call_rx = IncomingCallReceiver {
            on_incoming_call_rx,
        };

        let account_config = Self {
            account_config,
            on_incoming_call_rx,
            _id_owned: id,
            _uri_owned: uri,
            _cred_info: vec![cred_info0],
        };

        account_config
    }

    pub(crate) fn add_to_instance_init<'a>(
        mut self,
        pjsua_instance_started: &'a pjsua_softphone_api::PjsuaInstanceStarted,
    ) -> Result<AccountConfigAdded<'a>, PjsuaError> {
        let account_raw = self.as_mut();

        let mut account_id: pjsua::pjsua_acc_id = 2;

        unsafe {
            get_error_as_result(pjsua::pjsua_acc_add(
                account_raw,
                pjsua::pj_constants__PJ_TRUE as i32,
                &mut account_id,
            ))?;

            let user_data = pjsua::pjsua_acc_get_user_data(account_id);

            ffi_assert!(!user_data.is_null());
        }

        eprintln!("added account. account_id: {}", account_id);

        let config_added = AccountConfigAdded {
            account_id,
            account_config: self.account_config,
            on_incoming_call_rx: self.on_incoming_call_rx,
            _cred_info: self._cred_info,
            _id_owned: self._id_owned,
            _uri_owned: self._uri_owned,
            _pjsua_instance_started: pjsua_instance_started,
        };

        Ok(config_added)
    }
}

impl<'a> AccountConfigAdded<'a> {
    pub async fn next_call(&mut self) -> Result<pjsua_call::PjsuaIncomingCall, PjsuaError> {
        let (account_id, call_id) = self.on_incoming_call_rx.next_call().await;
        pjsua_call::PjsuaIncomingCall::new(account_id, call_id, self._pjsua_instance_started)
    }
}

impl<'a> Drop for AccountConfigAdded<'a> {
    fn drop(&mut self) {
        unsafe {
            let on_incoming_call_tx = pjsua::pjsua_acc_get_user_data(self.account_id)
                as *mut cb_user_data::AccountConfigUserData;

            assert!(!on_incoming_call_tx.is_null());
            let status = get_error_as_result(pjsua::pjsua_acc_del(self.account_id));
            if let Err(e) = status {
                eprintln!("error while dropping account: {}", e);
            }

            //assuming that on_incoming_call cb is neigther in progress nor to be called again
            //this assumption is made on the premises of:
            //https://docs.pjsip.org/en/latest/_static/PJSIP-Dev-Guide.pdf#page=13 [[Thread Safety]]

            eprintln!("dropping on_incoming_call_tx...");
            let on_incoming_call_tx = Box::from_raw(on_incoming_call_tx);
            drop(on_incoming_call_tx);
        }
    }
}

impl<'a> AsMut<pjsua::pjsua_acc_config> for AccountConfigAdded<'a> {
    fn as_mut(&mut self) -> &mut pjsua::pjsua_acc_config {
        &mut self.account_config
    }
}

impl AsMut<pjsua::pjsua_acc_config> for AccountConfig {
    fn as_mut(&mut self) -> &mut pjsua::pjsua_acc_config {
        &mut self.account_config
    }
}

pub(crate) mod cb_user_data {
    use tokio::sync::mpsc::Sender;

    #[allow(unused_parens)]
    pub(crate) type OnIncomingCallSendData = (pjsua::pjsua_acc_id, pjsua::pjsua_call_id);

    pub struct AccountConfigUserData {
        pub(crate) on_incoming_call_tx: Sender<OnIncomingCallSendData>,
    }
}
