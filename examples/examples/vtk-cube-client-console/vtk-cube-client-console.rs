use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use log::info;
use serde::{Deserialize, Serialize};
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::api::APIBuilder;
use webrtc::api::media_engine::MediaEngine;
use interceptor::registry::Registry;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::rtp_transceiver::rtp_receiver::RTCRtpReceiver;
use webrtc::track::track_remote::TrackRemote;
use webrtc::rtp_transceiver::rtp_codec::RTPCodecType;
use futures_util::{StreamExt, SinkExt};
use std::sync::Arc;
use webrtc::rtp_transceiver::RTCRtpTransceiver;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
enum SignalMessage {
    Offer { sdp: String },
    Answer { sdp: String },
    IceCandidate { candidate: String, sdp_mid: String, sdp_mline_index: u32 },
}

#[tokio::main]
async fn main() {
    env_logger::init();
    println!("vtk-cube-client-console: starting up");

    // Connect to signaling server (replace with actual URL if needed)
    let signaling_url = "ws://127.0.0.1:8080";
    let url = Url::parse(signaling_url).expect("Invalid signaling server URL");
    match connect_async(url).await {
        Ok((mut ws_stream, _)) => {
            info!("Connected to signaling server at {}", signaling_url);

            // Set up WebRTC API and PeerConnection
            let mut m = MediaEngine::default();
            m.register_default_codecs().expect("Failed to register codecs");
            let registry = register_default_interceptors(Registry::new(), &mut m).unwrap();
            let api = APIBuilder::new()
                .with_media_engine(m)
                .with_interceptor_registry(registry)
                .build();
            let config = RTCConfiguration::default();
            let pc = api.new_peer_connection(config).await.expect("Failed to create PeerConnection");

            // Add video transceiver (recvonly)
            pc.add_transceiver_from_kind(RTPCodecType::Video, None)
                .await
                .expect("Failed to add video transceiver");

            // Set up on_track handler to log frame properties
            pc.on_track(Box::new(move |track: Arc<TrackRemote>, _receiver: Arc<RTCRtpReceiver>, _transceiver: Arc<RTCRtpTransceiver>| {
                Box::pin(async move {
                    println!("Received remote track: {}", track.kind());
                    while let Ok((rtp, _)) = track.read_rtp().await {
                        println!(
                            "Frame: SSRC={}, Timestamp={}, PayloadType={}, Size={} bytes",
                            rtp.header.ssrc, rtp.header.timestamp, rtp.header.payload_type, rtp.payload.len()
                        );
                    }
                })
            }));

            // Signaling loop
            while let Some(msg) = ws_stream.next().await {
                match msg {
                    Ok(Message::Text(txt)) => {
                        if let Ok(signal) = serde_json::from_str::<SignalMessage>(&txt) {
                            match signal {
                                SignalMessage::Offer { sdp } => {
                                    pc.set_remote_description(RTCSessionDescription::offer(sdp).expect("Failed to create offer SDP")).await.expect("set_remote_description failed");
                                    let answer = pc.create_answer(None).await.expect("create_answer failed");
                                    pc.set_local_description(answer.clone()).await.expect("set_local_description failed");
                                    let answer_msg = SignalMessage::Answer { sdp: answer.sdp };
                                    let msg = serde_json::to_string(&answer_msg).unwrap();
                                    ws_stream.send(Message::Text(msg)).await.expect("send answer failed");
                                }
                                SignalMessage::IceCandidate { candidate, sdp_mid, sdp_mline_index } => {
                                    let ice = RTCIceCandidateInit {
                                        candidate,
                                        sdp_mid: Some(sdp_mid),
                                        sdp_mline_index: Some(sdp_mline_index as u16),
                                        username_fragment: None,
                                    };
                                    pc.add_ice_candidate(ice).await.expect("add_ice_candidate failed");
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to connect to signaling server: {}", e);
        }
    }
}