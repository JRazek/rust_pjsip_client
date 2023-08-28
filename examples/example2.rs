use pjsip_client::pjsua_account_config::AccountConfig;
use pjsip_client::pjsua_config::PjsuaConfig;
use pjsip_client::pjsua_softphone_api::PjsuaInstanceUninit;
use pjsip_client::transport::PjsuaTransport;

use pjsip_client::pjsua_memory_pool::PjsuaMemoryPool;
use pjsip_client::pjsua_sink_buffer_media_port::PjsuaSinkBufferMediaPort;

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

    let incoming_call = instance.next_call().await;

    println!("call: {:?}", incoming_call);

    println!("answering...");

    let mut memory_pool = PjsuaMemoryPool::new(10000, 10000).expect("PjsuaMemoryPool::new failed!");

    println!("memory_pool: {:?}", memory_pool);

    let sink_buffer_media_port =
        PjsuaSinkBufferMediaPort::new(2048, 8000, 1, 1024, &mut memory_pool)
            .expect("PjsuaSinkBufferMediaPort::new failed!");

    println!("sink_buffer_media_port: {:?}", sink_buffer_media_port);

    let call = incoming_call
        .answer_ok(sink_buffer_media_port)
        .expect("answer failed!");

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    println!("hanging up...");

    call.hangup().expect("hangup failed!");
}
