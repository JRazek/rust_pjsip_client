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

    let instance = instance.init(pjsua_config).expect("init failed!");

    let transport = PjsuaTransport::new(None);

    let instance = instance.set_transport(transport);

    let account_config = AccountConfig::new("7002", "7002", "127.0.0.1:5000");

    let instance = instance.start();

    let mut account_added = instance
        .add_account(account_config)
        .await
        .expect("add_account failed!");

    let incoming_call = account_added.next_call().await;

    println!("answering...");

    let mut memory_pool = PjsuaMemoryPool::new(10000, 10000).expect("PjsuaMemoryPool::new failed!");

    println!("memory_pool: {:?}", memory_pool);

    let call = incoming_call.answer_ok().await.expect("answer failed!");

    let mem_pool = PjsuaMemoryPool::new(1024, 1024).expect("Failed to create memory pool");

    let sink_buffer_media_port = PjsuaSinkBufferMediaPort::new(None, 8000, 1, 1024, &mem_pool)
        .expect("Failed to create sink buffer media port");

    let media_port_connected = call
        .connect_with_sink_media_port(sink_buffer_media_port, &mem_pool)
        .expect("Failed to connect sink buffer media port");

    for _ in 0..10 {
        media_port_connected
            .get_frame()
            .expect("Failed to get frame");

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    drop(media_port_connected);

    call.hangup().await.expect("Failed to hangup");

    //    call.await_hangup(media_port_connected)
    //        .await
    //        .expect("await_hangup failed!");
}
