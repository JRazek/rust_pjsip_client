use super::pjsua_memory_pool::{PjsuaMemoryPool, PoolBuffer};

pub struct PjString<'a> {
    pj_str: pjsua::pj_str_t,
    pool_buffer: PoolBuffer<'a, u8>,
}

impl<'a> PjString<'a> {
    pub fn alloc(string: &str, mem_pool: &'a PjsuaMemoryPool) -> PjString<'a> {
        let mut pool_buffer = mem_pool.alloc::<u8>(string.len() + 1);

        let pj_str = pjsua::pj_str_t {
            ptr: pool_buffer.as_mut() as *mut _ as *mut std::os::raw::c_char,
            slen: pool_buffer.len() as pjsua::pj_ssize_t,
        };

        unsafe { std::ptr::copy(string.as_ptr(), pj_str.ptr as *mut u8, string.len()) };

        PjString {
            pj_str,
            pool_buffer,
        }
    }
}

impl<'a> AsRef<pjsua::pj_str_t> for PjString<'a> {
    fn as_ref(&self) -> &pjsua::pj_str_t {
        &self.pj_str
    }
}

impl<'a> AsMut<pjsua::pj_str_t> for PjString<'a> {
    fn as_mut(&mut self) -> &mut pjsua::pj_str_t {
        &mut self.pj_str
    }
}

pub struct Frame {
    data: Box<[u8]>,
    time: std::time::Duration,
}

impl TryFrom<&mut pjsua::pjmedia_frame> for Frame {
    type Error = ();

    fn try_from(frame_raw: &mut pjsua::pjmedia_frame) -> Result<Self, Self::Error> {
        let frame_data = unsafe {
            std::slice::from_raw_parts(frame_raw.buf as *const u8, frame_raw.size as usize)
        };

        let frame_data = Box::from_iter(frame_data.iter().cloned());

        //todo time

        Ok(Frame {
            data: frame_data,
            time: time_duration,
        })
    }
}
