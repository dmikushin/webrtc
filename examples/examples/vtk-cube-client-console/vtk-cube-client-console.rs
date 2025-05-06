use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use log::info; // Ensure log macros are available
use serde::{Deserialize, Serialize};
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::api::APIBuilder;
use webrtc::api::media_engine::MediaEngine;
use interceptor::registry::Registry;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState; // Corrected path
use webrtc::ice_transport::ice_candidate::RTCIceCandidate; // Added for on_ice_candidate
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
    // Initialize env_logger. You can set RUST_LOG=info or RUST_LOG=debug for more verbosity
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("vtk-cube-client-console: starting up"); // Changed to info!

    // Connect to signaling server (replace with actual URL if needed)
    let signaling_url = "ws://127.0.0.1:8080";
    let url = Url::parse(signaling_url).expect("Invalid signaling server URL");
    match connect_async(url).await {
        Ok((mut ws_stream, _)) => {
            info!("Connected to signaling server at {}", signaling_url);

            // Add a small delay to allow the C++ server to connect to the signaling server first
            info!("Waiting for 3 seconds before sending offer...");
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

            // Set up WebRTC API and PeerConnection
            let mut m = MediaEngine::default();
            m.register_default_codecs().expect("Failed to register codecs");
            let registry = register_default_interceptors(Registry::new(), &mut m).unwrap();
            let api = APIBuilder::new()
                .with_media_engine(m)
                .with_interceptor_registry(registry)
                .build();
            info!("WebRTC API created");
            let config = RTCConfiguration::default();
            let pc = Arc::new(api.new_peer_connection(config).await.expect("Failed to create PeerConnection"));
            info!("PeerConnection created");

            // Add video transceiver (recvonly)
            pc.add_transceiver_from_kind(RTPCodecType::Video, None)
                .await
                .expect("Failed to add video transceiver");
            info!("Video transceiver added (recvonly)");

            // Log ICE candidates
            let pc_clone_for_ice = Arc::clone(&pc);
            pc.on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidate>| { // Changed RTCIceCandidateInit to RTCIceCandidate
                // let pc_clone_for_ice_inner = Arc::clone(&pc_clone_for_ice); // This variable was unused
                Box::pin(async move {
                    if let Some(c) = candidate { // Changed variable name to avoid conflict
                        match c.to_json() { // Corrected: to_json() is not async
                            Ok(json_candidate) => {
                                info!("Local ICE candidate gathered: {}", json_candidate.candidate);
                            }
                            Err(e) => {
                                log::error!("Failed to serialize ICE candidate to JSON: {}", e);
                            }
                        }
                        // In a real client, you'd send this to the remote peer via signaling
                        // For this example, we assume the server handles ICE negotiation primarily after offer/answer
                    } else {
                        info!("ICE candidate gathering complete.");
                    }
                })
            }));
            info!("on_ice_candidate handler set up");

            // Log ICE connection state changes
            pc.on_ice_connection_state_change(Box::new(|state: RTCIceConnectionState| {
                info!("ICE connection state changed: {}", state);
                Box::pin(async {})
            }));
            info!("on_ice_connection_state_change handler set up");

            // Set up on_track handler to log frame properties
            pc.on_track(Box::new(move |track: Arc<TrackRemote>, _receiver: Arc<RTCRtpReceiver>, _transceiver: Arc<RTCRtpTransceiver>| {
                info!("on_track handler triggered for track: Kind={}, ID={}, StreamID={}", track.kind(), track.id(), track.stream_id());
                Box::pin(async move {
                    info!("Received remote track: Kind={}, ID={}, Codec={:?}", track.kind(), track.id(), track.codec()); // Removed .await
                    loop {
                        match track.read_rtp().await {
                            Ok((rtp, _)) => {
                                info!( // Changed to info!
                                    "Frame received: SSRC={}, Timestamp={}, PayloadType={}, Size={} bytes",
                                    rtp.header.ssrc, rtp.header.timestamp, rtp.header.payload_type, rtp.payload.len()
                                );
                            }
                            Err(e) => {
                                log::error!("Error reading RTP packet from track {}: {}", track.id(), e);
                                break;
                            }
                        }
                    }
                    info!("Exiting on_track loop for track {}", track.id());
                })
            }));
            info!("on_track handler set up");

            // Create and send offer
            match pc.create_offer(None).await {
                Ok(offer) => {
                    info!("Offer created successfully");
                    match pc.set_local_description(offer.clone()).await {
                        Ok(_) => {
                            info!("Local description (offer) set successfully");
                            let offer_msg = SignalMessage::Offer { sdp: offer.sdp };
                            let msg_send = serde_json::to_string(&offer_msg).unwrap();
                            info!("Sending Offer to signaling server");
                            if let Err(e) = ws_stream.send(Message::Text(msg_send)).await {
                                log::error!("Failed to send offer: {}", e);
                                return; // Exit if sending offer fails
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to set local description (offer): {}", e);
                            return;
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to create offer: {}", e);
                    return; // Exit if creating offer fails
                }
            }

            // Signaling loop
            info!("Starting signaling loop...");
            while let Some(msg) = ws_stream.next().await {
                match msg {
                    Ok(Message::Text(txt)) => {
                        match serde_json::from_str::<SignalMessage>(&txt) {
                            Ok(signal) => { // Changed variable name from 'signal' to 'parsed_signal' to avoid conflict if needed, but direct use is fine.
                                match signal {
                                    SignalMessage::Offer { sdp: _ } => { // Marked sdp as unused with _
                                        // This client now sends the offer, so it should expect an Answer
                                        // However, a server might re-send an offer in some race conditions or complex scenarios.
                                        // For this example, we'll log if we get an unexpected offer.
                                        log::warn!("Received Offer from signaling server, but expected Answer. Ignoring offer for now.");
                                        // Optionally, handle this as an error or a new negotiation if the design requires it.
                                    }
                                    SignalMessage::Answer { sdp } => {
                                        info!("Received Answer from signaling server");
                                        match pc.set_remote_description(RTCSessionDescription::answer(sdp).expect("Failed to create answer SDP")).await {
                                            Ok(_) => info!("Remote description (answer) set successfully"),
                                            Err(e) => log::error!("Failed to set remote description (answer): {}", e),
                                        }
                                    }
                                    SignalMessage::IceCandidate { candidate, sdp_mid, sdp_mline_index } => {
                                        info!("Received ICE candidate from signaling server: Candidate={}, sdpMid={}, sdpMLineIndex={}", candidate, sdp_mid, sdp_mline_index);
                                        let ice = RTCIceCandidateInit {
                                            candidate,
                                            sdp_mid: Some(sdp_mid),
                                            sdp_mline_index: Some(sdp_mline_index as u16),
                                            username_fragment: None,
                                        };
                                        pc.add_ice_candidate(ice).await.expect("add_ice_candidate failed");
                                        info!("ICE candidate added successfully");
                                    }
                                    // Removed the unreachable '_' arm as SignalMessage is an enum with fixed variants.
                                    // If new variants are added to SignalMessage and not handled, the compiler will warn.
                                }
                            }
                            Err(_) => {
                                log::warn!("Failed to parse message from signaling server: {}. Original message: {}", txt, txt); // Log the original message on parse failure
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        info!("Signaling connection closed by server.");
                        break;
                    }
                    Err(e) => {
                        log::error!("Error receiving message from signaling server: {}", e);
                        break;
                    }
                    _ => { info!("Received other message type (e.g. Binary) from signaling server");}
                }
            }
            info!("Exited signaling loop.");
        }
        Err(e) => {
            eprintln!("Failed to connect to signaling server: {}", e);
        }
    }
}