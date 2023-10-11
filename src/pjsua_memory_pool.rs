use std::ffi::CStr;
use std::ptr::write;

#[derive(Debug)]
pub struct PjsuaMemoryPool {
    pjsua_pool: *mut pjsua::pj_pool_t,
}

pub struct PoolBuffer<'a, T: Sized + Default> {
    pool_buffer: &'a mut [T],
    objects_count: usize,
    _mem_pool: &'a PjsuaMemoryPool,
}

impl<'a, T: Sized + Default> PoolBuffer<'a, T> {
    pub fn len(&self) -> usize {
        self.objects_count
    }
}

impl<'a, T: Sized + Default> std::ops::Index<usize> for PoolBuffer<'a, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.pool_buffer[index]
    }
}

impl<'a, T: Sized + Default> AsMut<T> for PoolBuffer<'a, T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.pool_buffer[0]
    }
}

impl PjsuaMemoryPool {
    pub fn new(init_size: usize, increment_size: usize) -> Option<PjsuaMemoryPool> {
        let name = CStr::from_bytes_with_nul(b"pjsua_buffer_rust\0").unwrap();

        let pool = unsafe { pjsua::pjsua_pool_create(name.as_ptr(), init_size, increment_size) };

        match pool {
            pool if pool.is_null() => None,
            pool => Some(PjsuaMemoryPool { pjsua_pool: pool }),
        }
    }

    pub fn raw_handle(&self) -> *mut pjsua::pj_pool_t {
        self.pjsua_pool
    }

    pub fn alloc<'a, T: Sized + Default>(&'a self, objects_count: usize) -> PoolBuffer<'a, T> {
        unsafe {
            let elem_size = std::mem::size_of::<T>();

            let buffer = pjsua::pj_pool_calloc(self.pjsua_pool, objects_count, elem_size) as *mut T;

            write(buffer, T::default());

            let pool_buffer =
                std::slice::from_raw_parts_mut::<'a>(buffer, objects_count * elem_size);

            PoolBuffer {
                pool_buffer,
                objects_count,
                _mem_pool: self,
            }
        }
    }
}

impl Drop for PjsuaMemoryPool {
    fn drop(&mut self) {
        unsafe {
            pjsua::pj_pool_release(self.pjsua_pool);
        }
    }
}
