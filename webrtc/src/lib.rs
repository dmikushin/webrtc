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
pub mod mux;

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
use std::os::raw::{c_int, c_void};
use std::sync::{Arc, Mutex};
use bytes::Bytes;
use log::{debug, error, info, warn};
use crate::api::media_engine::MediaEngine;
use crate::api::APIBuilder;
use crate::peer_connection::RTCPeerConnection;
use crate::peer_connection::configuration::RTCConfiguration;
use crate::peer_connection::sdp::session_description::RTCSessionDescription;
use crate::peer_connection::peer_connection_state::RTCPeerConnectionState;
use crate::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use crate::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use crate::track::track_local::TrackLocal;
use crate::media::Sample;
use crate::ice_transport::ice_candidate::RTCIceCandidateInit;
use crate::data_channel::data_channel_init::RTCDataChannelInit;
use tokio::runtime::Runtime;
use vpx_encode::{Encoder, Config, VideoCodecId};
use std::os::raw::c_char;
// Remove incorrect Codec import

// Define the callback types with proper Rust naming conventions
pub type WebrtcInputCallbackT = extern "C" fn(data: *const c_void, len: c_int, user_data: *mut c_void);
pub type WebrtcSignalCallbackT = extern "C" fn(msg: *const c_char, user_data: *mut c_void);

// Structure to hold the encoder state
struct EncoderState {
    encoder: Encoder,
    width: u32,
    height: u32,
    frame_count: u64,
}

// Structure to hold the WebRTC session state
struct WebrtcSession {
    pc: Arc<RTCPeerConnection>,
    video_track: Arc<TrackLocalStaticSample>,
    signal_cb: Option<WebrtcSignalCallbackT>,
    signal_user_data: *mut c_void,
    input_cb: Option<WebrtcInputCallbackT>,
    input_user_data: *mut c_void,
    encoder_state: Option<EncoderState>,
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
    input_cb: Option<WebrtcInputCallbackT>,
    user_data: *mut c_void,
) -> *mut webrtc_session_t {
    // Set up media engine with default codecs
    let mut m = MediaEngine::default();
    
    match m.register_default_codecs() {
        Ok(_) => debug!("Registered default codecs"),
        Err(e) => {
            error!("Failed to register default codecs: {}", e);
            return std::ptr::null_mut();
        }
    }
    
    // Create API with media engine
    let api = APIBuilder::new().with_media_engine(m).build();
    
    // Create runtime for async operations
    let rt = match Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            error!("Failed to create runtime: {}", e);
            return std::ptr::null_mut();
        }
    };
    
    // Set up peer connection and video track
    let (pc, video_track) = match rt.block_on(async {
        // Create peer connection with default configuration
        let pc = match api.new_peer_connection(RTCConfiguration::default()).await {
            Ok(pc) => Arc::new(pc),
            Err(e) => {
                error!("Failed to create peer connection: {}", e);
                return Err(e);
            }
        };
        
        // Create video track with VP9 codec
        let video_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: "video/VP9".to_owned(),
                ..Default::default()
            },
            "video".to_owned(),
            "webrtc-rs".to_owned(),
        ));
        
        // Add track to peer connection
        match pc.add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>).await {
            Ok(_) => (),
            Err(e) => {
                error!("Failed to add track: {}", e);
                return Err(e);
            }
        }
        
        // Create data channel for input events - prefix with underscore to mark as intentionally unused
        let dc_init = RTCDataChannelInit {
            ordered: Some(true),
            ..Default::default()
        };
        
        // Create data channel with label "input"
        let _dc = match pc.create_data_channel("input", Some(dc_init)).await {
            Ok(dc) => dc,
            Err(e) => {
                error!("Failed to create data channel: {}", e);
                return Err(e);
            }
        };
        
        Ok((pc, video_track))
    }) {
        Ok(result) => result,
        Err(_) => return std::ptr::null_mut(),
    };
    
    // Create WebRTC session
    let session = WebrtcSession {
        pc,
        video_track,
        signal_cb: None,
        signal_user_data: std::ptr::null_mut(),
        input_cb,
        input_user_data: user_data,
        encoder_state: None,
        rt,
    };
    
    // Allocate session
    Box::into_raw(Box::new(webrtc_session_t {
        inner: Mutex::new(Some(session)),
    }))
}

