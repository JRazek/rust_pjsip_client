use super::error::ffi_assert_res;
use super::error::get_error_as_result;
use std::cell::RefCell;
use std::ffi::CString;
use std::mem::MaybeUninit;
use std::ptr;
use std::sync::atomic::AtomicUsize;
use tokio::task::spawn_blocking as tokio_spawn_blocking;
use tokio::task::JoinHandle;

struct PjsuaThreadMeta {
    _thread_name: CString,
    _descriptor: pjsua::pj_thread_desc,
    _pj_thread_handle: *mut pjsua::pj_thread_t,

    _not_send_sync: std::marker::PhantomData<*const ()>,
}

impl PjsuaThreadMeta {
    fn register() -> PjsuaThreadMeta {
        static THREAD_NAME_COUNTER: AtomicUsize = AtomicUsize::new(0);

        let counter = THREAD_NAME_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let thread_name = CString::new(format!("pjsua_thread_tokio_{}", counter)).unwrap();

        let mut descriptor = unsafe {
            let descriptor = MaybeUninit::<pjsua::pj_thread_desc>::zeroed().assume_init();
            descriptor
        };

        let mut handle = ptr::null_mut();

        let status = unsafe {
            match pjsua::pj_thread_is_registered() != 0 {
                false => pjsua::pj_thread_register(
                    thread_name.as_ptr() as *const _,
                    descriptor.as_mut_ptr(),
                    &mut handle,
                ),
                true => pjsua::pj_constants__PJ_SUCCESS as i32,
            }
        };

        let status = get_error_as_result(status);

        ffi_assert_res(status);

        PjsuaThreadMeta {
            _thread_name: thread_name,
            _descriptor: descriptor,
            _pj_thread_handle: handle,
            _not_send_sync: std::marker::PhantomData,
        }
    }
}

//spawn_pjsua wont work! task may be scheduled on a different thread and the initalization is not
//performed!
//this however wont resume the task on the same thread, but it will be scheduled on the same thread
pub(crate) fn spawn_blocking_pjsua<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let f_wrapped = move || {
        thread_local! {
            static THREAD_META: RefCell<Option<PjsuaThreadMeta>> = RefCell::new(None);
        }

        THREAD_META.with(|thread_meta_opt| {
            *thread_meta_opt.borrow_mut() = Some(PjsuaThreadMeta::register());
            eprintln!(
                "spawn_blocking_pjsua: running on thread {:?}",
                std::thread::current().id()
            );
        });
        f()
    };

    tokio_spawn_blocking(f_wrapped)
}
