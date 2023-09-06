use crate::pjsua_softphone_api;

use super::error::{get_error_as_result, PjsuaError};
use std::ptr;

use super::tokio_utils::spawn_blocking_pjsua;

use std::mem::MaybeUninit;

use super::pjsua_memory_pool::PjsuaMemoryPool;

use super::pjsua_sink_buffer_media_port::{
    PjsuaSinkBufferMediaPort, PjsuaSinkBufferMediaPortConnected,
};

pub(crate) mod answer_code {
    pub trait AnswerCode {
        fn as_u32(&self) -> u32;
    }

    pub struct SessionProgress;
    impl AnswerCode for SessionProgress {
        fn as_u32(&self) -> u32 {
            183
        }
    }

    pub struct Ok;
    impl AnswerCode for Ok {
        fn as_u32(&self) -> u32 {
            200
        }
    }
}

fn accept_incoming(
    call_id: pjsua::pjsua_call_id,
    answer_state: impl answer_code::AnswerCode,
) -> Result<(), PjsuaError> {
    unsafe {
        let status =
            pjsua::pjsua_call_answer(call_id, answer_state.as_u32(), ptr::null(), ptr::null());

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
    if !is_call_active(call_id) {
        return Ok(());
    }
    unsafe {
        let status = pjsua::pjsua_call_hangup(call_id, 200, ptr::null(), ptr::null());
        get_error_as_result(status)?;
    }

    Ok(())
}

#[allow(dead_code)]
fn get_call_info(call_id: pjsua::pjsua_call_id) -> Result<pjsua::pjsua_call_info, PjsuaError> {
    let call_info = unsafe {
        let mut call_info = MaybeUninit::<pjsua::pjsua_call_info>::zeroed().assume_init();
        let status = pjsua::pjsua_call_get_info(call_id, &mut call_info);
        get_error_as_result(status)?;

        call_info
    };

    Ok(call_info)
}

fn is_call_active(call_id: pjsua::pjsua_call_id) -> bool {
    let active = unsafe { pjsua::pjsua_call_is_active(call_id) };

    active != 0
}

#[derive(Debug)]
enum IncomingStatus {
    Answered,
    Rejected,
}

pub struct PjsuaCallHandle<'a> {
    call_id: pjsua::pjsua_call_id,
    user_data: Box<cb_user_data::StateChangedUserData>,
    _pjsua_instance_started: &'a pjsua_softphone_api::PjsuaInstanceStarted,
}

impl<'a> PjsuaCallHandle<'a> {
    pub fn new(
        call_id: pjsua::pjsua_call_id,
        pjsua_instance_started: &'a pjsua_softphone_api::PjsuaInstanceStarted,
    ) -> Result<Self, PjsuaError> {
        let (state_changed_tx, state_changed_rx) = tokio::sync::mpsc::channel(3);

        let mut user_data = Box::new(cb_user_data::StateChangedUserData {
            on_state_changed_tx: state_changed_tx,
        });

        let raw_user_data = user_data.as_mut() as *mut cb_user_data::StateChangedUserData;

        unsafe {
            eprintln!("Setting user data...");
            let status =
                pjsua::pjsua_call_set_user_data(call_id, raw_user_data as *mut std::ffi::c_void);

            get_error_as_result(status)?;
        }

        Ok(Self {
            call_id,
            user_data,
            _pjsua_instance_started: pjsua_instance_started,
        })
    }

    fn answer(&self, answer_code: impl answer_code::AnswerCode) -> Result<(), PjsuaError> {
        accept_incoming(self.call_id, answer_code)?;

        Ok(())
    }

    fn hangup(self) {}
}

impl<'a> Drop for PjsuaCallHandle<'a> {
    fn drop(&mut self) {
        eprintln!("Dropping PjsuaIncomingCall");
        //note: this will hangup the call if it's still active AND prevent any futher usafe of
        //on_state_changed. Then it follows that user_data will no longer be used.

        hangup_call(self.call_id).expect("Failed to reject incoming call");
    }
}

pub struct PjsuaIncomingCall<'a> {
    call_handle: Option<PjsuaCallHandle<'a>>,
    account_id: pjsua::pjsua_acc_id,
    pjsua_instance_started: &'a pjsua_softphone_api::PjsuaInstanceStarted,
}

