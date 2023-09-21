use super::error::PjsuaError;
use super::pjsua_memory_pool::PjsuaMemoryPool;
use crate::error::ffi_assert_res;
use crate::error::get_error_as_result;

use super::pj_types::PjString;

use super::pj_types::Frame;

use std::sync::atomic::AtomicU32;

use tokio::sync::mpsc as tokio_mpsc;

use crate::ffi_assert;

fn perform_pjmedia_format_checks_zero_division(
    samples_per_frame: u32,
    audio_format_detail: &pjsua::pjmedia_audio_format_detail,
) -> Result<(), PjsuaError> {
    let port_ptime = samples_per_frame / audio_format_detail.channel_count * 1000
        / audio_format_detail.clock_rate;

    if port_ptime == 0 {
        let message = format!("samples_per_frame is too low! port_ptime = samples_per_frame / channel_count * 1000 / clock_rate is 0! \
                              Reference: https://github.com/pjsip/pjproject/blob/7ff31e311373dc81174a5cb24698da5377885897/pjmedia/src/pjmedia/conference.c#L265-L389. \
                              Division by zero happens at conference.c:387 \
                                          if (conf_ptime % port_ptime)") ;

        return Err(PjsuaError { code: -1, message });
    }

    Ok(())
}

pub(crate) fn sample_duration(sample_rate: u32, channels_count: usize) -> std::time::Duration {
    let sample_time_usec = 1_000_000 / channels_count as u64 / sample_rate as u64;

    std::time::Duration::from_micros(sample_time_usec)
}

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
    let bit_info = unsafe { (*frame).bit_info };

    //    ffi_assert!(bit_info == 0);
//    eprintln!("bit_info: {:?}", bit_info);
    ffi_assert!(frame_type == pjsua::pjmedia_frame_type_PJMEDIA_FRAME_TYPE_AUDIO);

    let frame = ffi_assert_res(Frame::new(&*frame, sample_rate, channels_count));

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

use super::pjsua_softphone_api::PjsuaInstanceStarted;

const BITS_PER_SAMPLE: u32 = 16;

impl<'a> CustomSinkMediaPort<'a> {
    pub fn new(
        sample_rate: u32,
        channels_count: usize,
        samples_per_frame: u32,
        mem_pool: &'a PjsuaMemoryPool,
    ) -> Result<(Self, CustomSinkMediaPortRx), PjsuaError> {
        let mut base: Box<pjsua::pjmedia_port> = Box::new(unsafe { std::mem::zeroed() });

        let name = PjString::alloc("CustomMediaPort", &mem_pool);

        let format = Box::new(Self::port_format(
            sample_rate,
            channels_count,
            samples_per_frame,
        )?);

        let port_info = unsafe { Self::port_info(format.as_ref(), name.as_ref()) };

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

    fn rand_signature() -> u32 {
        static mut SIGNATURE: AtomicU32 = AtomicU32::new(0);

        unsafe { SIGNATURE.fetch_add(1, std::sync::atomic::Ordering::SeqCst) }
    }

    unsafe fn port_info(
        format: *const pjsua::pjmedia_format,
        name: *const pjsua::pj_str_t,
    ) -> Result<pjsua::pjmedia_port_info, PjsuaError> {
        let mut port_info: pjsua::pjmedia_port_info = unsafe { std::mem::zeroed() };

        let signature = Self::rand_signature();

        //TODO: research the output format.
        //https://github.com/chakrit/pjsip/blob/b0af6c8fc8ed97bb03d3afa4ab42c24f46a9212b/pjmedia/src/pjmedia/port.c#L33
        //https://github.com/pjsip/pjproject/blob/01d37bf15a9121e6e78afe41a5c3ef4fa2ae3308/pjsip-apps/src/samples/playsine.c#L140C5-L140C27
        unsafe {
            let status = get_error_as_result(pjsua::pjmedia_port_info_init2(
                &mut port_info,
                name,
                signature,
                pjsua::pjmedia_dir_PJMEDIA_DIR_ENCODING_DECODING,
                format,
            ));

            status?;
        }

        eprintln!("fmt.type: {}", port_info.fmt.type_);
        eprintln!("fmt.type_detail: {}", port_info.fmt.detail_type);

        Ok(port_info)
    }

    pub(crate) fn port_format(
        sample_rate: u32,
        channels_count: usize,
        samples_per_frame: u32,
    ) -> Result<pjsua::pjmedia_format, PjsuaError> {
        let mut format: pjsua::pjmedia_format = unsafe { std::mem::zeroed() };

        const FORMAT_ID: u32 = pjsua::pjmedia_format_id_PJMEDIA_FORMAT_L16;

        format.id = FORMAT_ID;
        format.type_ = pjsua::pjmedia_type_PJMEDIA_TYPE_AUDIO;
        format.detail_type = pjsua::pjmedia_format_detail_type_PJMEDIA_FORMAT_DETAIL_AUDIO;

        let frame_time_usec =
            samples_per_frame as u64 * 1_000_000 / channels_count as u64 / sample_rate as u64;

        let avg_bps = sample_rate * channels_count as u32 * BITS_PER_SAMPLE;

        //TODO: value that is set here, is not visible in
        //format.det.aud!
        unsafe {
            let det = &mut format.det.aud;

            det.clock_rate = sample_rate;
            det.channel_count = channels_count as u32;
            det.bits_per_sample = BITS_PER_SAMPLE;
            det.frame_time_usec = frame_time_usec as u32;
            det.avg_bps = avg_bps;
            det.max_bps = avg_bps;

            perform_pjmedia_format_checks_zero_division(samples_per_frame, &det)?;
        }

        Ok(format)
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
    pjsua_instance: &'a PjsuaInstanceStarted,
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
            pjsua_instance,
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
