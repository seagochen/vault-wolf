//! Async TCP transport for the IB TWS API protocol.
//!
//! Handles V100+ message framing (4-byte big-endian length prefix), the
//! connection handshake, and reading/writing complete messages.
//!
//! Ported from: `EClientSocket` (connect, framing), `ESocket` (TCP send/recv),
//! `EReader` (message reading), `EClient::sendConnectRequest` / `startApi`.

use bytes::{Buf, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

use crate::decoder::MessageDecoder;
use crate::encoder::{build_connect_request, MessageEncoder};
use crate::errors::{IBApiError, Result};
use crate::protocol::{
    HEADER_LEN, MAX_CLIENT_VER, MAX_MSG_LEN, MIN_CLIENT_VER, outgoing, server_version,
};

// ============================================================================
// Connection State
// ============================================================================

/// Connection state of the transport.
///
/// Mirrors C++ `EClient::ConnState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnState {
    Disconnected,
    Connecting,
    Connected,
}

// ============================================================================
// Transport
// ============================================================================

/// Async TCP transport for the IB TWS API protocol.
///
/// Handles V100+ message framing: each message is prefixed with a 4-byte
/// big-endian length. The transport reads complete messages from the server
/// and sends framed messages to the server.
///
/// Replaces C++ `EClientSocket` + `ESocket` + `EReader` (read loop).
pub struct Transport {
    reader: OwnedReadHalf,
    writer: OwnedWriteHalf,
    read_buf: BytesMut,
    server_version: i32,
    tws_time: String,
    conn_state: ConnState,
}

impl Transport {
    /// Connect to TWS/Gateway at the given host and port, perform the V100+
    /// handshake, and return a ready-to-use Transport.
    ///
    /// This mirrors the combined logic of:
    /// - C++ `EClientSocket::eConnect` (TCP connection)
    /// - C++ `EClient::sendConnectRequest` (send "API\0" + version range)
    /// - C++ `EDecoder::processConnectAck` (read server version + time)
    ///
    /// The `start_api()` call (sending client ID) is separate — call it
    /// after connect returns.
    pub async fn connect(
        host: &str,
        port: u16,
        connect_options: Option<&str>,
    ) -> Result<Self> {
        // 1. TCP connect
        let addr = format!("{host}:{port}");
        let stream = TcpStream::connect(&addr).await.map_err(|e| {
            IBApiError::Connection(format!("failed to connect to {addr}: {e}"))
        })?;

        let (reader, writer) = stream.into_split();
        let mut transport = Self {
            reader,
            writer,
            read_buf: BytesMut::with_capacity(8192),
            server_version: 0,
            tws_time: String::new(),
            conn_state: ConnState::Connecting,
        };

        // 2. Send connect request: "API\0" + [4-byte length] + "v100..203"
        transport.send_connect_request(connect_options).await?;

        // 3. Read handshake response and validate
        transport.process_connect_ack().await?;

        Ok(transport)
    }

    // ========================================================================
    // Handshake
    // ========================================================================

    /// Send the V100+ connect request.
    ///
    /// Wire format: `"API\0"` + `[4-byte BE length]` + `"v100..203"`.
    /// Mirrors C++ `EClient::sendConnectRequest`.
    async fn send_connect_request(
        &mut self,
        connect_options: Option<&str>,
    ) -> Result<()> {
        let bytes = build_connect_request(connect_options)?;
        self.writer.write_all(&bytes).await.map_err(|e| {
            IBApiError::Connection(format!(
                "failed to send connect request: {e}"
            ))
        })?;
        Ok(())
    }

    /// Read and process the connect acknowledgment from the server.
    ///
    /// Server sends: `[4-byte length][server_version\0][tws_time\0]`
    /// or for redirect: `[4-byte length][negative_version\0][host:port\0]`.
    ///
    /// Mirrors C++ `EDecoder::processConnectAck`.
    async fn process_connect_ack(&mut self) -> Result<()> {
        let msg = self.read_message().await?;
        let mut dec = MessageDecoder::new(&msg, 0);

        // First field: server version (or negative for redirect)
        let sv = dec.decode_i32()?;

        if sv < 0 {
            // Redirect: read host:port
            let hostport = dec.decode_string()?;
            return Err(IBApiError::Protocol(format!(
                "server redirect to {hostport}"
            )));
        }

        // Validate server version is within our supported range
        if !(MIN_CLIENT_VER..=MAX_CLIENT_VER).contains(&sv) {
            return Err(IBApiError::Protocol(format!(
                "unsupported server version {sv} (expected {MIN_CLIENT_VER}..{MAX_CLIENT_VER})"
            )));
        }

        // Read TWS time (always present for V100+ servers)
        let tws_time = dec.decode_string()?;

        self.server_version = sv;
        self.tws_time = tws_time;
        self.conn_state = ConnState::Connected;

        tracing::info!(
            server_version = sv,
            tws_time = %self.tws_time,
            "IB TWS API handshake complete"
        );

        Ok(())
    }

