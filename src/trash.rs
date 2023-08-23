#[allow(non_camel_case_types)]
mod example {

    use std::{
        alloc::{alloc, dealloc, Layout, LayoutError},
        mem::align_of,
        mem::size_of,
        ptr::NonNull,
    };

    pub struct ByteBuffer<T> {
        ptr: Option<NonNull<T>>,
        len: usize,
    }

    enum AllocError {
        AllocationFailed,
        LayoutError(LayoutError),
    }

    impl From<LayoutError> for AllocError {
        fn from(err: LayoutError) -> Self {
            AllocError::LayoutError(err)
        }
    }

    //    impl<T> ByteBuffer<T> {
    //        #[allow(dead_code)]
    //        fn new(len: usize) -> Result<Self, AllocError> {
    //            let layout = Layout::from_size_align(size_of::<T>() * len, align_of::<T>())?;
    //
    //            match unsafe { alloc(layout) } {
    //                ptr if !ptr.is_null() => Ok(ByteBuffer {
    //                    ptr: NonNull::new(ptr),
    //                    len,
    //                }),
    //                _ => Err(AllocError::AllocationFailed),
    //            }
    //        }
    //    }
    //
    //    impl<T> Drop for ByteBuffer<T> {
    //        fn drop(&mut self) {
    //            if let Some(mut ptr) = self.ptr {
    //                unsafe {
    //                    dealloc(
    //                        ptr.as_mut(),
    //                        Layout::from_size_align_unchecked(
    //                            size_of::<T>() * self.len,
    //                            align_of::<T>(),
    //                        ),
    //                    );
    //                }
    //            }
    //        }
    //    }

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct pj_str_t {
        pub ptr: *mut ::std::os::raw::c_char,
        pub slen: pj_ssize_t,
    }

    pub type pj_ssize_t = ::std::os::raw::c_long;

    pub struct PjString(pub Vec<u8>);

    impl PjString {
        #[allow(dead_code)]
        unsafe fn copy_from_pj_str(pj_str: &pj_str_t) -> Self {
            let len = pj_str.slen as usize;
            let mut buffer = Vec::with_capacity(len);

            let pj_str_ptr = pj_str.ptr as *const u8;
            let pj_str_slice = std::slice::from_raw_parts(pj_str_ptr, len); //unsafe!

            let buffer_slice = buffer.as_mut_slice();

            buffer_slice.copy_from_slice(pj_str_slice);

            PjString(buffer)
        }
    }

    //now create some mapping pj_str_t -> PjString

    //example struct to automatically convert

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct simple_pj_c_type {
        some_str1: pj_str_t,
        some_foo_value: pj_str_t,
    }

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct nested_pj_c_type {
        some_str1: pj_str_t,

        simple_pj_c_type: simple_pj_c_type,
    }

    //sth automatically implement these types

    //gen
    pub struct SimplePjRustType {
        some_str1: PjString,
        some_foo_value: PjString,
    }

    pub struct NestedPjRustType {
        some_str1: PjString,
        simple_pj_c_type: SimplePjRustType,
    }

    impl SimplePjRustType {
        unsafe fn new(pj_c_type: &simple_pj_c_type) -> Self {
            SimplePjRustType {
                some_str1: PjString::copy_from_pj_str(&pj_c_type.some_str1),
                some_foo_value: PjString::copy_from_pj_str(&pj_c_type.some_foo_value),
            }
        }
    }

    impl NestedPjRustType {
        unsafe fn new(pj_c_type: &nested_pj_c_type) -> Self {
            NestedPjRustType {
                some_str1: PjString::copy_from_pj_str(&pj_c_type.some_str1),
                simple_pj_c_type: SimplePjRustType::new(&pj_c_type.simple_pj_c_type),
            }
        }
    }
    //gen
}
