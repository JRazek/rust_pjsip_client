use std::ffi::CStr;
pub struct PjsuaMemoryPool {
    pjsua_pool: *mut pjsua::pj_pool_t,
}

impl PjsuaMemoryPool {
    pub fn new(init_size: usize, increment_size: usize) -> Option<PjsuaMemoryPool> {
        let name = CStr::from_bytes_with_nul(b"pjsua_buffer_rust\0").unwrap();

        let pool = unsafe {
            pjsua::pjsua_pool_create(name.as_ptr(), init_size as u64, increment_size as u64)
        };

        match pool {
            pool if pool.is_null() => None,
            pool => Some(PjsuaMemoryPool { pjsua_pool: pool }),
        }
    }
}

impl AsMut<pjsua::pj_pool_t> for PjsuaMemoryPool {
    fn as_mut(&mut self) -> &mut pjsua::pj_pool_t {
        unsafe { &mut *self.pjsua_pool }
    }
}

impl Drop for PjsuaMemoryPool {
    fn drop(&mut self) {
        unsafe {
            pjsua::pj_pool_release(self.pjsua_pool);
        }
    }
}