    // ========================================================================
    // Message Reading (V100+ framing)
    // ========================================================================

    /// Read a single complete message from the server.
    ///
    /// V100+ framing: `[4-byte BE length][message body of that length]`.
    /// Returns the message body (without the length header).
    ///
    /// Handles TCP fragmentation by accumulating data in the internal read
    /// buffer until a complete frame is available.
    ///
    /// Mirrors C++ `EReader::readSingleMsg`.
    pub async fn read_message(&mut self) -> Result<Vec<u8>> {
        // Ensure we have at least 4 bytes for the length header
        while self.read_buf.len() < HEADER_LEN {
            let n = self.reader.read_buf(&mut self.read_buf).await?;
            if n == 0 {
                return Err(IBApiError::Disconnected(
                    "connection closed while reading message header".into(),
                ));
            }
        }

        // Parse the message length
        let len_bytes: [u8; 4] = self.read_buf[..4].try_into().unwrap();
        let msg_len = u32::from_be_bytes(len_bytes) as usize;

        if msg_len == 0 || msg_len > MAX_MSG_LEN {
            return Err(IBApiError::Protocol(format!(
                "invalid message length: {msg_len}"
            )));
        }

        // Read until we have the complete message
        let total_needed = HEADER_LEN + msg_len;
        while self.read_buf.len() < total_needed {
            let n = self.reader.read_buf(&mut self.read_buf).await?;
            if n == 0 {
                return Err(IBApiError::Disconnected(
                    "connection closed while reading message body".into(),
                ));
            }
        }

        // Extract the message body, advancing the buffer
        self.read_buf.advance(HEADER_LEN);
        let msg = self.read_buf.split_to(msg_len).to_vec();
        Ok(msg)
    }

    // ========================================================================
    // Message Sending
    // ========================================================================

    /// Send a pre-encoded, framed message to the server.
    ///
    /// The `data` should already include the 4-byte length header
    /// (as produced by `MessageEncoder::finalize()`).
    pub async fn send_message(&mut self, data: &[u8]) -> Result<()> {
        self.writer.write_all(data).await.map_err(|e| {
            IBApiError::Connection(format!("failed to send message: {e}"))
        })?;
        Ok(())
    }

    /// Build and send the START_API message.
    ///
    /// This must be called after a successful `connect()` to complete the
    /// initialization sequence. Sends the client ID and optional capabilities.
    ///
    /// Mirrors C++ `EClient::startApi`.
    pub async fn start_api(
        &mut self,
        client_id: i32,
        optional_capabilities: Option<&str>,
    ) -> Result<()> {
        let mut enc = MessageEncoder::new(self.server_version);
        enc.encode_msg_id(outgoing::START_API);
        enc.encode_field_i32(2); // VERSION = 2
        enc.encode_field_i32(client_id);

        if self.server_version >= server_version::OPTIONAL_CAPABILITIES {
            enc.encode_field_str(optional_capabilities.unwrap_or(""));
        }

        let bytes = enc.finalize()?;
        self.send_message(&bytes).await
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Negotiated server version from the handshake.
    pub fn server_version(&self) -> i32 {
        self.server_version
    }

    /// TWS connection time string from the handshake.
    pub fn tws_time(&self) -> &str {
        &self.tws_time
    }

    /// Current connection state.
    pub fn conn_state(&self) -> ConnState {
        self.conn_state
    }

    /// Whether the transport is in the Connected state.
    pub fn is_connected(&self) -> bool {
        self.conn_state == ConnState::Connected
    }

    /// Disconnect from the server by shutting down the writer.
    pub async fn disconnect(&mut self) {
        self.conn_state = ConnState::Disconnected;
        let _ = self.writer.shutdown().await;
    }

    // ========================================================================
    // Split for concurrent read/write (Phase 3)
    // ========================================================================

    /// Split the transport into separate reader and writer halves.
    ///
    /// This is needed for running read and write loops concurrently
    /// in separate tokio tasks (Phase 3 client architecture).
    pub fn into_split(self) -> (TransportReader, TransportWriter) {
        (
            TransportReader {
                reader: self.reader,
                read_buf: self.read_buf,
                server_version: self.server_version,
            },
            TransportWriter {
                writer: self.writer,
                server_version: self.server_version,
            },
        )
    }
}

// ============================================================================
// TransportReader
// ============================================================================

/// Read half of a split transport.
///
/// Owns the TCP read half and the read buffer. Provides `read_message()`
/// for reading complete framed messages from the server.
pub struct TransportReader {
    reader: OwnedReadHalf,
    read_buf: BytesMut,
    server_version: i32,
}

impl TransportReader {
    pub fn server_version(&self) -> i32 {
        self.server_version
    }

