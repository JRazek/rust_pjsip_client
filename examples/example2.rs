use std::path::Path;

use pjsip_client::pj_types::Frame;
use pjsip_client::pjsua_account_config::AccountConfig;
use pjsip_client::pjsua_config::PjsuaConfig;
use pjsip_client::pjsua_softphone_api::PjsuaInstanceUninit;
use pjsip_client::transport::PjsuaTransport;

use pjsip_client::pjsua_memory_pool::PjsuaMemoryPool;

use pjsip_client::pjmedia::pjmedia_port_audio_sink::{CustomSinkMediaPort, CustomSinkMediaPortRx};
use pjsip_client::pjmedia::pjmedia_port_audio_stream::{
    CustomStreamMediaPort, CustomStreamMediaPortTx,
};
use pjsip_client::pjsua_call;

use tokio::fs::read as read_file;

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

pub async fn read_pcm_and_send_task(
    path: impl AsRef<Path>,
    frames_tx: CustomStreamMediaPortTx,
    sample_rate: u32,
    channels_count: usize,
    bits_per_sample: usize,
    samples_per_frame: usize,
) -> Result<(), std::io::Error> {
    let buffer = tokio::fs::read(path).await?;

    let mut i = 0;

    let sample_duration_usec = 1_000_000 / channels_count as u32 / sample_rate as u32;

    assert!(bits_per_sample % 8 == 0);

    let bytes_in_sample = bits_per_sample / 8;

    while let Some(chunk) = buffer.chunks(samples_per_frame * bytes_in_sample).next() {
        let frame = Frame {
            data: chunk.into(),
            time: std::time::Duration::from_micros(i * sample_duration_usec as u64),
        };

        frames_tx.send(frame).await.unwrap();

        i += 1;
    }

    Ok(())
}

pub async fn handle_call(incoming_call: pjsua_call::PjsuaIncomingCall<'_>) {
    let mem_pool = PjsuaMemoryPool::new(10000, 10000).expect("Failed to create memory pool");

    let call = incoming_call
        .answer_session_progress()
        .await
        .expect("answer failed!");

    let (sink_media_port, frames_rx) =
        CustomSinkMediaPort::new(8000, 1, 8000, &mem_pool).expect("test");

    let (stream_media_port, frames_tx) =
        CustomStreamMediaPort::new(22050, 1, 8000, &mem_pool).expect("test");

    let call = call
        .add(sink_media_port, stream_media_port, &mem_pool)
        .await
        .expect("connect failed!");

    tokio::spawn(async {
        read_pcm_and_send_task(
            Path::new("samples/gettysburg.raw"),
            frames_tx,
            22050,
            1,
            16,
            8000,
        )
        .await
        .unwrap()
    });

    call.await_hangup().await.expect("hangup failed!");
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

    let mut account_added = instance
        .add_account(account_config1)
        .await
        .expect("add_account failed!");

    while let Ok(incoming_call) = account_added.next_call().await {
        handle_call(incoming_call).await;
    }
}
