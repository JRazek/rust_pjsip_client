#[derive(Debug)]
pub struct PjsuaCall {
    account_id: pjsua::pjsua_acc_id,
    call_id: pjsua::pjsua_call_id,
}

impl PjsuaCall {
    pub fn new(account_id: pjsua::pjsua_acc_id, call_id: pjsua::pjsua_call_id) -> Self {
        Self {
            account_id,
            call_id,
        }
    }
}