#[no_mangle]
pub extern "C" fn webrtc_session_set_signal_callback(
    session: *mut webrtc_session_t,
    cb: Option<WebrtcSignalCallbackT>,
    user_data: *mut c_void,
) {
    if session.is_null() {
        error!("Null session pointer in webrtc_session_set_signal_callback");
        return;
    }
    
    let session = unsafe { &mut *session };
    let mut guard = match session.inner.lock() {
        Ok(guard) => guard,
        Err(e) => {
            error!("Failed to lock session mutex: {}", e);
            return;
        }
    };
    
    if let Some(ref mut s) = *guard {
        // Set callback and user data
        s.signal_cb = cb;
        s.signal_user_data = user_data;
        
        // Clone for async closure
        let pc = Arc::clone(&s.pc);
        let user_data_val = user_data as usize;
        let callback = cb;
        
        // Handle ICE candidates and connection state changes
        s.rt.spawn(async move {
            // Set up ICE candidate handler
            pc.on_ice_candidate(Box::new(move |cand| {
                let cb = callback;
                let user_data_val = user_data_val;
                
                Box::pin(async move {
                    if let Some(c) = cand {
                        // Convert candidate to JSON
                        match c.to_json() {
                            Ok(json) => {
                                match serde_json::to_string(&json) {
                                    Ok(json_str) => {
                                        if let Some(cb_fn) = cb {
                                            match CString::new(json_str) {
                                                Ok(cstr) => {
                                                    cb_fn(cstr.as_ptr(), user_data_val as *mut c_void);
                                                },
                                                Err(e) => error!("Failed to create CString: {}", e),
                                            }
                                        }
                                    },
                                    Err(e) => error!("Failed to serialize ICE candidate: {}", e),
                                }
                            },
                            Err(e) => error!("Failed to convert ICE candidate to JSON: {}", e),
                        }
                    }
                })
            }));
            
            // Set up connection state handler
            pc.on_peer_connection_state_change(Box::new(move |state| {
                Box::pin(async move {
                    match state {
                        RTCPeerConnectionState::Connected => {
                            info!("PeerConnection Connected");
                        },
                        RTCPeerConnectionState::Failed => {
                            error!("PeerConnection Failed");
                        },
                        RTCPeerConnectionState::Disconnected => {
                            warn!("PeerConnection Disconnected");
                        },
                        RTCPeerConnectionState::Closed => {
                            info!("PeerConnection Closed");
                        },
                        _ => {}
                    }
                })
            }));
        });
        
        // Also set up a data channel handler for the peer connection
        let pc = Arc::clone(&s.pc);
        let input_cb = s.input_cb;
        let input_user_data = s.input_user_data as usize;
        
        s.rt.spawn(async move {
            pc.on_data_channel(Box::new(move |dc| {
                let input_cb = input_cb;
                let input_user_data = input_user_data;
                
                Box::pin(async move {
                    let label = dc.label(); // Remove .await
                    info!("New DataChannel: {}", label);
                    
                    if label == "input" {
                        dc.on_message(Box::new(move |msg| {
                            let input_cb = input_cb;
                            let input_user_data = input_user_data;
                            
                            Box::pin(async move {
                                if let Some(cb_fn) = input_cb {
                                    let data = msg.data.as_ref();
                                    cb_fn(
                                        data.as_ptr() as *const c_void,
                                        data.len() as c_int,
                                        input_user_data as *mut c_void,
                                    );
                                }
                            })
                        }));
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
    if session.is_null() || sdp_json.is_null() {
        error!("Null pointer in webrtc_session_set_remote_description");
        return;
    }
    
    let session = unsafe { &mut *session };
    let sdp_json = match unsafe { CStr::from_ptr(sdp_json).to_str() } {
        Ok(s) => s.to_string(),
        Err(e) => {
            error!("Failed to convert SDP JSON to string: {}", e);
            return;
        }
    };
    
    let mut guard = match session.inner.lock() {
        Ok(guard) => guard,
        Err(e) => {
            error!("Failed to lock session mutex: {}", e);
            return;
        }
    };
    
    if let Some(ref mut s) = *guard {
        let pc = Arc::clone(&s.pc);
        let cb = s.signal_cb;
        let user_data_val = s.signal_user_data as usize;
        let rt = &s.rt;
        
        rt.spawn(async move {
            // Parse SDP from JSON
            let sdp: RTCSessionDescription = match serde_json::from_str(&sdp_json) {
                Ok(sdp) => sdp,
                Err(e) => {
                    error!("Failed to parse SDP JSON: {}", e);
                    return;
                }
            };
            
            // Set remote description
            info!("Setting remote description: {}", sdp.sdp_type);
            if let Err(e) = pc.set_remote_description(sdp).await {
                error!("Failed to set remote description: {}", e);
                return;
            }
            
            // Check if remote description is an offer
            if pc.remote_description().await.map(|rd| rd.sdp_type == "offer".into()).unwrap_or(false) {
                info!("Creating answer");
                let answer = match pc.create_answer(None).await {
                    Ok(answer) => answer,
                    Err(e) => {
                        error!("Failed to create answer: {}", e);
                        return;
                    }
                };
                
                // Set local description
                if let Err(e) = pc.set_local_description(answer.clone()).await {
                    error!("Failed to set local description: {}", e);
                    return;
                }
                
                // Serialize answer to JSON
                let answer_json = match serde_json::to_string(&answer) {
                    Ok(json) => json,
                    Err(e) => {
                        error!("Failed to serialize answer: {}", e);
                        return;
                    }
                };
                
                // Send answer to callback
                if let Some(cb_fn) = cb {
                    match CString::new(answer_json) {
                        Ok(cstr) => {
                            cb_fn(cstr.as_ptr(), user_data_val as *mut c_void);
                        },
                        Err(e) => error!("Failed to create CString: {}", e),
                    }
                }
            }
        });
    }
}

#[no_mangle]
pub extern "C" fn webrtc_session_add_ice_candidate(
    session: *mut webrtc_session_t,
    candidate_json: *const c_char,
) {
    if session.is_null() || candidate_json.is_null() {
        error!("Null pointer in webrtc_session_add_ice_candidate");
        return;
    }
    
    let session = unsafe { &mut *session };
    let candidate_json = match unsafe { CStr::from_ptr(candidate_json).to_str() } {
        Ok(s) => s.to_string(),
        Err(e) => {
            error!("Failed to convert ICE candidate JSON to string: {}", e);
            return;
        }
    };
    
    let mut guard = match session.inner.lock() {
        Ok(guard) => guard,
        Err(e) => {
            error!("Failed to lock session mutex: {}", e);
            return;
        }
    };
    
    if let Some(ref mut s) = *guard {
        let pc = Arc::clone(&s.pc);
        let rt = &s.rt;
        
        rt.spawn(async move {
            // Parse ICE candidate from JSON
            let cand: RTCIceCandidateInit = match serde_json::from_str(&candidate_json) {
                Ok(cand) => cand,
                Err(e) => {
                    error!("Failed to parse ICE candidate JSON: {}", e);
                    return;
                }
            };
            
            // Add ICE candidate
            if let Err(e) = pc.add_ice_candidate(cand).await {
                error!("Failed to add ICE candidate: {}", e);
            }
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
    if session.is_null() || yuv.is_null() {
        error!("Null pointer in webrtc_session_send_frame");
        return;
    }
    
    if width <= 0 || height <= 0 {
        error!("Invalid dimensions in webrtc_session_send_frame: {}x{}", width, height);
        return;
    }
    
    let session = unsafe { &mut *session };
    let w = width as u32;
    let h = height as u32;
    
    // Calculate YUV buffer size (YUV420)
    let yuv_size = (w * h * 3 / 2) as usize;
    let yuv_slice = unsafe { std::slice::from_raw_parts(yuv, yuv_size) };
    
    let mut guard = match session.inner.lock() {
        Ok(guard) => guard,
        Err(e) => {
            error!("Failed to lock session mutex: {}", e);
            return;
        }
    };

    // Extract needed values and drop the guard before async work
    let (video_track, rt, encoder_state_ptr) = if let Some(ref mut s) = *guard {
        let video_track = Arc::clone(&s.video_track);
        let rt = s.rt.handle().clone();
        let need_new_encoder = match s.encoder_state {
            Some(ref state) => state.width != w || state.height != h,
            None => true,
        };
        if need_new_encoder {
            info!("Creating VP9 encoder: {}x{}", w, h);
            match Encoder::new(Config {
                width: w,
                height: h,
                timebase: [1, 30],
                bitrate: 1_000_000,
                codec: VideoCodecId::VP9,
            }) {
                Ok(encoder) => {
                    s.encoder_state = Some(EncoderState {
                        encoder,
                        width: w,
                        height: h,
                        frame_count: 0,
                    });
                }
                Err(e) => {
                    error!("Failed to create VP9 encoder: {}", e);
                    return;
                }
            }
        }
        let encoder_state_ptr = s.encoder_state.as_mut().unwrap() as *mut EncoderState;
        (video_track, rt, encoder_state_ptr)
    } else {
        return;
    };
    drop(guard);

    // SAFETY: encoder_state_ptr is valid because we have exclusive access and no one else can access it until this function returns
    let encoder_state = unsafe { &mut *encoder_state_ptr };
    encoder_state.frame_count += 1;
    let pts: i64 = (encoder_state.frame_count * 3000) as i64;
    match encoder_state.encoder.encode(pts, yuv_slice) {
        Ok(packets) => {
            for pkt in packets {
                let sample = Sample {
                    data: Bytes::from(pkt.data),
                    duration: std::time::Duration::from_millis(33),
                    ..Default::default()
                };
                let video_track = Arc::clone(&video_track);
                rt.spawn(async move {
                    if let Err(e) = video_track.write_sample(&sample).await {
                        error!("Failed to write sample: {}", e);
                    }
                });
            }
        }
        Err(e) => {
            error!("VP9 encode failed: {}", e);
        }
    }
    return;
}

#[no_mangle]
pub extern "C" fn webrtc_session_destroy(session: *mut webrtc_session_t) {
    if session.is_null() {
        return;
    }
    
    let session_box = unsafe { Box::from_raw(session) };
    let mut guard = match session_box.inner.lock() {
        Ok(guard) => guard,
        Err(e) => {
            error!("Failed to lock session mutex during destruction: {}", e);
            return;
        }
    };
    
    if let Some(ref mut s) = *guard {
        // Close peer connection
        let pc = Arc::clone(&s.pc);
        let rt = &s.rt;
        
        rt.block_on(async {
            if let Err(e) = pc.close().await {
                error!("Failed to close peer connection: {}", e);
            }
        });
    }
    
    // Remove session from mutex
    *guard = None;
    
    // Session will be dropped when guard is dropped
}

#[no_mangle]
pub extern "C" fn webrtc_session_get_diagnostics(session: *mut webrtc_session_t) -> *mut c_char {
    use std::ffi::CString;
    use std::ptr;
    use serde_json::json;

    if session.is_null() {
        return ptr::null_mut();
    }
    let session = unsafe { &mut *session };
    let guard = match session.inner.lock() {
        Ok(guard) => guard,
        Err(_) => return ptr::null_mut(),
    };
    if let Some(ref s) = *guard {
        let pc = Arc::clone(&s.pc);
        let rt = &s.rt;
        // Block on async to get ICE info
        let result: Result<serde_json::Value, ()> = rt.block_on(async move {
            let mut diagnostics = json!({});
            // Local ICE credentials
            if let Some(params) = pc.get_local_ice_parameters().await {
                diagnostics["local_ice_ufrag"] = json!(params.username_fragment);
                diagnostics["local_ice_pwd"] = json!(params.password);
            }
            // Selected candidate pair (if available)
            if let Some(pair) = pc.sctp().transport().ice_transport().get_selected_candidate_pair().await {
                let local = pair.local_candidate();
                let remote = pair.remote_candidate();
                diagnostics["selected_local_candidate"] = json!({
                    "address": local.address,
                    "port": local.port,
                    "type": format!("{:?}", local.typ),
                });
                diagnostics["selected_remote_candidate"] = json!({
                    "address": remote.address,
                    "port": remote.port,
                    "type": format!("{:?}", remote.typ),
                });
            }
            Ok(diagnostics)
        });
        drop(guard);
        if let Ok(json_val) = result {
            if let Ok(json_str) = serde_json::to_string(&json_val) {
                if let Ok(cstr) = CString::new(json_str) {
                    return cstr.into_raw();
                }
            }
        }
    }
    ptr::null_mut()
}
