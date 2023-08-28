use super::error::{get_error_as_result, PjsuaError};
use super::pjmedia_bridge::PjmediaBridge;
use std::ptr;

fn accept_incoming(call_id: pjsua::pjsua_call_id) -> Result<(), PjsuaError> {
    unsafe {
        let status = pjsua::pjsua_call_answer(call_id, 200, ptr::null(), ptr::null());
        get_error_as_result(status)?;
    }

    Ok(())
}

fn reject_incoming(call_id: pjsua::pjsua_call_id) -> Result<(), PjsuaError> {
    unsafe {
        let status = pjsua::pjsua_call_hangup(call_id, 486, ptr::null(), ptr::null());
        get_error_as_result(status)?;
    }

    Ok(())
}

fn hangup_call(call_id: pjsua::pjsua_call_id) -> Result<(), PjsuaError> {
    unsafe {
        let status = pjsua::pjsua_call_hangup(call_id, 200, ptr::null(), ptr::null());
        get_error_as_result(status)?;
    }

    Ok(())
}

fn connect_ports(
    call_id: pjsua::pjsua_call_id,
    sink_port: pjsua::pjmedia_port,
    source_port: pjsua::pjmedia_port,
) -> Result<(), PjsuaError> {
    unsafe {
        //        let status = pjsua::pjmedia_conf_connect_port(sink_port, source_port);
        //        get_error_as_result(status)?;
    }

    Ok(())
}

#[derive(Debug)]
enum IncomingStatus {
    Answered,
    Rejected,
}

#[derive(Debug)]
pub struct PjsuaIncomingCall {
    account_id: pjsua::pjsua_acc_id,
    call_id: pjsua::pjsua_call_id,
    status: Option<IncomingStatus>,
}

impl PjsuaIncomingCall {
    pub(crate) fn new(account_id: pjsua::pjsua_acc_id, call_id: pjsua::pjsua_call_id) -> Self {
        Self {
            account_id,
            call_id,
            status: None,
        }
    }

    pub fn answer_ok<'a, Sink: PjsuaSinkMediaPort<'a>>(
        mut self,
        sink: Sink,
    ) -> Result<PjsuaCall<'a, Sink>, PjsuaError> {
        self.status = Some(IncomingStatus::Answered);
        PjsuaCall::new(self, sink)
    }

    pub fn reject(mut self) -> Result<(), PjsuaError> {
        self.status = Some(IncomingStatus::Rejected);
        reject_incoming(self.call_id)?;

        Ok(())
    }
}

impl Drop for PjsuaIncomingCall {
    fn drop(&mut self) {
        if let Some(IncomingStatus::Rejected) = &self.status {
            reject_incoming(self.call_id).unwrap();
        }
    }
}

#[derive(Debug)]
pub struct PjsuaCall<'a, Sink: PjsuaSinkMediaPort<'a>> {
    _account_id: pjsua::pjsua_acc_id,
    call_id: pjsua::pjsua_call_id,
    _phantom: std::marker::PhantomData<&'a Sink>,
    _sink: Sink,
}

impl<'a, Sink: PjsuaSinkMediaPort<'a>> PjsuaCall<'a, Sink> {
    fn new(incoming_call: PjsuaIncomingCall, sink: Sink) -> Result<Self, PjsuaError> {
        accept_incoming(incoming_call.call_id)?;

        let capture_media = Ok(Self {
            _account_id: incoming_call.account_id,
            call_id: incoming_call.call_id,
            _phantom: std::marker::PhantomData,
            _sink: sink,
        });

        capture_media
    }

    pub fn hangup(self) -> Result<(), PjsuaError> {
        hangup_call(self.call_id)?;

        Ok(())
    }
}

pub trait PjsuaSinkMediaPort<'a> {
    fn as_pjmedia_port(&mut self) -> *mut pjsua::pjmedia_port;
}