impl<'a> PjsuaIncomingCall<'a> {
    pub(crate) fn new(
        account_id: pjsua::pjsua_acc_id,
        call_id: pjsua::pjsua_call_id,
        pjsua_instance_started: &'a pjsua_softphone_api::PjsuaInstanceStarted,
    ) -> Result<Self, PjsuaError> {
        let call_handle = PjsuaCallHandle::new(call_id, pjsua_instance_started)?;

        Ok(Self {
            call_handle: Some(call_handle),
            account_id,
            pjsua_instance_started,
        })
    }

    pub async fn answer_session_progress(self) -> Result<PjsuaCallSetup<'a>, PjsuaError> {
        PjsuaCallSetup::new(self).await
    }

    pub async fn reject(mut self) -> Result<(), PjsuaError> {
        let call_id = self.call_handle.take().unwrap().call_id;

        spawn_blocking_pjsua(move || {
            reject_incoming(call_id)?;

            Ok::<(), PjsuaError>(())
        })
        .await
        .unwrap()
    }
}

type CallStateReceiver = tokio::sync::mpsc::Receiver<(pjsua::pjsua_call_id, State)>;

use std::cell::RefCell;

use super::pjsua_conf_bridge::SinkMediaPortAdded;

pub struct PjsuaCallSetup<'a> {
    _account_id: pjsua::pjsua_acc_id,
    call_handle: PjsuaCallHandle<'a>,
    pjsua_instance_started: &'a pjsua_softphone_api::PjsuaInstanceStarted,
    on_call_state_changed_rx: RefCell<CallStateReceiver>,
}

impl<'a> PjsuaCallSetup<'a> {
    async fn new(
        mut incoming_call: PjsuaIncomingCall<'a>,
    ) -> Result<PjsuaCallSetup<'a>, PjsuaError> {
        let (state_changed_tx, state_changed_rx) = tokio::sync::mpsc::channel(3);
        let user_data = Box::new(cb_user_data::StateChangedUserData {
            on_state_changed_tx: state_changed_tx,
        });

        let call_handle = incoming_call
            .call_handle
            .take()
            .expect("Call handle is None!");

        unsafe {
            eprintln!("Setting user data...");
            let status = pjsua::pjsua_call_set_user_data(
                call_handle.call_id,
                Box::into_raw(user_data) as *mut std::ffi::c_void,
            );
            get_error_as_result(status).expect("Failed to set user data");
        }

        spawn_blocking_pjsua(move || {
            accept_incoming(call_handle.call_id, answer_code::SessionProgress)?;

            Ok::<(), PjsuaError>(())
        })
        .await
        .unwrap()?;

        let call = Self {
            call_handle,
            _account_id: incoming_call.account_id,
            pjsua_instance_started: incoming_call.pjsua_instance_started,
            on_call_state_changed_rx: RefCell::new(state_changed_rx),
        };

        Ok(call)
    }

    pub fn connect_with_sink_media_port(
        &'a self,
        mut sink_media_port: impl SinkMediaPortAdded,
        mem_pool: &'a PjsuaMemoryPool,
    ) -> Result<PjsuaSinkBufferMediaPortConnected<'a>, PjsuaError> {
        let bridge = self.pjsua_instance_started.get_bridge();

        let sink_connected = bridge.add_sink(sink_media_port.as_mut(), mem_pool)?.connect(&self)?;

        Ok(sink_connected)
    }

    pub(crate) fn get_conf_port_slot(&self) -> Result<pjsua::pjsua_conf_port_id, PjsuaError> {
        let conf_slot = unsafe { pjsua::pjsua_call_get_conf_port(self.call_handle.call_id) };

        match conf_slot {
            pjsua::pjsua_invalid_id_const__PJSUA_INVALID_ID => Err(PjsuaError {
                code: -1,
                message: "Invalid conf slot".to_string(),
            }),
            _ => Ok(conf_slot),
        }
    }

    pub async fn hangup(self) {}

    pub async fn wait_for_state_change(&self) -> Result<State, PjsuaError> {
        let (call_id, state) = self
            .on_call_state_changed_rx
            .borrow_mut()
            .recv()
            .await
            .expect("this should not happen");

        assert_eq!(call_id, self.call_handle.call_id);

        Ok(state)
    }

    pub async fn await_hangup(&self) -> Result<(), PjsuaError> {
        loop {
            let state = self.wait_for_state_change().await?;

            if let State::PjsipInvStateDisconnected = state {
                break;
            }
        }

        Ok(())
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
