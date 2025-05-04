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

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::sync::{Arc, Mutex};
use crate::api::media_engine::MediaEngine;
use crate::api::APIBuilder;
use crate::peer_connection::RTCPeerConnection;
use crate::peer_connection::configuration::RTCConfiguration;
use crate::peer_connection::sdp::session_description::RTCSessionDescription;
use crate::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use crate::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use crate::track::track_local::TrackLocal;
use crate::media::Sample;
use crate::ice_transport::ice_candidate::RTCIceCandidateInit;
use tokio::runtime::Runtime;
use vpx_encode::{Encoder, Config};
use vpx_encode::Codec;

pub type webrtc_input_callback_t = Option<extern "C" fn(data: *const c_void, len: c_int, user_data: *mut c_void)>;
pub type webrtc_signal_callback_t = Option<extern "C" fn(msg: *const c_char, user_data: *mut c_void)>;

type SignalCallback = Option<extern "C" fn(msg: *const c_char, user_data: *mut c_void)>;

struct WebrtcSession {
    pc: Arc<RTCPeerConnection>,
    video_track: Arc<TrackLocalStaticSample>,
    signal_cb: SignalCallback,
    signal_user_data: *mut c_void,
    rt: Runtime,
}

#[repr(C)]
pub struct webrtc_session_t {
    inner: Mutex<Option<WebrtcSession>>,
}

unsafe impl Send for webrtc_session_t {}
unsafe impl Sync for webrtc_session_t {}

#[no_mangle]
pub extern "C" fn webrtc_session_create(
    _config_json: *const c_char,
    _input_cb: webrtc_input_callback_t,
    _user_data: *mut c_void,
) -> *mut webrtc_session_t {
    let mut m = MediaEngine::default();
    m.register_default_codecs().unwrap();
    let api = APIBuilder::new().with_media_engine(m).build();
    let rt = Runtime::new().unwrap();
    let (pc, video_track) = rt.block_on(async {
        let pc = Arc::new(api.new_peer_connection(RTCConfiguration::default()).await.unwrap());
        let video_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: "video/VP8".to_owned(),
                ..Default::default()
            },
            "video".to_owned(),
            "webrtc-rs".to_owned(),
        ));
        pc.add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>).await.unwrap();
        (pc, video_track)
    });
    let session = WebrtcSession {
        pc,
        video_track,
        signal_cb: None,
        signal_user_data: std::ptr::null_mut(),
        rt,
    };
    Box::into_raw(Box::new(webrtc_session_t {
        inner: Mutex::new(Some(session)),
    }))
}

#[no_mangle]
pub extern "C" fn webrtc_session_set_signal_callback(
    session: *mut webrtc_session_t,
    cb: webrtc_signal_callback_t,
    user_data: *mut c_void,
) {
    let session = unsafe { &mut *session };
    if let Some(ref mut s) = *session.inner.lock().unwrap() {
        s.signal_cb = cb;
        s.signal_user_data = user_data;
        let pc = Arc::clone(&s.pc);
        // Copy user_data to a usize for Send safety
        let user_data_val = user_data as usize;
        let cb = cb;
        s.rt.spawn(async move {
            pc.on_ice_candidate(Box::new(move |cand| {
                let cb = cb;
                let user_data_val = user_data_val;
                Box::pin(async move {
                    if let Some(c) = cand {
                        let json = serde_json::to_string(&c.to_json().unwrap()).unwrap();
                        if let Some(cb) = cb {
                            let cstr = CString::new(json).unwrap();
                            // Cast back to pointer
                            cb(cstr.as_ptr(), user_data_val as *mut c_void);
                        }
                    }
                })
            }));
        });
    }
}

#[no_mangle]
pub extern "C" fn webrtc_session_set_remote_description(
    session: *mut webrtc_session_t,
    sdp_json: *const c_char,
) {
    let session = unsafe { &mut *session };
    let sdp_json = unsafe { CStr::from_ptr(sdp_json).to_string_lossy().to_string() };
    if let Some(ref mut s) = *session.inner.lock().unwrap() {
        let pc = Arc::clone(&s.pc);
        let cb = s.signal_cb;
        let user_data_val = s.signal_user_data as usize;
        let rt = &s.rt;
        rt.spawn(async move {
            let sdp: RTCSessionDescription = serde_json::from_str(&sdp_json).unwrap();
            pc.set_remote_description(sdp).await.unwrap();
            let answer = pc.create_answer(None).await.unwrap();
            pc.set_local_description(answer.clone()).await.unwrap();
            let answer_json = serde_json::to_string(&answer).unwrap();
            if let Some(cb) = cb {
                let cstr = CString::new(answer_json).unwrap();
                cb(cstr.as_ptr(), user_data_val as *mut c_void);
            }
        });
    }
}

#[no_mangle]
pub extern "C" fn webrtc_session_add_ice_candidate(
    session: *mut webrtc_session_t,
    candidate_json: *const c_char,
) {
    let session = unsafe { &mut *session };
    let candidate_json = unsafe { CStr::from_ptr(candidate_json).to_string_lossy().to_string() };
    if let Some(ref mut s) = *session.inner.lock().unwrap() {
        let pc = Arc::clone(&s.pc);
        let rt = &s.rt;
        rt.spawn(async move {
            let cand: RTCIceCandidateInit = serde_json::from_str(&candidate_json).unwrap();
            pc.add_ice_candidate(cand).await.unwrap();
        });
    }
}

#[no_mangle]
pub extern "C" fn webrtc_session_send_frame(
    session: *mut webrtc_session_t,
    width: c_int,
    height: c_int,
    yuv: *const u8,
) {
    let session = unsafe { &mut *session };
    if let Some(ref mut s) = *session.inner.lock().unwrap() {
        let w = width as u32;
        let h = height as u32;
        let yuv_slice = unsafe { std::slice::from_raw_parts(yuv, (w * h * 3 / 2) as usize) };
        let mut encoder = Encoder::new(Config {
            width: w,
            height: h,
            timebase: [1, 30],
            bitrate: 1_000_000,
            codec: Codec::VP9,
        }).expect("Failed to create VP9 encoder");
        let pts = 0; // TODO: use real timestamp
        let packets = encoder.encode(pts, yuv_slice).expect("VP9 encode failed");
        for pkt in packets {
            let data = pkt.data;
            let duration = std::time::Duration::from_millis(33); // ~30 FPS
            let sample = Sample {
                data: data.into(),
                duration,
                ..Default::default()
            };
            let video_track = Arc::clone(&s.video_track);
            s.rt.spawn(async move {
                let _ = video_track.write_sample(&sample).await;
            });
        }
    }
}

#[no_mangle]
pub extern "C" fn webrtc_session_destroy(session: *mut webrtc_session_t) {
    if !session.is_null() {
        unsafe { Box::from_raw(session) };
    }
}
