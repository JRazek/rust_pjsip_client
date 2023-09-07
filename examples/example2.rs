use pjsip_client::pjsua_account_config::AccountConfig;
use pjsip_client::pjsua_config::PjsuaConfig;
use pjsip_client::pjsua_softphone_api::PjsuaInstanceUninit;
use pjsip_client::transport::PjsuaTransport;

use pjsip_client::pjsua_memory_pool::PjsuaMemoryPool;

use pjsip_client::pjsua_call::{PjsuaCall, PjsuaCallSetup};
use pjsip_client::pjsua_sink_buffer_media_port::{
    PjsuaSinkBufferMediaPort, PjsuaSinkBufferMediaPortConnected,
};

use pjsip_client::pjmedia_port_audio_sink::CustomSinkMediaPort;

async fn run_call<'a>(pjsua_call: PjsuaCall<'a>) {
    tokio::select! {
        res = pjsua_call.await_hangup() => {
            if let Err(res) = res {
                println!("call ended with error: {:?}", res);
            } else {
                println!("call ended");
            }
        }
        _ = tokio::time::sleep(tokio::time::Duration::from_secs(120)) => {
            println!("call timed out");
        }
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

    while let Ok(incoming_call) = account_added1.next_call().await {
        println!("answering...");

        let call = incoming_call
            .answer_session_progress()
            .await
            .expect("answer failed!");

        let sink_buffer_media_port = CustomSinkMediaPort::new(8000, 1, 160, &mem_pool);

        let call = call
            .connect(sink_buffer_media_port, &mem_pool)
            .await
            .expect("connect failed!");

        run_call(call).await;
    }
}
