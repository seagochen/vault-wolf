//! Async message reader for the IB TWS API.
//!
//! Spawns a tokio task that continuously reads messages from the server,
//! decodes them into `IBEvent`s, and sends them through an mpsc channel.
//!
//! Replaces C++ `EReader` (pthread + message queue + signal mechanism)
//! with Rust async/await + tokio::spawn + mpsc channel.

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::decoder::decode_server_msg;
use crate::errors::IBApiError;
use crate::transport::TransportReader;
use crate::wrapper::IBEvent;

// ============================================================================
// MessageReader
// ============================================================================

/// Async message reader that runs in a spawned tokio task.
///
/// Reads complete framed messages from the `TransportReader`, decodes them
/// into `IBEvent` variants, and sends them to the consumer through an
/// unbounded mpsc channel.
///
/// ## Usage
///
/// ```rust,ignore
/// let (transport_reader, transport_writer) = transport.into_split();
/// let reader = MessageReader::new(transport_reader, server_version);
/// let (rx, handle) = reader.spawn();
///
/// // Receive events
/// while let Some(event) = rx.recv().await {
///     println!("{event:?}");
/// }
/// ```
pub struct MessageReader {
    transport_reader: TransportReader,
    server_version: i32,
}

impl MessageReader {
    /// Create a new reader wrapping a `TransportReader`.
    pub fn new(transport_reader: TransportReader, server_version: i32) -> Self {
        Self {
            transport_reader,
            server_version,
        }
    }

    /// Spawn the reader task and return the event receiver + task handle.
    ///
    /// The spawned task runs until the connection closes or the receiver
    /// is dropped. Returns:
    /// - An unbounded receiver for consuming `IBEvent`s
    /// - A `JoinHandle` for waiting on or aborting the reader task
    pub fn spawn(self) -> (mpsc::UnboundedReceiver<IBEvent>, JoinHandle<()>) {
        let (tx, rx) = mpsc::unbounded_channel();

        let handle = tokio::spawn(async move {
            self.run(tx).await;
        });

        (rx, handle)
    }

    /// Main read loop. Runs until connection closes or receiver is dropped.
    async fn run(mut self, tx: mpsc::UnboundedSender<IBEvent>) {
        loop {
            match self.transport_reader.read_message().await {
                Ok(msg) => {
                    let event = decode_server_msg(&msg, self.server_version);
                    if tx.send(event).is_err() {
                        // Receiver dropped — stop reading
                        tracing::debug!("event receiver dropped, reader stopping");
                        break;
                    }
                }
                Err(IBApiError::Disconnected(reason)) => {
                    tracing::info!("server disconnected: {reason}");
                    let _ = tx.send(IBEvent::ConnectionClosed);
                    break;
                }
                Err(e) => {
                    tracing::error!("reader error: {e}");
                    let _ = tx.send(IBEvent::Error {
                        req_id: -1,
                        error_time: 0,
                        code: 0,
                        message: format!("reader error: {e}"),
                        advanced_order_reject_json: String::new(),
                    });
                    let _ = tx.send(IBEvent::ConnectionClosed);
                    break;
                }
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Build a framed message from null-terminated fields.
    fn build_framed_msg(fields: &[&str]) -> Vec<u8> {
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

    /// Create a mock TWS that completes handshake and sends given messages.
    async fn mock_tws_with_messages(
        sv: i32,
        messages: Vec<Vec<u8>>,
    ) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();

            // Read connect request
            let mut buf = vec![0u8; 512];
            let _ = stream.read(&mut buf).await.unwrap();

            // Send handshake response
            let handshake = build_framed_msg(&[&sv.to_string(), "20260101 12:00:00"]);
            stream.write_all(&handshake).await.unwrap();

            // Read start_api
            let _ = stream.read(&mut buf).await.unwrap();

            // Send test messages
            for msg in messages {
                stream.write_all(&msg).await.unwrap();
            }

            // Close connection (reader will get EOF)
            drop(stream);
        });

        tokio::task::yield_now().await;
        port
    }

    #[tokio::test]
    async fn reader_receives_events() {
        let messages = vec![
            // NEXT_VALID_ID: msg_id=9, version=1, orderId=100
            build_framed_msg(&["9", "1", "100"]),
            // MANAGED_ACCTS: msg_id=15, version=1, accounts="DU123"
            build_framed_msg(&["15", "1", "DU123"]),
        ];

        let port = mock_tws_with_messages(176, messages).await;

        let mut transport =
            crate::transport::Transport::connect("127.0.0.1", port, None)
                .await
                .unwrap();
        transport.start_api(0, None).await.unwrap();
        let sv = transport.server_version();
        let (reader_half, _writer_half) = transport.into_split();

        let reader = MessageReader::new(reader_half, sv);
        let (mut rx, handle) = reader.spawn();

        // Collect events
        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            events.push(event);
        }

        // Should have received: NextValidId, ManagedAccounts, ConnectionClosed
        assert!(events.len() >= 2, "expected at least 2 events, got {}", events.len());

        match &events[0] {
            IBEvent::NextValidId { order_id } => assert_eq!(*order_id, 100),
            other => panic!("expected NextValidId, got {other:?}"),
        }

        match &events[1] {
            IBEvent::ManagedAccounts { accounts } => assert_eq!(accounts, "DU123"),
            other => panic!("expected ManagedAccounts, got {other:?}"),
        }

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn reader_sends_connection_closed_on_disconnect() {
        // Empty messages = server immediately closes
        let port = mock_tws_with_messages(176, vec![]).await;

        let mut transport =
            crate::transport::Transport::connect("127.0.0.1", port, None)
                .await
                .unwrap();
        transport.start_api(0, None).await.unwrap();
        let sv = transport.server_version();
        let (reader_half, _writer_half) = transport.into_split();

        let reader = MessageReader::new(reader_half, sv);
        let (mut rx, handle) = reader.spawn();

        // Should get ConnectionClosed
        let event = rx.recv().await.unwrap();
        match event {
            IBEvent::ConnectionClosed => {}
            other => panic!("expected ConnectionClosed, got {other:?}"),
        }

        handle.await.unwrap();
    }

    #[tokio::test]
    async fn reader_stops_when_receiver_dropped() {
        let messages = vec![
            build_framed_msg(&["9", "1", "100"]),
            build_framed_msg(&["9", "1", "101"]),
            build_framed_msg(&["9", "1", "102"]),
        ];

        let port = mock_tws_with_messages(176, messages).await;

        let mut transport =
            crate::transport::Transport::connect("127.0.0.1", port, None)
                .await
                .unwrap();
        transport.start_api(0, None).await.unwrap();
        let sv = transport.server_version();
        let (reader_half, _writer_half) = transport.into_split();

        let reader = MessageReader::new(reader_half, sv);
        let (rx, handle) = reader.spawn();

        // Drop the receiver immediately
        drop(rx);

        // Reader task should finish cleanly
        handle.await.unwrap();
    }
}
