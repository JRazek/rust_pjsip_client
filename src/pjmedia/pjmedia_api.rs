use crate::error::get_error_as_result;
use crate::error::PjsuaError;
use std::sync::atomic::AtomicU32;

const BITS_PER_SAMPLE: u32 = 16;

pub(super) fn perform_pjmedia_format_checks_zero_division(
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

pub(super) fn sample_duration(sample_rate: u32, channels_count: usize) -> std::time::Duration {
    let sample_time_usec = 1_000_000 / channels_count as u64 / sample_rate as u64;

    std::time::Duration::from_micros(sample_time_usec)
}

fn rand_signature() -> u32 {
    static mut SIGNATURE: AtomicU32 = AtomicU32::new(0);

    unsafe { SIGNATURE.fetch_add(1, std::sync::atomic::Ordering::SeqCst) }
}

pub unsafe fn port_info(
    format: *const pjsua::pjmedia_format,
    name: *const pjsua::pj_str_t,
) -> Result<pjsua::pjmedia_port_info, PjsuaError> {
    let mut port_info: pjsua::pjmedia_port_info = unsafe { std::mem::zeroed() };

    let signature = rand_signature();

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

pub(super) fn port_format(
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

pub struct Frame {
    pub data: Box<[u8]>,
    pub time: std::time::Duration,
}

impl Frame {
    pub(crate) unsafe fn from_raw_frame(
        frame_raw: &pjsua::pjmedia_frame,
        sample_rate: u32,
        channels_count: usize,
    ) -> Result<Self, PjsuaError> {
        type SampleType = u8;

        let buffer_size = frame_raw.size / std::mem::size_of::<SampleType>() as usize;

        let frame_data =
            unsafe { std::slice::from_raw_parts(frame_raw.buf as *const SampleType, buffer_size) };
        let frame_data = Box::from_iter(frame_data.iter().cloned());

        let timestamp0: pjsua::pj_timestamp = unsafe { std::mem::zeroed() };

        let samples_elapsed = unsafe { get_samples_diff(timestamp0, frame_raw.timestamp) };

        Ok(Frame {
            data: frame_data,
            time: samples_elapsed * sample_duration(sample_rate, channels_count),
        })
    }

    pub(crate) fn new(data: Box<[u8]>, time: std::time::Duration) -> Self {
        Frame { data, time }
    }
}

pub(crate) unsafe fn get_samples_diff(
    timestamp1: pjsua::pj_timestamp,
    timestamp2: pjsua::pj_timestamp,
) -> u32 {
    let samples_diff = pjsua::pj_elapsed_nanosec(&timestamp1, &timestamp2);

    samples_diff
}
