use pjsip_client::pjsua_account_config::AccountConfig;
use pjsip_client::pjsua_config::PjsuaConfig;
use pjsip_client::pjsua_softphone_api::PjsuaInstanceUninit;
use pjsip_client::transport::PjsuaTransport;

use pjsip_client::pjsua_memory_pool::PjsuaMemoryPool;

use pjsip_client::pjmedia_port_audio_sink::{CustomSinkMediaPort, CustomSinkMediaPortRx};

pub async fn recv_task(mut frames_rx: CustomSinkMediaPortRx) {
    let mut i = 0;

    while let Some(frame) = frames_rx.recv().await {
        eprintln!(
            "received frame #{}. Size: {}, time: {:?}",
            i,
            frame.data.len(),
            frame.time
        );

        i += 1;
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

    let incoming_call = account_added1.next_call().await.expect("test");
    println!("answering...");

    let call = incoming_call
        .answer_session_progress()
        .await
        .expect("answer failed!");

    let (sink_buffer_media_port, frames_rx) =
        CustomSinkMediaPort::new(8000, 1, 8000, &mem_pool).expect("test");

    let call = call
        .add(sink_buffer_media_port, &mem_pool)
        .await
        .expect("connect failed!");

    tokio::select! {
        _ = call.await_hangup() => {},
        _ = recv_task(frames_rx) => {},
    };
}
