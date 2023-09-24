use super::pjsua_memory_pool::{PjsuaMemoryPool, PoolBuffer};

pub struct PjString<'a> {
    pj_str: pjsua::pj_str_t,
    _pool_buffer: PoolBuffer<'a, u8>,
}

impl<'a> PjString<'a> {
    pub fn alloc(string: impl AsRef<str>, mem_pool: &'a PjsuaMemoryPool) -> PjString<'a> {
        let string = string.as_ref();
        let mut pool_buffer = mem_pool.alloc::<u8>(string.len() + 1);

        let pj_str = pjsua::pj_str_t {
            ptr: pool_buffer.as_mut() as *mut _ as *mut std::os::raw::c_char,
            slen: pool_buffer.len() as pjsua::pj_ssize_t,
        };

        unsafe { std::ptr::copy(string.as_ptr(), pj_str.ptr as *mut u8, string.len()) };

        PjString {
            pj_str,
            _pool_buffer: pool_buffer,
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

pub use super::pjmedia::pjmedia_api::Frame;
