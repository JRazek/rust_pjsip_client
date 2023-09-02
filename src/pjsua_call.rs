use crate::pjsua_softphone_api;

use super::error::{get_error_as_result, PjsuaError};
use std::ptr;

use super::tokio_utils::spawn_blocking_pjsua;

use std::mem::MaybeUninit;

use super::pjsua_memory_pool::PjsuaMemoryPool;

use super::pjsua_sink_buffer_media_port::{
    PjsuaSinkBufferMediaPort, PjsuaSinkBufferMediaPortConnected,
};

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
    if !is_call_active(call_id) {
        return Ok(());
    }
    unsafe {
        let status = pjsua::pjsua_call_hangup(call_id, 200, ptr::null(), ptr::null());
        get_error_as_result(status)?;
    }

    Ok(())
}

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

pub struct PjsuaIncomingCall<'a> {
    account_id: pjsua::pjsua_acc_id,
    call_id: pjsua::pjsua_call_id,
    status: Option<IncomingStatus>,
    pjsua_instance_started: &'a pjsua_softphone_api::PjsuaInstanceStarted,
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
            pjsua_instance_started,
        }
    }

    pub async fn answer_ok(mut self) -> Result<PjsuaCall<'a>, PjsuaError> {
        self.status = Some(IncomingStatus::Answered);
        PjsuaCall::new(self).await
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
        match &self.status {
            Some(IncomingStatus::Answered) => {}
            _ => {
                eprintln!("Dropping PjsuaIncomingCall");
                reject_incoming(self.call_id).expect("Failed to reject incoming call");
            }
        }
    }
}

type CallStateReceiver = tokio::sync::mpsc::Receiver<(pjsua::pjsua_call_id, State)>;

use std::cell::RefCell;

pub struct PjsuaCall<'a> {
    _account_id: pjsua::pjsua_acc_id,
    call_id: pjsua::pjsua_call_id,
    pjsua_instance_started: &'a pjsua_softphone_api::PjsuaInstanceStarted,
    on_call_state_changed_rx: RefCell<CallStateReceiver>,
}

impl<'a> PjsuaCall<'a> {
    async fn new(incoming_call: PjsuaIncomingCall<'a>) -> Result<PjsuaCall<'a>, PjsuaError> {
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
            get_error_as_result(status).expect("Failed to set user data");
        }

        spawn_blocking_pjsua(move || {
            accept_incoming(incoming_call.call_id)?;

            Ok::<(), PjsuaError>(())
        })
        .await
        .unwrap()?;

        let call = Self {
            _account_id: incoming_call.account_id,
            call_id: incoming_call.call_id,
            pjsua_instance_started: incoming_call.pjsua_instance_started,
            on_call_state_changed_rx: RefCell::new(state_changed_rx),
        };

        Ok(call)
    }

    pub fn connect_with_sink_media_port(
        &'a self,
        sink_media_port: PjsuaSinkBufferMediaPort<'a>,
        mem_pool: &'a PjsuaMemoryPool,
    ) -> Result<PjsuaSinkBufferMediaPortConnected<'a>, PjsuaError> {
        let bridge = self.pjsua_instance_started.get_bridge();

        let sink_connected = bridge.add_sink(sink_media_port, mem_pool)?.connect(&self)?;

        let has = unsafe { pjsua::pjsua_call_has_media(self.call_id) };

        println!("has media: {}", has);

        Ok(sink_connected)
    }

    pub(crate) fn get_conf_port_slot(&self) -> Result<pjsua::pjsua_conf_port_id, PjsuaError> {
        let conf_slot = unsafe { pjsua::pjsua_call_get_conf_port(self.call_id) };

        Ok(conf_slot)
    }

    pub async fn hangup(self) -> Result<(), RemoteAlreadyHangUpError> {
        spawn_blocking_pjsua(move || match hangup_call(self.call_id) {
            Ok(()) => Ok(()),
            Err(_) => Err(RemoteAlreadyHangUpError),
        })
        .await
        .unwrap()?;

        Ok(())
    }

    pub async fn wait_for_state_change(&self) -> Result<State, PjsuaError> {
        let (call_id, state) = self
            .on_call_state_changed_rx
            .borrow_mut()
            .recv()
            .await
            .expect("this should not happen");

        assert_eq!(call_id, self.call_id);

        Ok(state)
    }

    pub async fn await_hangup(
        &self,
        port: PjsuaSinkBufferMediaPortConnected<'a>,
    ) -> Result<(), PjsuaError> {
        loop {
            let state = self.wait_for_state_change().await?;

            if let State::PjsipInvStateDisconnected = state {
                break;
            }
        }

        drop(port);

        Ok(())
    }
}

impl<'a> Drop for PjsuaCall<'a> {
    fn drop(&mut self) {
        eprintln!("Dropping PjsuaCall...");
        _ = hangup_call(self.call_id);
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
