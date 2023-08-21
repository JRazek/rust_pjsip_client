use std::ffi::CString;

#[derive(Debug, Clone)]
pub struct Error {
    pub code: pjsua::pj_status_t,
    pub message: String,
}

impl From<pjsua::pj_status_t> for Error {
    fn from(code: pjsua::pj_status_t) -> Self {
        let message = unsafe {
            let mut buffer = [0u8; 256];
            let ptr = buffer.as_mut_ptr() as *mut i8;

            _ = pjsua::pj_strerror(code, ptr, buffer.len() as pjsua::pj_size_t);

            let str = CString::from_raw(ptr)
                .into_string()
                .expect("CString::into_string failed!");

            str
        };

        Error { code, message }
    }
}

pub fn get_error_as_option(code: pjsua::pj_status_t) -> Option<Error> {
    const PJSUA_SUCCESS: i32 = pjsua::pj_constants__PJ_SUCCESS as i32;
    match code {
        PJSUA_SUCCESS => None,
        _ => Some(Error::from(code)),
    }
}

pub fn get_error_as_result(code: pjsua::pj_status_t) -> Result<(), Error> {
    const PJSUA_SUCCESS: i32 = pjsua::pj_constants__PJ_SUCCESS as i32;
    match code {
        PJSUA_SUCCESS => Ok(()),
        _ => Err(Error::from(code)),
    }
}

pub fn map_option_to_result<T>(error: Option<Error>) -> Result<(), Error> {
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
