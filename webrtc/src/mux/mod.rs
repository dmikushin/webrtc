#[cfg(test)]
mod mux_test;

pub mod endpoint;
pub mod mux_func;

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use portable_atomic::AtomicUsize;
use tokio::sync::{mpsc, Mutex};
use util::{Buffer, Conn};

use crate::error::Result;
use crate::mux::endpoint::Endpoint;
use crate::mux::mux_func::MatchFunc;
use crate::util::Error;

/// mux multiplexes packets on a single socket (RFC7983)
///
/// The maximum amount of data that can be buffered before returning errors.
const MAX_BUFFER_SIZE: usize = 1000 * 1000; // 1MB

/// Config collects the arguments to mux.Mux construction into
/// a single structure
pub struct Config {
    pub conn: Arc<dyn Conn + Send + Sync>,
    pub buffer_size: usize,
}

/// Mux allows multiplexing
#[derive(Clone)]
pub struct Mux {
    id: Arc<AtomicUsize>,
    next_conn: Arc<dyn Conn + Send + Sync>,
    endpoints: Arc<Mutex<HashMap<usize, Arc<Endpoint>>>>,
    buffer_size: usize,
    closed_ch_tx: Option<mpsc::Sender<()>>,
}

impl Mux {
    pub fn new(config: Config) -> Self {
        let (closed_ch_tx, closed_ch_rx) = mpsc::channel(1);
        let m = Mux {
            id: Arc::new(AtomicUsize::new(0)),
            next_conn: Arc::clone(&config.conn),
            endpoints: Arc::new(Mutex::new(HashMap::new())),
            buffer_size: config.buffer_size,
            closed_ch_tx: Some(closed_ch_tx),
        };

        let buffer_size = m.buffer_size;
        let next_conn = Arc::clone(&m.next_conn);
        let endpoints = Arc::clone(&m.endpoints);
        tokio::spawn(async move {
            Mux::read_loop(buffer_size, next_conn, closed_ch_rx, endpoints).await;
        });

        m
    }

    /// creates a new Endpoint
    pub async fn new_endpoint(&self, f: MatchFunc) -> Arc<Endpoint> {
        let mut endpoints = self.endpoints.lock().await;

        let id = self.id.fetch_add(1, Ordering::SeqCst);
        // Set a maximum size of the buffer in bytes.
        let e = Arc::new(Endpoint {
            id,
            buffer: Buffer::new(0, MAX_BUFFER_SIZE),
            match_fn: f,
            next_conn: Arc::clone(&self.next_conn),
            endpoints: Arc::clone(&self.endpoints),
        });

        endpoints.insert(e.id, Arc::clone(&e));

        e
    }

    /// remove_endpoint removes an endpoint from the Mux
    pub async fn remove_endpoint(&mut self, e: &Endpoint) {
        let mut endpoints = self.endpoints.lock().await;
        endpoints.remove(&e.id);
    }

    /// Close closes the Mux and all associated Endpoints.
    pub async fn close(&mut self) {
        self.closed_ch_tx.take();

        let mut endpoints = self.endpoints.lock().await;
        endpoints.clear();
    }

    async fn read_loop(
        buffer_size: usize,
        next_conn: Arc<dyn Conn + Send + Sync>,
        mut closed_ch_rx: mpsc::Receiver<()>,
        endpoints: Arc<Mutex<HashMap<usize, Arc<Endpoint>>>>,
    ) {
        let mut buf = vec![0u8; buffer_size];
        let mut n = 0usize;
        loop {
            tokio::select! {
                _ = closed_ch_rx.recv() => break,
                result = next_conn.recv(&mut buf) => {
                    if let Ok(m) = result{
                        n = m;
                    }
                }
            };

            if let Err(err) = Mux::dispatch(&buf[..n], &endpoints).await {
                log::error!("mux: ending readLoop dispatch error {:?}", err);
                break;
            }
        }
    }

    async fn dispatch(
        buf: &[u8],
        endpoints: &Arc<Mutex<HashMap<usize, Arc<Endpoint>>>>,
    ) -> Result<()> {
        let mut endpoint = None;

        log::warn!("[Mux dispatch] Received packet of {} bytes", buf.len());
        if !buf.is_empty() {
            log::warn!("[Mux dispatch] Packet starts with byte value: {}", buf[0]);
        }

        {
            let eps = endpoints.lock().await;
            log::warn!("[Mux dispatch] Looking for matching endpoint among {} registered endpoints", eps.len());
            
            for (id, ep) in eps.iter() {
                let matches = (ep.match_fn)(buf);
                log::warn!("[Mux dispatch] Testing endpoint {}: match = {}", id, matches);
                if matches {
                    endpoint = Some(Arc::clone(ep));
                    break;
                }
            }
        }

        if let Some(ep) = endpoint {
            log::warn!("[Mux dispatch] Found matching endpoint (id: {}), writing {} bytes to buffer", ep.id, buf.len());
            match ep.buffer.write(buf).await {
                // Expected when bytes are received faster than the endpoint can process them
                Err(Error::ErrBufferFull) => {
                    log::warn!("[Mux dispatch] Endpoint buffer is full, dropping packet")
                }
                Ok(n) => log::warn!("[Mux dispatch] Successfully wrote {} bytes to endpoint buffer", n),
                Err(e) => {
                    log::error!("[Mux dispatch] Error writing to endpoint buffer: {:?}", e);
                    return Err(crate::Error::Util(e));
                }
            }
        } else if !buf.is_empty() {
            log::warn!(
                "[Mux dispatch] No matching endpoint for packet starting with byte {}",
                buf[0]
            );
        } else {
            log::warn!("[Mux dispatch] No matching endpoint for zero length packet");
        }

        Ok(())
    }
}
