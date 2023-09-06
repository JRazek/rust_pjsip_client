use pjsip_client::pjsua_account_config::AccountConfig;
use pjsip_client::pjsua_config::PjsuaConfig;
use pjsip_client::pjsua_softphone_api::PjsuaInstanceUninit;
use pjsip_client::transport::PjsuaTransport;

use pjsip_client::pjsua_memory_pool::PjsuaMemoryPool;

use pjsip_client::pjsua_call::PjsuaCallSetup;
use pjsip_client::pjsua_sink_buffer_media_port::{
    PjsuaSinkBufferMediaPort, PjsuaSinkBufferMediaPortConnected,
};

async fn run_call<'a>(
    sink_buffer_media_port: PjsuaSinkBufferMediaPortConnected<'a>,
    pjsua_call: &'a PjsuaCallSetup<'a>,
) {
    while let Some(frame) = sink_buffer_media_port.get_frame().await {
        println!("frame: {:?}", frame);
    }
}

#[tokio::main]
async fn main() {
    let instance =
        PjsuaInstanceUninit::get_instance().expect("PjsuaInstance::get_instance failed!");

    let pjsua_config = PjsuaConfig::new();

    let instance = instance.init(pjsua_config).expect("init failed!");

    let transport = PjsuaTransport::new(None);

    let instance = instance
        .set_transport(transport)
        .expect("set_transport failed!");

    let account_config1 = AccountConfig::new("7002", "7002", "127.0.0.1:5000");

    let instance = instance.start().expect("start failed!");

    let mut account_added1 = instance
        .add_account(account_config1)
        .await
        .expect("add_account failed!");

    let mem_pool = PjsuaMemoryPool::new(10000, 10000).expect("Failed to create memory pool");

    for _ in 0..100 {
        let incoming_call1 = account_added1.next_call().await.expect("next_call failed!");

        println!("answering...");

        let call1 = incoming_call1
            .answer_session_progress()
            .await
            .expect("answer failed!");

        tokio::time::sleep(tokio::time::Duration::from_secs(100)).await;

        let sink_buffer_media_port = PjsuaSinkBufferMediaPort::new(None, 8000, 1, 160, &mem_pool)
            .expect("Failed to create sink buffer media port");

        let media_port_connected = call1
            .connect_with_sink_media_port(sink_buffer_media_port, &mem_pool)
            .expect("Failed to connect sink buffer media port");

        run_call(media_port_connected, &call1).await;

        call1.await_hangup().await.expect("await_hangup failed!");
    }
}
