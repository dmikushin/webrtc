use tokio::sync::mpsc;
use util::marshal::*;
use util::Buffer;

use crate::error::{Error, Result};
use interceptor::{RTPReader, RTCPReader, Attributes}; // Added RTCPReader
use async_trait::async_trait;

/// Limit the buffer size to 1MB
pub const SRTP_BUFFER_SIZE: usize = 1000 * 1000;

/// Limit the buffer size to 100KB
pub const SRTCP_BUFFER_SIZE: usize = 100 * 1000;

/// Stream handles decryption for a single RTP/RTCP SSRC
#[derive(Debug)]
pub struct Stream {
    ssrc: u32,
    tx: mpsc::Sender<u32>,
    pub(crate) buffer: Buffer,
    is_rtp: bool,
}

impl Stream {
    /// Create a new stream
    pub fn new(ssrc: u32, tx: mpsc::Sender<u32>, is_rtp: bool) -> Self {
        Stream {
            ssrc,
            tx,
            // Create a buffer with a 1MB limit
            buffer: Buffer::new(
                0,
                if is_rtp {
                    SRTP_BUFFER_SIZE
                } else {
                    SRTCP_BUFFER_SIZE
                },
            ),
            is_rtp,
        }
    }

    /// GetSSRC returns the SSRC we are demuxing for
    pub fn get_ssrc(&self) -> u32 {
        return self.ssrc;
    }

    /// Check if RTP is a stream.
    pub fn is_rtp_stream(&self) -> bool {
        return self.is_rtp;
    }

    /// Read reads and decrypts full RTP packet from the nextConn
    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        log::warn!("[SRTP Stream {}] read: called", self.ssrc); // Changed to warn!
        let result = self.buffer.read(buf, None).await;
        log::warn!("[SRTP Stream {}] read: buffer.read result: {:?}", self.ssrc, &result.as_ref().map_err(|e| format!("{:?}", e))); // Changed to warn!
        Ok(result?)
    }

    /// ReadRTP reads and decrypts full RTP packet and its header from the nextConn
    pub async fn read_rtp(&self, buf: &mut [u8]) -> Result<rtp::packet::Packet> {
        log::warn!(
            "[SRTP Stream {}] read_rtp: called. Buffer capacity: {}.",
            self.ssrc,
            buf.len()
        );
        if !self.is_rtp {
            log::error!("[SRTP Stream {}] read_rtp: attempt to read_rtp on non-RTP stream", self.ssrc);
            return Err(Error::InvalidRtpStream);
        }

        log::trace!("[SRTP Stream {}] read_rtp: calling buffer.read", self.ssrc);
        let n = self.buffer.read(buf, None).await.map_err(|e| {
            log::error!("[SRTP Stream {}] read_rtp: buffer.read error: {:?}", self.ssrc, e);
            e
        })?;
        log::warn!("[SRTP Stream {}] read_rtp: buffer.read returned {} bytes.", self.ssrc, n);

        let mut b = &buf[..n];
        let pkt = rtp::packet::Packet::unmarshal(&mut b).map_err(|e| {
            log::error!("[SRTP Stream {}] read_rtp: rtp::packet::Packet::unmarshal error: {:?}", self.ssrc, e);
            e
        })?;
        log::warn!("[SRTP Stream {}] read_rtp: successfully unmarshalled packet (PT: {}, Seq: {}, SSRC: {}) (WARN)", self.ssrc, pkt.header.payload_type, pkt.header.sequence_number, pkt.header.ssrc); // Changed to warn!

        Ok(pkt)
    }

    /// read_rtcp reads and decrypts full RTP packet and its header from the nextConn
    pub async fn read_rtcp(
        &self,
        buf: &mut [u8],
    ) -> Result<Vec<Box<dyn rtcp::packet::Packet + Send + Sync>>> {
        log::warn!(
            "[SRTP Stream {}] read_rtcp: called. Buffer capacity: {}.",
            self.ssrc,
            buf.len()
        );
        if self.is_rtp {
            return Err(Error::InvalidRtcpStream);
        }

        let n = self.buffer.read(buf, None).await?;
        log::warn!("[SRTP Stream {}] read_rtcp: buffer.read returned {} bytes.", self.ssrc, n);
        let mut b = &buf[..n];
        let pkt = rtcp::packet::unmarshal(&mut b)?;

        Ok(pkt)
    }

    /// Close removes the ReadStream from the session and cleans up any associated state
    pub async fn close(&self) -> Result<()> {
        self.buffer.close().await;
        let _ = self.tx.send(self.ssrc).await;
        Ok(())
    }
}

// We need to use interceptor's Result and Error types for the trait implementation
type InterceptorResult<T> = std::result::Result<T, interceptor::Error>;

#[async_trait]
impl RTPReader for Stream {
    async fn read(&self, buf: &mut [u8], a: &Attributes) -> InterceptorResult<(rtp::packet::Packet, Attributes)> {
        log::warn!(
            "[SRTP Stream {}] RTPReader::read called. Buffer capacity: {}.",
            self.ssrc,
            buf.len()
        );
        
        // Map our error type to interceptor::Error
        let pkt = self.read_rtp(buf).await.map_err(|e| {
            interceptor::Error::Other(format!("SRTP error: {}", e))
        })?;
        
        log::warn!("[SRTP Stream {}] RTPReader::read successful, got packet PT: {}, Seq: {}, SSRC: {}", self.ssrc, pkt.header.payload_type, pkt.header.sequence_number, pkt.header.ssrc);
        Ok((pkt, a.clone()))
    }
}

#[async_trait]
impl RTCPReader for Stream {
    async fn read(&self, buf: &mut [u8], a: &Attributes) -> InterceptorResult<(Vec<Box<dyn rtcp::packet::Packet + Send + Sync>>, Attributes)> {
        log::warn!(
            "[SRTP Stream {}] RTCPReader::read called. Buffer capacity: {}.",
            self.ssrc,
            buf.len()
        );
        
        // Map our error type to interceptor::Error
        let pkts = self.read_rtcp(buf).await.map_err(|e| {
            interceptor::Error::Other(format!("SRTP error: {}", e))
        })?;
        
        log::warn!("[SRTP Stream {}] RTCPReader::read successful, got {} RTCP packets", 
            self.ssrc, pkts.len());
        Ok((pkts, a.clone()))
    }
}
