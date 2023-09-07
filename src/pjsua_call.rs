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
    pub trait AnswerCode: Send + 'static {
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

pub struct PjsuaCallHandle<'a> {
    call_id: pjsua::pjsua_call_id,
    user_data: Box<cb_user_data::StateChangedUserData>,
    _pjsua_instance_started: &'a pjsua_softphone_api::PjsuaInstanceStarted,
    state_changed_rx: CallStateReceiver,
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
            state_changed_rx,
            _pjsua_instance_started: pjsua_instance_started,
        })
    }

    async fn answer(&self, answer_code: impl answer_code::AnswerCode) -> Result<(), PjsuaError> {
        let call_id = self.call_id;
        spawn_blocking_pjsua(move || {
            accept_incoming(call_id, answer_code)?;

            Ok::<(), PjsuaError>(())
        })
        .await
        .unwrap()?;

        Ok(())
    }

    fn hangup(self) {}
}

impl<'a> Drop for PjsuaCallHandle<'a> {
    fn drop(&mut self) {
        eprintln!("Dropping PjsuaCallHandle");
        //note: this will hangup the call if it's still active AND prevent any futher usafe of
        //on_state_changed. Then it follows that user_data will no longer be used.

        hangup_call(self.call_id).expect("Failed to reject incoming call");

        eprintln!("Dropped PjsuaCallHandle");
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

use super::pjmedia_port_audio_sink::*;

async fn await_state(state_rx: &mut CallStateReceiver, state: State) -> Result<(), PjsuaError> {
    eprintln!("Awaiting state: {:?}", state);

    if let Some((_, state_recv)) = state_rx.recv().await {
        if state_recv == state {
            eprintln!("State received: {:?}", state);
            return Ok(());
        }
    }

    return Err(PjsuaError {
        code: -1,
        message: "Unexpected state".to_string(),
    });
}

pub struct PjsuaCallSetup<'a> {
    _account_id: pjsua::pjsua_acc_id,
    call_handle: PjsuaCallHandle<'a>,
    pjsua_instance_started: &'a pjsua_softphone_api::PjsuaInstanceStarted,
}

impl<'a> PjsuaCallSetup<'a> {
    async fn new(
        mut incoming_call: PjsuaIncomingCall<'a>,
    ) -> Result<PjsuaCallSetup<'a>, PjsuaError> {
        let mut call_handle = incoming_call
            .call_handle
            .take()
            .expect("Call handle is None!");

        call_handle.answer(answer_code::SessionProgress).await?;

        await_state(&mut call_handle.state_changed_rx, State::PjsipInvStateEarly).await?;

        let call = Self {
            call_handle,
            _account_id: incoming_call.account_id,
            pjsua_instance_started: incoming_call.pjsua_instance_started,
        };

        Ok(call)
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

    pub async fn connect(
        self,
        custom_media_port: CustomSinkMediaPort<'a>,
        mem_pool: &'a PjsuaMemoryPool,
    ) -> Result<PjsuaCall<'a>, PjsuaError> {
        let bridge = self.pjsua_instance_started.get_bridge();

        let call = bridge
            .setup_media(custom_media_port, self, mem_pool)
            .await?;

        Ok(call)
    }
}

pub struct PjsuaCall<'a> {
    media_sink: CustomSinkMediaPortConnected<'a>,
    call_handle: PjsuaCallHandle<'a>,
}

impl<'a> PjsuaCall<'a> {
    pub async fn new(
        pjsua_call_setup: PjsuaCallSetup<'a>,
        media_sink: CustomSinkMediaPortConnected<'a>,
    ) -> Result<PjsuaCall<'a>, PjsuaError> {
        let mut call_handle = pjsua_call_setup.call_handle;

        call_handle.answer(answer_code::Ok).await?;

        await_state(
            &mut call_handle.state_changed_rx,
            State::PjsipInvStateConnecting,
        )
        .await?;

        await_state(
            &mut call_handle.state_changed_rx,
            State::PjsipInvStateConfirmed,
        )
        .await?;

        Ok(Self {
            media_sink,
            call_handle,
        })
    }

    pub async fn await_hangup(mut self) -> Result<(), PjsuaError> {
        await_state(
            &mut self.call_handle.state_changed_rx,
            State::PjsipInvStateDisconnected,
        )
        .await?;

        Ok(())
    }

    //    pub async fn recv(&mut self) -> Result<Vec<u8>, PjsuaError> {
    //        self.media_sink.recv().await
    //    }
}

impl<'a> PjsuaCall<'a> {
    delegate::delegate! {
        to self.call_handle {
            pub fn hangup(self);
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

pub(crate) mod cb_user_data {
    use super::State;
    use tokio::sync::mpsc::Sender;

    #[allow(unused_parens)]
    pub(crate) type OnStateChangedSendData = (pjsua::pjsua_call_id, State);

    pub struct StateChangedUserData {
        pub(crate) on_state_changed_tx: Sender<OnStateChangedSendData>,
    }
}

#[derive(Debug, PartialEq, Eq)]
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
