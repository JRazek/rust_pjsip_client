#![feature(core_intrinsics)]
#![feature(iter_next_chunk)]
#![feature(iter_array_chunks)]

pub mod error;
pub mod pj_types;
pub mod pjmedia;
pub mod pjsua_account_config;
pub mod pjsua_call;
pub mod pjsua_config;
pub mod pjsua_memory_pool;
pub mod pjsua_softphone_api;
pub mod tokio_utils;
pub mod transport;
