#![warn(rust_2018_idioms)]
#![allow(dead_code)]

pub use {data, dtls, ice, interceptor, mdns, media, rtcp, rtp, sctp, sdp, srtp, stun, turn, util};

/// [`peer_connection::RTCPeerConnection`] allows to establish connection between two peers given RTC configuration. Its API is similar to one in JavaScript.
pub mod peer_connection;

/// The utilities defining transport between peers. Contains [`ice_transport::ice_server::RTCIceServer`] struct which describes how peer does ICE (Interactive Connectivity Establishment).
pub mod ice_transport;

/// WebRTC DataChannel can be used for peer-to-peer transmitting arbitrary binary data.
pub mod data_channel;

/// Module responsible for multiplexing data streams of different protocols on one socket. Custom [`mux::endpoint::Endpoint`] with [`mux::mux_func::MatchFunc`] can be used for parsing your application-specific byte stream.
pub mod mux; // TODO: why is this public? does someone really extend WebRTC stack?

/// Measuring connection statistics, such as amount of data transmitted or round trip time.
pub mod stats;

/// [`Error`] enumerates WebRTC problems, [`error::OnErrorHdlrFn`] defines type for callback-logger.
pub mod error;

/// Set of constructors for WebRTC primitives. Subject to deprecation in future.
pub mod api;

pub mod dtls_transport;
pub mod rtp_transceiver;
pub mod sctp_transport;
pub mod track;

pub use error::Error;

#[macro_use]
extern crate lazy_static;

pub(crate) const UNSPECIFIED_STR: &str = "Unspecified";

/// Equal to UDP MTU
pub(crate) const RECEIVE_MTU: usize = 1460;

pub(crate) const SDP_ATTRIBUTE_RID: &str = "rid";
pub(crate) const SDP_ATTRIBUTE_SIMULCAST: &str = "simulcast";
pub(crate) const GENERATED_CERTIFICATE_ORIGIN: &str = "WebRTC";

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};

#[repr(C)]
pub struct webrtc_session_t {
    _private: [u8; 0],
}

pub type webrtc_input_callback_t = Option<extern "C" fn(data: *const c_void, len: c_int, user_data: *mut c_void)>;
pub type webrtc_signal_callback_t = Option<extern "C" fn(msg: *const c_char, user_data: *mut c_void)>;

#[no_mangle]
pub extern "C" fn webrtc_session_create(
    config_json: *const c_char,
    cb: webrtc_input_callback_t,
    user_data: *mut c_void,
) -> *mut webrtc_session_t {
    // TODO: Implement real session creation
    std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn webrtc_session_send_frame(
    session: *mut webrtc_session_t,
    width: c_int,
    height: c_int,
    yuv: *const u8,
) {
    // TODO: Implement real frame sending
}

#[no_mangle]
pub extern "C" fn webrtc_session_set_signal_callback(
    _session: *mut webrtc_session_t,
    _cb: webrtc_signal_callback_t,
    _user_data: *mut c_void,
) {
    // TODO: Store callback and call it when local signaling messages are generated
}

#[no_mangle]
pub extern "C" fn webrtc_session_set_remote_description(
    _session: *mut webrtc_session_t,
    _sdp_json: *const c_char,
) {
    // TODO: Pass remote SDP to the session
}

#[no_mangle]
pub extern "C" fn webrtc_session_add_ice_candidate(
    _session: *mut webrtc_session_t,
    _candidate_json: *const c_char,
) {
    // TODO: Pass ICE candidate to the session
}

#[no_mangle]
pub extern "C" fn webrtc_session_destroy(session: *mut webrtc_session_t) {
    // TODO: Implement real destruction
}
