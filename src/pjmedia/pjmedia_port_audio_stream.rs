use crate::error::get_error_as_result;
use crate::error::PjsuaError;
use crate::pjsua_memory_pool::PjsuaMemoryPool;

use crate::pj_types::PjString;

use crate::pj_types::Frame;

use tokio::sync::mpsc as tokio_mpsc;

use super::pjmedia_api;

use crate::pjsua_softphone_api::PjsuaInstanceStarted;

use std::panic::catch_unwind;

use crate::error::ffi_assert_res;

use super::next_num;

use std::ptr;

unsafe extern "C" fn custom_port_get_frame(
    port: *mut pjsua::pjmedia_port,
    tx_frame: *mut pjsua::pjmedia_frame,
) -> pjsua::pj_status_t {
    let res = catch_unwind(|| {
        let media_port_data = unsafe { (*port).port_data.pdata as *mut MediaPortData };
        let _sample_rate = (*media_port_data).sample_rate;
        let _channels_count = (*media_port_data).channels_count;

        let frame_type = unsafe { (*tx_frame).type_ };

        let tx_frame = &mut *tx_frame;

        let tx_frame_data: &mut [u8] =
            std::slice::from_raw_parts_mut(tx_frame.buf as *mut _, tx_frame.size);

        if let pjsua::pjmedia_frame_type_PJMEDIA_FRAME_TYPE_AUDIO = frame_type {
            match (*media_port_data).frames_rx.try_recv() {
                Ok(rx_frame) => {
                    assert!(rx_frame.data.len() == tx_frame_data.len());

                    tx_frame_data.copy_from_slice(rx_frame.data.as_ref());
                    tx_frame.timestamp.u64_ = rx_frame.time.as_micros() as u64;
                }
                _ => {
                    ptr::write_bytes(tx_frame.buf, 0, tx_frame.size);
                    tx_frame.timestamp.u64_ = 0;
                    tx_frame.size = 0;
                    tx_frame.type_ = pjsua::pjmedia_frame_type_PJMEDIA_FRAME_TYPE_NONE;
                }
            }
        }
    });

    ffi_assert_res(res);

    return 0; // or appropriate status
}

unsafe extern "C" fn custom_port_on_destroy(port: *mut pjsua::pjmedia_port) -> pjsua::pj_status_t {
    let _port: Box<MediaPortData> = Box::from_raw((*port).port_data.pdata as *mut MediaPortData);

    eprintln!("custom_port_on_destroy");
    return 0; // or appropriate status
}

struct MediaPortData {
    frames_rx: tokio_mpsc::Receiver<Frame>,

    sample_rate: u32,
    channels_count: usize,
}

pub struct CustomStreamMediaPort<'a> {
    base: Box<pjsua::pjmedia_port>,
    _format: Box<pjsua::pjmedia_format>,
    _name: PjString<'a>,
}

pub struct CustomStreamMediaPortTx {
    frames_tx: tokio_mpsc::Sender<Frame>,
    bits_per_sample: usize,
    samples_per_frame: usize,
}

use super::pjmedia_api::SendError;

use std::intrinsics::unlikely as unlikely_branch;

impl CustomStreamMediaPortTx {
    pub fn bits_per_sample(&self) -> usize {
        self.bits_per_sample
    }
    pub fn samples_per_frame(&self) -> usize {
        self.samples_per_frame
    }

    pub async fn send(&self, mut frame: Frame) -> Result<(), SendError> {
        assert!(self.bits_per_sample % 8 == 0);
        let bytes_in_sample = self.bits_per_sample / 8;

        if unlikely_branch(frame.data.len() / bytes_in_sample > self.samples_per_frame) {
            return Err(SendError::InvalidSizeFrameError(frame));
        }

        if unlikely_branch(frame.data.len() / bytes_in_sample < self.samples_per_frame) {
            let mut frame_resized = frame.data.into_vec();

            frame_resized.resize(self.samples_per_frame * bytes_in_sample, 0);

            frame = Frame {
                data: frame_resized.into_boxed_slice(),
                time: frame.time,
            };
        }

        match self.frames_tx.send(frame).await {
            Ok(_) => Ok(()),
            Err(e) => Err(SendError::TokioSendError(e)),
        }
    }
}

impl<'a> CustomStreamMediaPort<'a> {
    pub fn new(
        sample_rate: u32,
        channels_count: usize,
        samples_per_frame: usize,
        mem_pool: &'a PjsuaMemoryPool,
    ) -> Result<(Self, CustomStreamMediaPortTx), PjsuaError> {
        let mut base: Box<pjsua::pjmedia_port> = Box::new(unsafe { std::mem::zeroed() });

        let name = PjString::alloc(format!("CustomStreamMediaPort_{}", next_num()), &mem_pool);

        let format = Box::new(pjmedia_api::port_format(
            sample_rate,
            channels_count,
            samples_per_frame,
        )?);

        let port_info = unsafe { pjmedia_api::port_info(format.as_ref(), name.as_ref()) };

        base.get_frame = Some(custom_port_get_frame);

        base.info = port_info?;

        let (frames_tx, frames_rx) = tokio_mpsc::channel(512);

        base.port_data.pdata = Box::into_raw(Box::new(MediaPortData {
            frames_rx,
            sample_rate,
            channels_count,
        })) as *mut _;

        base.on_destroy = Some(custom_port_on_destroy);

        Ok((
            CustomStreamMediaPort {
                base,
                _name: name,
                _format: format,
            },
            CustomStreamMediaPortTx {
                frames_tx,
                samples_per_frame: samples_per_frame as usize,
                bits_per_sample: pjmedia_api::BITS_PER_SAMPLE,
            },
        ))
    }

    pub(crate) fn add(
        self,
        mem_pool: &'a PjsuaMemoryPool,
        instance_started: &'a PjsuaInstanceStarted,
    ) -> Result<CustomStreamMediaPortAdded<'a>, PjsuaError> {
        CustomStreamMediaPortAdded::new(self, mem_pool, instance_started)
    }
}

pub struct CustomStreamMediaPortAdded<'a> {
    base: Box<pjsua::pjmedia_port>,
    _pjsua_instance: &'a PjsuaInstanceStarted,
    port_slot: pjsua::pjsua_conf_port_id,
}

impl<'a> CustomStreamMediaPortAdded<'a> {
    pub(crate) fn new(
        media_port: CustomStreamMediaPort<'a>,
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

        base.put_frame = Some(custom_port_get_frame);

        Ok(CustomStreamMediaPortAdded {
            base,
            _pjsua_instance: pjsua_instance,
            port_slot,
        })
    }

    pub fn port_slot(&self) -> i32 {
        self.port_slot
    }
}

impl<'a> Drop for CustomStreamMediaPortAdded<'a> {
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
