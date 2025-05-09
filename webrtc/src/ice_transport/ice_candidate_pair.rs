use std::fmt;

use crate::ice_transport::ice_candidate::*;

/// ICECandidatePair represents an ICE Candidate pair
///
/// ## Specifications
///
/// * [MDN]
///
/// [MDN]: https://developer.mozilla.org/en-US/docs/Web/API/RTCIceCandidatePair
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct RTCIceCandidatePair {
    stats_id: String,
    local: RTCIceCandidate,
    remote: RTCIceCandidate,
}

impl fmt::Display for RTCIceCandidatePair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(local) {} <-> (remote) {}", self.local, self.remote)
    }
}

impl RTCIceCandidatePair {
    fn stats_id(local_id: &str, remote_id: &str) -> String {
        format!("{local_id}-{remote_id}")
    }

    /// returns an initialized ICECandidatePair
    /// for the given pair of ICECandidate instances
    pub fn new(local: RTCIceCandidate, remote: RTCIceCandidate) -> Self {
        let stats_id = Self::stats_id(&local.stats_id, &remote.stats_id);
        RTCIceCandidatePair {
            stats_id,
            local,
            remote,
        }
    }

    /// Returns a reference to the local candidate
    pub fn local_candidate(&self) -> &RTCIceCandidate {
        &self.local
    }

    /// Returns a reference to the remote candidate
    pub fn remote_candidate(&self) -> &RTCIceCandidate {
        &self.remote
    }
}
