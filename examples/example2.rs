use pjsip_client::pjsua_account_config::AccountConfig;
use pjsip_client::pjsua_config::PjsuaConfig;
use pjsip_client::pjsua_softphone_api::PjsuaInstanceUninit;
use pjsip_client::transport::PjsuaTransport;

#[tokio::main]
async fn main() {
    let instance =
        PjsuaInstanceUninit::get_instance().expect("PjsuaInstance::get_instance failed!");

    let pjsua_config = PjsuaConfig::new();

    let instance = instance.init(pjsua_config);

    let transport = PjsuaTransport::new(None);

    let mut instance = instance.set_transport(transport);

    let account_config = AccountConfig::new("7002", "7002", "127.0.0.1:5000");

    instance.add_account(account_config).await;

    let mut instance = instance.start();

    let call = instance.next_call().await;

    println!("call: {:?}", call);

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
}