    /// Read a single complete message from the server.
    ///
    /// Same logic as `Transport::read_message`.
    pub async fn read_message(&mut self) -> Result<Vec<u8>> {
        // Read length header
        while self.read_buf.len() < HEADER_LEN {
            let n = self.reader.read_buf(&mut self.read_buf).await?;
            if n == 0 {
                return Err(IBApiError::Disconnected(
                    "connection closed while reading message header".into(),
                ));
            }
        }

        let len_bytes: [u8; 4] = self.read_buf[..4].try_into().unwrap();
        let msg_len = u32::from_be_bytes(len_bytes) as usize;

        if msg_len == 0 || msg_len > MAX_MSG_LEN {
            return Err(IBApiError::Protocol(format!(
                "invalid message length: {msg_len}"
            )));
        }

        // Read message body
        let total_needed = HEADER_LEN + msg_len;
        while self.read_buf.len() < total_needed {
            let n = self.reader.read_buf(&mut self.read_buf).await?;
            if n == 0 {
                return Err(IBApiError::Disconnected(
                    "connection closed while reading message body".into(),
                ));
            }
        }

        self.read_buf.advance(HEADER_LEN);
        let msg = self.read_buf.split_to(msg_len).to_vec();
        Ok(msg)
    }
}

// ============================================================================
// TransportWriter
// ============================================================================

/// Write half of a split transport.
///
/// Owns the TCP write half. Provides `send_message()` for sending
/// pre-encoded framed messages to the server.
pub struct TransportWriter {
    writer: OwnedWriteHalf,
    server_version: i32,
}

impl TransportWriter {
    pub fn server_version(&self) -> i32 {
        self.server_version
    }

    /// Send a pre-encoded, framed message to the server.
    pub async fn send_message(&mut self, data: &[u8]) -> Result<()> {
        self.writer.write_all(data).await.map_err(|e| {
            IBApiError::Connection(format!("failed to send: {e}"))
        })?;
        Ok(())
    }

    /// Shut down the write half of the TCP connection.
    ///
    /// Sends a TCP FIN to the server. After this, the reader will eventually
    /// receive EOF when the server closes its side.
    pub async fn shutdown(&mut self) {
        let _ = self.writer.shutdown().await;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    /// Build a framed server response from null-terminated fields.
    fn build_framed_response(fields: &[&str]) -> Vec<u8> {
        let mut body = Vec::new();
        for f in fields {
            body.extend_from_slice(f.as_bytes());
            body.push(0);
        }
        let mut frame = Vec::new();
        frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
        frame.extend(body);
        frame
    }

    /// Create a mock TWS server that accepts one connection,
    /// reads the connect request, and sends a handshake response.
    /// Returns the port the server is listening on.
    async fn mock_tws_handshake(sv: i32, tws_time: &str) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let time_owned = tws_time.to_string();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();

            // Read the connect request (we don't validate it in tests)
            let mut buf = vec![0u8; 256];
            let _ = stream.read(&mut buf).await.unwrap();

            // Send handshake response
            let response = build_framed_response(&[
                &sv.to_string(),
                &time_owned,
            ]);
            stream.write_all(&response).await.unwrap();
        });

