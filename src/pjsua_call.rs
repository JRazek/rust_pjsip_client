use crate::pjsua_softphone_api;

use super::error::{get_error_as_result, PjsuaError};
use super::pjmedia_bridge::PjmediaBridge;
use std::ptr;

use super::tokio_utils::spawn_blocking_pjsua;

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

#[derive(Debug)]
enum IncomingStatus {
    Answered,
    Rejected,
}

#[derive(Debug)]
enum CallStatus {
    InProgress,
    LocalHangup,
}

pub struct PjsuaIncomingCall<'a> {
    account_id: pjsua::pjsua_acc_id,
    call_id: pjsua::pjsua_call_id,
    status: Option<IncomingStatus>,
    _pjsua_instance_started: &'a pjsua_softphone_api::PjsuaInstanceStarted,
}

impl<'a> PjsuaIncomingCall<'a> {
    pub(crate) fn new(
        account_id: pjsua::pjsua_acc_id,
        call_id: pjsua::pjsua_call_id,
        pjsua_instance_started: &'a pjsua_softphone_api::PjsuaInstanceStarted,
    ) -> Self {
        Self {
            account_id,
            call_id,
            status: None,
            _pjsua_instance_started: pjsua_instance_started,
        }
    }

    pub async fn answer_ok<Sink: PjsuaSinkMediaPort<'a>>(
        mut self,
        sink: Sink,
    ) -> Result<PjsuaCall<'a, Sink>, PjsuaError> {
        self.status = Some(IncomingStatus::Answered);
        PjsuaCall::new(self, sink).await
    }

    pub async fn reject(mut self) -> Result<(), PjsuaError> {
        self.status = Some(IncomingStatus::Rejected);

        spawn_blocking_pjsua(move || {
            reject_incoming(self.call_id)?;

            Ok::<(), PjsuaError>(())
        })
        .await
        .unwrap()
    }
}

impl<'a> Drop for PjsuaIncomingCall<'a> {
    fn drop(&mut self) {
        if let Some(IncomingStatus::Rejected) = &self.status {
            reject_incoming(self.call_id).unwrap();
        }
    }
}

pub struct PjsuaCall<'a, Sink: PjsuaSinkMediaPort<'a>> {
    _account_id: pjsua::pjsua_acc_id,
    call_id: pjsua::pjsua_call_id,
    _phantom: std::marker::PhantomData<&'a Sink>,
    _sink: Sink,
    status: CallStatus,
    pjsua_instance_started: &'a pjsua_softphone_api::PjsuaInstanceStarted,
    on_call_state_changed_rx: tokio::sync::mpsc::Receiver<(pjsua::pjsua_call_id, State)>,
}

impl<'a, Sink: PjsuaSinkMediaPort<'a>> PjsuaCall<'a, Sink> {
    async fn new(
        incoming_call: PjsuaIncomingCall<'a>,
        sink: Sink,
    ) -> Result<PjsuaCall<'a, Sink>, PjsuaError> {
        let (state_changed_tx, state_changed_rx) = tokio::sync::mpsc::channel(3);
        let user_data = Box::new(cb_user_data::StateChangedUserData {
            on_state_changed_tx: state_changed_tx,
        });

        unsafe {
            eprintln!("Setting user data...");
            let status = pjsua::pjsua_call_set_user_data(
                incoming_call.call_id,
                Box::into_raw(user_data) as *mut std::ffi::c_void,
            );
            get_error_as_result(status)?;
        }

        spawn_blocking_pjsua(move || {
            accept_incoming(incoming_call.call_id)?;

            Ok::<(), PjsuaError>(())
        })
        .await
        .unwrap()?;

        let capture_media = Ok(Self {
            _account_id: incoming_call.account_id,
            call_id: incoming_call.call_id,
            _phantom: std::marker::PhantomData,
            _sink: sink,
            status: CallStatus::InProgress,
            pjsua_instance_started: incoming_call._pjsua_instance_started,
            on_call_state_changed_rx: state_changed_rx,
        });

        capture_media
    }

    pub async fn hangup(mut self) -> Result<(), RemoteAlreadyHangUpError> {
        spawn_blocking_pjsua(move || match hangup_call(self.call_id) {
            Ok(()) => Ok(()),
            Err(_) => Err(RemoteAlreadyHangUpError),
        })
        .await
        .unwrap()?;

        self.status = CallStatus::LocalHangup;

        Ok(())
    }

    pub async fn wait_for_state_change(&mut self) -> Result<State, PjsuaError> {
        let (call_id, state) = self
            .on_call_state_changed_rx
            .recv()
            .await
            .expect("this should not happen");

        assert_eq!(call_id, self.call_id);

        Ok(state)
    }
}

impl<'a, Sink: PjsuaSinkMediaPort<'a>> Drop for PjsuaCall<'a, Sink> {
    fn drop(&mut self) {
        if let CallStatus::InProgress = self.status {
            _ = hangup_call(self.call_id);
        }
    }
}

#[derive(Debug)]
pub struct RemoteAlreadyHangUpError;

impl std::fmt::Display for RemoteAlreadyHangUpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Remote already hang up")
    }
}

impl std::error::Error for RemoteAlreadyHangUpError {}

pub trait PjsuaSinkMediaPort<'a> {
    fn as_pjmedia_port(&mut self) -> *mut pjsua::pjmedia_port;
}

pub(crate) mod cb_user_data {
    use super::State;
    use tokio::sync::mpsc::Sender;

    #[allow(unused_parens)]
    pub(crate) type OnStateChangedSendData = (pjsua::pjsua_call_id, State);

    pub struct StateChangedUserData {
        pub(crate) on_state_changed_tx: Sender<OnStateChangedSendData>,
    }
}

#[derive(Debug)]
pub enum State {
    PjsipInvStateNull,
    PjsipInvStateCalling,
    PjsipInvStateIncoming,
    PjsipInvStateEarly,
    PjsipInvStateConnecting,
    PjsipInvStateConfirmed,
    PjsipInvStateDisconnected,
}

impl TryFrom<u32> for State {
    type Error = ();
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(State::PjsipInvStateNull),
            1 => Ok(State::PjsipInvStateCalling),
            2 => Ok(State::PjsipInvStateIncoming),
            3 => Ok(State::PjsipInvStateEarly),
            4 => Ok(State::PjsipInvStateConnecting),
            5 => Ok(State::PjsipInvStateConfirmed),
            6 => Ok(State::PjsipInvStateDisconnected),
            _ => Err(()),
        }
    }
}
