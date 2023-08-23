use pjsip_client::pjsua_account_config::AccountConfig;
use pjsip_client::pjsua_config::PjsuaConfig;
use pjsip_client::pjsua_softphone_api::PjsuaInstanceUninit;
use pjsip_client::transport::PjsuaTransport;

fn main() {
    let instance =
        PjsuaInstanceUninit::get_instance().expect("PjsuaInstance::get_instance failed!");

    let pjsua_config = PjsuaConfig::new();

    let mut instance = instance.init(pjsua_config);

    let transport = PjsuaTransport::new(None);

    instance.set_transport(transport);

    unsafe {
        pjsua::pjsua_start();
    }

    let account_config = AccountConfig::new("username", "password", "sip_server");

    instance.add_account(account_config);
}
