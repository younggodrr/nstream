use bytes::Bytes;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info, warn};

use nstream_core::{JobId, MediaPayload, NStreamError};

/// A raw TCP test source for development. Production RTMP/RTMPS/SRT protocol
/// ingest is provided by `LiveIngestSession`, which delegates framing to FFmpeg.
pub struct RtmpIngest {
    /// Maximum number of bytes to read per connection before wrapping into a payload.
    pub max_read_bytes: usize,
}

impl RtmpIngest {
    pub fn new(max_read_bytes: usize) -> Self {
        Self { max_read_bytes }
    }

    /// Start listening on `addr` and handle incoming connections.
    /// Each connection reads up to `max_read_bytes` and wraps them in a `MediaPayload`.
    pub async fn listen(&self, addr: &str) -> Result<(), NStreamError> {
        let listener = TcpListener::bind(addr).await?;
        info!(addr = addr, "RtmpIngest listening");

        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    info!(peer = %peer_addr, "Accepted RTMP connection");
                    let max_bytes = self.max_read_bytes;
                    tokio::spawn(async move {
                        if let Err(e) =
                            Self::handle_connection(stream, peer_addr.to_string(), max_bytes).await
                        {
                            error!(peer = %peer_addr, error = %e, "RTMP connection handler failed");
                        }
                    });
                }
                Err(e) => {
                    warn!(error = %e, "Failed to accept RTMP connection");
                }
            }
        }
    }

    /// Handle a single RTMP TCP connection.
    async fn handle_connection(
        mut stream: TcpStream,
        peer: String,
        max_bytes: usize,
    ) -> Result<MediaPayload, NStreamError> {
        let mut buf = vec![0u8; max_bytes];
        let n = stream.read(&mut buf).await?;
        buf.truncate(n);

        debug!(peer = %peer, bytes_read = n, "Read raw bytes from RTMP connection");

        let job_id = JobId::new();
        let payload = MediaPayload::new(job_id, Bytes::from(buf))
            .with_metadata("ingest_source", "rtmp")
            .with_metadata("peer_addr", peer);

        info!(job_id = %job_id, bytes = n, "Created MediaPayload from RTMP stream");
        Ok(payload)
    }
}
