use crate::error::ffi_assert_res;
use crate::error::get_error_as_result;
use crate::error::PjsuaError;
use crate::pjsua_memory_pool::PjsuaMemoryPool;

use crate::pj_types::PjString;

use crate::pj_types::Frame;

use std::sync::atomic::AtomicU32;

use tokio::sync::mpsc as tokio_mpsc;

use crate::ffi_assert;

use super::pjmedia_api;

unsafe extern "C" fn custom_port_put_frame(
    port: *mut pjsua::pjmedia_port,
    frame: *mut pjsua::pjmedia_frame,
) -> pjsua::pj_status_t {
    static mut COUNTER: AtomicU32 = AtomicU32::new(0);

    let count = unsafe { COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst) };

    if count % 100 == 0 {
        println!(
            "custom_port_put_frame: frame buffer size: {:?}",
            (*frame).size
        );
    }

    if frame.is_null() || (*frame).buf.is_null() || (*frame).size == 0 {
        return 0;
    }

    let media_port_data = unsafe { (*port).port_data.pdata as *mut MediaPortData };
    let sample_rate = (*media_port_data).sample_rate;
    let channels_count = (*media_port_data).channels_count;

    let frame_type = unsafe { (*frame).type_ };
    let _bit_info = unsafe { (*frame).bit_info };

    ffi_assert!(frame_type == pjsua::pjmedia_frame_type_PJMEDIA_FRAME_TYPE_AUDIO);

    let frame =
        ffi_assert_res(unsafe { Frame::from_raw_frame(&*frame, sample_rate, channels_count) });

    if let Err(_) = (*media_port_data).frames_tx.try_send(frame) {
        eprintln!("Buffer full, dropping frame...");
    }

    return 0; // or appropriate status
}

unsafe extern "C" fn custom_port_get_frame(
    _port: *mut pjsua::pjmedia_port,
    frame: *mut pjsua::pjmedia_frame,
) -> pjsua::pj_status_t {
    println!(
        "custom_port_get_frame: frame buffer size: {:?}",
        (*frame).size
    );

    return 0; // or appropriate status
}

unsafe extern "C" fn custom_port_on_destroy(port: *mut pjsua::pjmedia_port) -> pjsua::pj_status_t {
    //base.port_data.pdata

    let _port: Box<MediaPortData> = Box::from_raw((*port).port_data.pdata as *mut MediaPortData);

    eprintln!("custom_port_on_destroy");
    return 0; // or appropriate status
}

struct MediaPortData {
    frames_tx: tokio_mpsc::Sender<Frame>,
    sample_rate: u32,
    channels_count: usize,
}

pub struct CustomSinkMediaPort<'a> {
    base: Box<pjsua::pjmedia_port>,
    _format: Box<pjsua::pjmedia_format>,
    _name: PjString<'a>,
}

pub struct CustomSinkMediaPortRx {
    frames_rx: tokio_mpsc::Receiver<Frame>,
}

impl CustomSinkMediaPortRx {
    pub async fn recv(&mut self) -> Option<Frame> {
        self.frames_rx.recv().await
    }
}

use crate::pjsua_softphone_api::PjsuaInstanceStarted;

use super::next_num;

impl<'a> CustomSinkMediaPort<'a> {
    pub fn new(
        sample_rate: u32,
        channels_count: usize,
        samples_per_frame: usize,
        mem_pool: &'a PjsuaMemoryPool,
    ) -> Result<(Self, CustomSinkMediaPortRx), PjsuaError> {
        let mut base: Box<pjsua::pjmedia_port> = Box::new(unsafe { std::mem::zeroed() });

        let name = PjString::alloc(format!("CustomSinkMediaPort_{}", next_num()), &mem_pool);

        let format = Box::new(pjmedia_api::port_format(
            sample_rate,
            channels_count,
            samples_per_frame as _,
        )?);

        let port_info = unsafe { pjmedia_api::port_info(format.as_ref(), name.as_ref()) };

        base.put_frame = Some(custom_port_put_frame);
        base.get_frame = Some(custom_port_get_frame);

        base.info = port_info?;

        let (frames_tx, frames_rx) = tokio_mpsc::channel(512);

        base.port_data.pdata = Box::into_raw(Box::new(MediaPortData {
            frames_tx,
            sample_rate,
            channels_count,
        })) as *mut _;

        base.on_destroy = Some(custom_port_on_destroy);

        Ok((
            CustomSinkMediaPort {
                base,
                _name: name,
                _format: format,
            },
            CustomSinkMediaPortRx { frames_rx },
        ))
    }

    pub(crate) fn add(
        self,
        mem_pool: &'a PjsuaMemoryPool,
        instance_started: &'a PjsuaInstanceStarted,
    ) -> Result<CustomSinkMediaPortAdded<'a>, PjsuaError> {
        CustomSinkMediaPortAdded::new(self, mem_pool, instance_started)
    }
}

pub struct CustomSinkMediaPortAdded<'a> {
    base: Box<pjsua::pjmedia_port>,
    _pjsua_instance: &'a PjsuaInstanceStarted,
    port_slot: pjsua::pjsua_conf_port_id,
}

impl<'a> CustomSinkMediaPortAdded<'a> {
    pub(crate) fn new(
        media_port: CustomSinkMediaPort<'a>,
        mem_pool: &'a PjsuaMemoryPool,
        pjsua_instance: &'a PjsuaInstanceStarted,
    ) -> Result<Self, PjsuaError> {
        let mut base = media_port.base;
        let mut port_slot = pjsua::pjsua_conf_port_id::default();

        unsafe {
            let status =
                pjsua::pjsua_conf_add_port(mem_pool.raw_handle(), base.as_mut(), &mut port_slot);
            get_error_as_result(status)?;
            eprintln!("added port to conf bridge: {:?}", port_slot);
        }

        base.put_frame = Some(custom_port_put_frame);

        Ok(CustomSinkMediaPortAdded {
            base,
            _pjsua_instance: pjsua_instance,
            port_slot,
        })
    }

    pub fn port_slot(&self) -> i32 {
        self.port_slot
    }
}

impl<'a> Drop for CustomSinkMediaPortAdded<'a> {
    fn drop(&mut self) {
        unsafe {
            eprintln!("removing port from conf bridge: {:?}", self.port_slot);
            let status = pjsua::pjsua_conf_remove_port(self.port_slot);
            get_error_as_result(status).unwrap();
        }

        let status = unsafe { pjsua::pjmedia_port_destroy(self.base.as_mut()) };
        get_error_as_result(status).unwrap();
    }
}

use futures::Stream;

impl Stream for CustomSinkMediaPortRx {
    type Item = Frame;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Frame>> {
        self.frames_rx.poll_recv(cx)
    }
}
