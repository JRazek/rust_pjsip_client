use super::error::PjsuaError;
use super::pjsua_conf_bridge::ConfBridgeHandle;
use super::pjsua_memory_pool::PjsuaMemoryPool;
use crate::error::get_error_as_result;

use super::pj_types::PjString;

use super::pj_types::Frame;

use std::sync::atomic::AtomicU32;

use tokio::sync::mpsc as tokio_mpsc;

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

    let frame_data =
        unsafe { std::slice::from_raw_parts((*frame).buf as *const u8, (*frame).size as usize) };

    let frame_data = Box::from_iter(frame_data.iter().cloned());

    let media_port_data = unsafe { (*port).port_data.pdata as *mut MediaPortData };

    (*media_port_data)
        .frames_tx
        .try_send(Frame { data: frame_data })
        .unwrap();

    if count % 100 == 0 {
        println!("custom_port_put_frame: frame data: {:?}", frame_data);
    }

    return 0; // or appropriate status
}

unsafe extern "C" fn custom_port_get_frame(
    port: *mut pjsua::pjmedia_port,
    frame: *mut pjsua::pjmedia_frame,
) -> pjsua::pj_status_t {
    static mut COUNTER: AtomicU32 = AtomicU32::new(0);

    let count = unsafe { COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst) };

    if count % 100 == 0 {
        println!(
            "custom_port_get_frame: frame buffer size: {:?}",
            (*frame).size
        );
    }

    return 0; // or appropriate status
}

unsafe extern "C" fn custom_port_on_destroy(port: *mut pjsua::pjmedia_port) -> pjsua::pj_status_t {
    eprintln!("custom_port_on_destroy");
    return 0; // or appropriate status
}

struct MediaPortData {
    frames_tx: tokio_mpsc::Sender<Frame>,
}

pub struct CustomSinkMediaPort<'a> {
    base: Box<pjsua::pjmedia_port>,
    _name: PjString<'a>,
}

impl<'a> CustomSinkMediaPort<'a> {
    pub fn new(
        sample_rate: u32,
        channels_count: usize,
        samples_per_frame: u32,
        mem_pool: &'a PjsuaMemoryPool,
    ) -> Self {
        let mut base: Box<pjsua::pjmedia_port> = Box::new(unsafe { std::mem::zeroed() });

        let name = PjString::alloc("CustomMediaPort", &mem_pool);

        let port_info = Self::port_info(
            sample_rate,
            channels_count,
            samples_per_frame,
            name.as_ref(),
        );

        base.put_frame = Some(custom_port_put_frame);
        base.get_frame = Some(custom_port_get_frame);

        base.info = port_info;

        let (frames_tx, frames_rx) = tokio_mpsc::channel(100);

        base.port_data.pdata = Box::into_raw(Box::new(MediaPortData { frames_tx })) as *mut _;

        base.on_destroy = Some(custom_port_on_destroy);

        CustomSinkMediaPort { base, _name: name }
    }

    fn rand_signature() -> u32 {
        static mut SIGNATURE: AtomicU32 = AtomicU32::new(0);

        unsafe { SIGNATURE.fetch_add(1, std::sync::atomic::Ordering::SeqCst) }
    }

    fn port_info(
        sample_rate: u32,
        channels_count: usize,
        samples_per_frame: u32,
        name: &pjsua::pj_str_t,
    ) -> pjsua::pjmedia_port_info {
        let mut port_info: pjsua::pjmedia_port_info = unsafe { std::mem::zeroed() };

        let signature = Self::rand_signature();

        unsafe {
            pjsua::pjmedia_port_info_init(
                &mut port_info,
                name,
                signature,
                sample_rate,
                channels_count as u32,
                16,
                samples_per_frame,
            );
        }

        eprint!("fmt.type: {}", port_info.fmt.type_);
        eprint!("fmt.type_detail: {}", port_info.fmt.detail_type);

        port_info
    }

    pub(crate) fn add(
        self,
        mem_pool: &'a PjsuaMemoryPool,
        conf_bridge: &'a ConfBridgeHandle,
    ) -> Result<CustomSinkMediaPortAdded<'a>, PjsuaError> {
        CustomSinkMediaPortAdded::new(self, mem_pool, conf_bridge)
    }
}

pub struct CustomSinkMediaPortAdded<'a> {
    base: Box<pjsua::pjmedia_port>,
    _conf_bridge: &'a ConfBridgeHandle,
    port_slot: pjsua::pjsua_conf_port_id,
}

impl<'a> CustomSinkMediaPortAdded<'a> {
    pub(crate) fn new(
        media_port: CustomSinkMediaPort<'a>,
        mem_pool: &'a PjsuaMemoryPool,
        conf_bridge: &'a ConfBridgeHandle,
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
            _conf_bridge: conf_bridge,
            port_slot,
        })
    }

    pub fn port_slot(&self) -> i32 {
        self.port_slot
    }
}

impl<'a> Drop for CustomSinkMediaPortAdded<'a> {
    fn drop(&mut self) {
        let status = unsafe { pjsua::pjmedia_port_destroy(self.base.as_mut()) };
        get_error_as_result(status).unwrap();
    }
}
