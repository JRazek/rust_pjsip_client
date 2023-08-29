use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct PjsuaError {
    pub code: pjsua::pj_status_t,
    pub message: String,
}

impl std::fmt::Display for PjsuaError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "PjsuaError: code: {}, message: {}",
            self.code, self.message
        )
    }
}

impl std::error::Error for PjsuaError {}

impl From<pjsua::pj_status_t> for PjsuaError {
    fn from(code: pjsua::pj_status_t) -> Self {
        let message = unsafe {
            let mut buffer = [0u8; 256];
            let ptr = buffer.as_mut_ptr() as *mut i8;

            let pjstr = pjsua::pj_strerror(code, ptr, buffer.len() as pjsua::pj_size_t);

            let str =
                String::from_iter(buffer.iter().take(pjstr.slen as usize).map(|&c| c as char));

            str
        };

        PjsuaError { code, message }
    }
}

pub fn get_error_as_option(code: pjsua::pj_status_t) -> Option<PjsuaError> {
    const PJSUA_SUCCESS: i32 = pjsua::pj_constants__PJ_SUCCESS as i32;
    match code {
        PJSUA_SUCCESS => None,
        _ => Some(PjsuaError::from(code)),
    }
}

pub fn get_error_as_result(code: pjsua::pj_status_t) -> Result<(), PjsuaError> {
    const PJSUA_SUCCESS: i32 = pjsua::pj_constants__PJ_SUCCESS as i32;
    match code {
        PJSUA_SUCCESS => Ok(()),
        _ => Err(PjsuaError::from(code)),
    }
}

pub fn map_option_to_result<T>(error: Option<PjsuaError>) -> Result<(), PjsuaError> {
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[macro_export]
macro_rules! ffi_assert {
    ($cond:expr) => {
        if !$cond {
            let backtrace = std::backtrace::Backtrace::capture();
            eprintln!("Assertion failed: {}", stringify!($cond));
            eprintln!("{:?}", backtrace);
            std::process::exit(1);
        }
    };
    ($cond:expr, $($arg:tt)*) => {
        if !$cond {
            let backtrace = std::backtrace::Backtrace::capture();
            eprintln!("Assertion failed: {}", stringify!($cond));
            eprintln!("{:?}", backtrace);
            eprintln!($($arg)*);

            std::process::exit(1);
        }
    };
}

pub fn ffi_assert_res<T: Debug, E: Debug>(res: Result<T, E>) -> T {
    ffi_assert!(res.is_ok(), "{:?}", res);

    res.unwrap()
}