        // Brief yield to let the server start listening
        tokio::task::yield_now().await;
        port
    }

    #[tokio::test]
    async fn connect_and_handshake() {
        let port = mock_tws_handshake(176, "20260101 12:00:00 EST").await;

        let transport = Transport::connect("127.0.0.1", port, None)
            .await
            .unwrap();
        assert_eq!(transport.server_version(), 176);
        assert_eq!(transport.tws_time(), "20260101 12:00:00 EST");
        assert!(transport.is_connected());
        assert_eq!(transport.conn_state(), ConnState::Connected);
    }

    #[tokio::test]
    async fn connect_unsupported_version_too_low() {
        let port = mock_tws_handshake(50, "time").await;

        let result = Transport::connect("127.0.0.1", port, None).await;
        match result {
            Err(e) => assert!(
                e.to_string().contains("unsupported server version"),
                "unexpected error: {e}"
            ),
            Ok(_) => panic!("expected error for unsupported version"),
        }
    }

    #[tokio::test]
    async fn connect_unsupported_version_too_high() {
        let port = mock_tws_handshake(999, "time").await;

        let result = Transport::connect("127.0.0.1", port, None).await;
        match result {
            Err(e) => assert!(
                e.to_string().contains("unsupported server version"),
                "unexpected error: {e}"
            ),
            Ok(_) => panic!("expected error for unsupported version"),
        }
    }

    #[tokio::test]
    async fn connect_redirect() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 256];
            let _ = stream.read(&mut buf).await.unwrap();

            // Send negative version (redirect)
            let response = build_framed_response(&["-1", "10.0.0.1:4002"]);
            stream.write_all(&response).await.unwrap();
        });

        tokio::task::yield_now().await;

        let result = Transport::connect("127.0.0.1", port, None).await;
        match result {
            Err(e) => assert!(
                e.to_string().contains("redirect"),
                "unexpected error: {e}"
            ),
            Ok(_) => panic!("expected redirect error"),
        }
    }

    #[tokio::test]
    async fn connect_refused() {
        // Port 1 is almost certainly not listening
        let result = Transport::connect("127.0.0.1", 1, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn read_message_after_handshake() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 256];
            let _ = stream.read(&mut buf).await.unwrap();

            // Send handshake
            let handshake = build_framed_response(&["176", "20260101 12:00:00"]);
            stream.write_all(&handshake).await.unwrap();

            // Send a test message: msg_id=9 (NEXT_VALID_ID), version=1, orderId=100
            let msg = build_framed_response(&["9", "1", "100"]);
            stream.write_all(&msg).await.unwrap();
        });

        tokio::task::yield_now().await;

        let mut transport = Transport::connect("127.0.0.1", port, None)
            .await
            .unwrap();
        assert_eq!(transport.server_version(), 176);

        // Read the test message
        let msg = transport.read_message().await.unwrap();
        let mut dec = MessageDecoder::new(&msg, 176);
        assert_eq!(dec.decode_i32().unwrap(), 9);   // msg_id = NEXT_VALID_ID
        assert_eq!(dec.decode_i32().unwrap(), 1);   // version
        assert_eq!(dec.decode_i32().unwrap(), 100); // orderId
    }

    #[tokio::test]
    async fn send_message_test() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 256];

            // Read connect request
            let _ = stream.read(&mut buf).await.unwrap();

            // Send handshake
            let handshake = build_framed_response(&["176", "20260101 12:00:00"]);
            stream.write_all(&handshake).await.unwrap();

            // Read the message sent by client
            let mut msg_buf = vec![0u8; 256];
            let n = stream.read(&mut msg_buf).await.unwrap();
            msg_buf.truncate(n);
            msg_buf
        });

        tokio::task::yield_now().await;

        let mut transport = Transport::connect("127.0.0.1", port, None)
            .await
            .unwrap();

        // Build and send a test message
        let mut enc = MessageEncoder::new(176);
        enc.encode_field_i32(49); // REQ_CURRENT_TIME
        enc.encode_field_i32(1);  // version
        let bytes = enc.finalize().unwrap();
        transport.send_message(&bytes).await.unwrap();

        // Verify the server received it correctly
        let received = handle.await.unwrap();
        assert_eq!(&received[..], &bytes[..]);
    }

    #[tokio::test]
    async fn start_api_message() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 256];
            let _ = stream.read(&mut buf).await.unwrap();

            // Send handshake (version 176 supports OPTIONAL_CAPABILITIES=72)
            let handshake = build_framed_response(&["176", "20260101 12:00:00"]);
            stream.write_all(&handshake).await.unwrap();

            // Read start_api message
            let mut msg_buf = vec![0u8; 256];
            let n = stream.read(&mut msg_buf).await.unwrap();
            msg_buf.truncate(n);
            msg_buf
        });

        tokio::task::yield_now().await;

        let mut transport = Transport::connect("127.0.0.1", port, None)
            .await
            .unwrap();
        transport.start_api(0, None).await.unwrap();

        let received = handle.await.unwrap();
        // Skip the 4-byte length header, parse the body
        let body = &received[HEADER_LEN..];
        let mut dec = MessageDecoder::new(body, 176);
        assert_eq!(dec.decode_i32().unwrap(), 71);  // START_API msg id
        assert_eq!(dec.decode_i32().unwrap(), 2);   // version
        assert_eq!(dec.decode_i32().unwrap(), 0);   // client_id
        assert_eq!(dec.decode_string().unwrap(), ""); // optional_capabilities (empty)
    }

    #[tokio::test]
    async fn into_split() {
        let port = mock_tws_handshake(176, "20260101 12:00:00").await;

        let transport = Transport::connect("127.0.0.1", port, None)
            .await
            .unwrap();

        let (reader, writer) = transport.into_split();
        assert_eq!(reader.server_version(), 176);
        assert_eq!(writer.server_version(), 176);
    }

    #[tokio::test]
    async fn disconnect() {
        let port = mock_tws_handshake(176, "20260101 12:00:00").await;

        let mut transport = Transport::connect("127.0.0.1", port, None)
            .await
            .unwrap();
        assert!(transport.is_connected());

        transport.disconnect().await;
        assert!(!transport.is_connected());
        assert_eq!(transport.conn_state(), ConnState::Disconnected);
    }
}
