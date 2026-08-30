//! Tokio byte-stream adapter for reliable WebRTC data channels.

use std::future::Future as _;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;

use async_trait::async_trait;
use bytes::Buf as _;
use bytes::Bytes;
use bytes::BytesMut;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::ReadBuf;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::PollSender;
use webrtc::data_channel::DataChannel;
use webrtc::data_channel::DataChannelEvent;

/// Maximum payload placed in one WebRTC data-channel message.
pub const DATA_CHANNEL_FRAME_SIZE: usize = 16 * 1024;

const READ_QUEUE_CAPACITY: usize = 16;
const WRITE_QUEUE_CAPACITY: usize = 8;

/// A reliable WebRTC data channel exposed as a Tokio byte stream.
///
/// Incoming WebRTC messages are concatenated, while writes are fragmented into
/// [`DATA_CHANNEL_FRAME_SIZE`] messages. Both directions use bounded queues.
pub struct DataChannelIo {
    read_rx: mpsc::Receiver<ReadEvent>,
    read_buffer: Bytes,
    write_tx: PollSender<WriteCommand>,
    opened_rx: Option<oneshot::Receiver<Result<(), StoredError>>>,
    flush_rx: Option<oneshot::Receiver<Result<(), StoredError>>>,
    shutdown_rx: Option<oneshot::Receiver<Result<(), StoredError>>>,
    shutdown_complete: bool,
    terminal_error: Arc<Mutex<Option<StoredError>>>,
}

impl DataChannelIo {
    /// Starts adapting `data_channel` without waiting for its open event.
    pub fn new(data_channel: Arc<dyn DataChannel>) -> Self {
        Self::from_transport(Arc::new(WebRtcTransport(data_channel)))
    }

    /// Resolves only after the WebRTC data channel reports that it is open.
    pub async fn wait_open(mut self) -> io::Result<Self> {
        let Some(opened_rx) = self.opened_rx.take() else {
            return Ok(self);
        };
        opened_rx
            .await
            .map_err(|_| self.current_error("Data channel closed before opening"))?
            .map_err(StoredError::into_io)?;
        Ok(self)
    }

    fn from_transport(transport: Arc<dyn DataChannelTransport>) -> Self {
        let (read_tx, read_rx) = mpsc::channel(READ_QUEUE_CAPACITY);
        let (write_tx, write_rx) = mpsc::channel(WRITE_QUEUE_CAPACITY);
        let (opened_tx, opened_rx) = oneshot::channel();
        let terminal_error = Arc::new(Mutex::new(None));

        tokio::spawn(read_data_channel(
            transport.clone(),
            read_tx.clone(),
            opened_tx,
            terminal_error.clone(),
        ));
        tokio::spawn(write_data_channel(
            transport,
            write_rx,
            read_tx,
            terminal_error.clone(),
        ));

        Self {
            read_rx,
            read_buffer: Bytes::new(),
            write_tx: PollSender::new(write_tx),
            opened_rx: Some(opened_rx),
            flush_rx: None,
            shutdown_rx: None,
            shutdown_complete: false,
            terminal_error,
        }
    }

    fn stored_error(&self) -> Option<StoredError> {
        self.terminal_error.lock().expect("terminal_error").clone()
    }

    fn current_error(&self, fallback: &'static str) -> io::Error {
        self.stored_error()
            .map(StoredError::into_io)
            .unwrap_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, fallback))
    }

    fn poll_send_command(
        &mut self,
        context: &mut Context<'_>,
        command: impl FnOnce() -> WriteCommand,
    ) -> Poll<io::Result<()>> {
        match self.write_tx.poll_reserve(context) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(_)) => Poll::Ready(Err(self.current_error("Data channel is closed"))),
            Poll::Ready(Ok(())) => match self.write_tx.send_item(command()) {
                Ok(()) => Poll::Ready(Ok(())),
                Err(_) => Poll::Ready(Err(self.current_error("Data channel is closed"))),
            },
        }
    }

    fn poll_reply(
        reply: &mut oneshot::Receiver<Result<(), StoredError>>,
        context: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        match Pin::new(reply).poll(context) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(Ok(()))) => Poll::Ready(Ok(())),
            Poll::Ready(Ok(Err(error))) => Poll::Ready(Err(error.into_io())),
            Poll::Ready(Err(_)) => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Data channel writer stopped",
            ))),
        }
    }
}

impl AsyncRead for DataChannelIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        loop {
            if self.read_buffer.has_remaining() {
                let length = self.read_buffer.remaining().min(buffer.remaining());
                buffer.put_slice(&self.read_buffer[..length]);
                self.read_buffer.advance(length);
                return Poll::Ready(Ok(()));
            }

            match self.read_rx.poll_recv(context) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(ReadEvent::Data(bytes))) if bytes.is_empty() => continue,
                Poll::Ready(Some(ReadEvent::Data(bytes))) => self.read_buffer = bytes,
                Poll::Ready(Some(ReadEvent::Error(error))) => {
                    return Poll::Ready(Err(error.into_io()));
                }
                Poll::Ready(Some(ReadEvent::Eof) | None) => return Poll::Ready(Ok(())),
            }
        }
    }
}

impl AsyncWrite for DataChannelIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        if self.shutdown_complete || self.shutdown_rx.is_some() {
            return Poll::Ready(Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Data channel writer is shut down",
            )));
        }
        if let Some(reply) = &mut self.flush_rx {
            match Self::poll_reply(reply, context) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(error)) => {
                    self.flush_rx = None;
                    return Poll::Ready(Err(error));
                }
                Poll::Ready(Ok(())) => self.flush_rx = None,
            }
        }
        if let Some(error) = self.stored_error() {
            return Poll::Ready(Err(error.into_io()));
        }
        if buffer.is_empty() {
            return Poll::Ready(Ok(0));
        }

        let length = buffer.len().min(DATA_CHANNEL_FRAME_SIZE);
        let bytes = BytesMut::from(&buffer[..length]);
        self.poll_send_command(context, || WriteCommand::Data(bytes))
            .map_ok(|()| length)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        if self.shutdown_complete {
            return Poll::Ready(Ok(()));
        }
        if let Some(reply) = &mut self.shutdown_rx {
            let result = Self::poll_reply(reply, context);
            if result.is_ready() {
                self.shutdown_rx = None;
                self.write_tx.close();
                if matches!(&result, Poll::Ready(Ok(()))) {
                    self.shutdown_complete = true;
                }
            }
            return result;
        }
        loop {
            if let Some(reply) = &mut self.flush_rx {
                let result = Self::poll_reply(reply, context);
                if result.is_ready() {
                    self.flush_rx = None;
                }
                return result;
            }

            let (reply_tx, reply_rx) = oneshot::channel();
            match self.poll_send_command(context, || WriteCommand::Flush(reply_tx)) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Ready(Ok(())) => self.flush_rx = Some(reply_rx),
            }
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        if self.shutdown_complete {
            return Poll::Ready(Ok(()));
        }
        if self.shutdown_rx.is_none() {
            match self.as_mut().poll_flush(context) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Ready(Ok(())) => {}
            }

            let (reply_tx, reply_rx) = oneshot::channel();
            match self.poll_send_command(context, || WriteCommand::Shutdown(reply_tx)) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Ready(Ok(())) => self.shutdown_rx = Some(reply_rx),
            }
        }

        let result = Self::poll_reply(self.shutdown_rx.as_mut().expect("shutdown_rx"), context);
        if result.is_ready() {
            self.shutdown_rx = None;
            self.write_tx.close();
            if matches!(&result, Poll::Ready(Ok(()))) {
                self.shutdown_complete = true;
            }
        }
        result
    }
}

impl Drop for DataChannelIo {
    fn drop(&mut self) {
        self.write_tx.close();
    }
}

#[derive(Clone, Debug)]
struct StoredError {
    kind: io::ErrorKind,
    message: Arc<str>,
}

impl StoredError {
    fn new(kind: io::ErrorKind, message: impl Into<Arc<str>>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    fn into_io(self) -> io::Error {
        io::Error::new(self.kind, self.message.to_string())
    }
}

enum ReadEvent {
    Data(Bytes),
    Eof,
    Error(StoredError),
}

enum WriteCommand {
    Data(BytesMut),
    Flush(oneshot::Sender<Result<(), StoredError>>),
    Shutdown(oneshot::Sender<Result<(), StoredError>>),
}

enum ChannelEvent {
    Open,
    Data(BytesMut),
    Close,
    Error,
}

#[async_trait]
trait DataChannelTransport: Send + Sync + 'static {
    async fn send(&self, data: BytesMut) -> io::Result<()>;
    async fn poll(&self) -> Option<ChannelEvent>;
    async fn close(&self) -> io::Result<()>;
}

struct WebRtcTransport(Arc<dyn DataChannel>);

#[async_trait]
impl DataChannelTransport for WebRtcTransport {
    async fn send(&self, data: BytesMut) -> io::Result<()> {
        self.0
            .send(data)
            .await
            .map_err(|error| io::Error::other(error.to_string()))
    }

    async fn poll(&self) -> Option<ChannelEvent> {
        loop {
            let event = self.0.poll().await?;
            match event {
                DataChannelEvent::OnOpen => return Some(ChannelEvent::Open),
                DataChannelEvent::OnMessage(message) => {
                    return Some(ChannelEvent::Data(message.data));
                }
                DataChannelEvent::OnError => return Some(ChannelEvent::Error),
                DataChannelEvent::OnClosing | DataChannelEvent::OnClose => {
                    return Some(ChannelEvent::Close);
                }
                DataChannelEvent::OnBufferedAmountLow | DataChannelEvent::OnBufferedAmountHigh => {}
            }
        }
    }

    async fn close(&self) -> io::Result<()> {
        self.0
            .close()
            .await
            .map_err(|error| io::Error::other(error.to_string()))
    }
}

async fn read_data_channel(
    transport: Arc<dyn DataChannelTransport>,
    read_tx: mpsc::Sender<ReadEvent>,
    opened_tx: oneshot::Sender<Result<(), StoredError>>,
    terminal_error: Arc<Mutex<Option<StoredError>>>,
) {
    let mut opened_tx = Some(opened_tx);
    loop {
        let Some(event) = transport.poll().await else {
            let _ = read_tx.send(ReadEvent::Eof).await;
            if let Some(opened_tx) = opened_tx.take() {
                let _ = opened_tx.send(Err(StoredError::new(
                    io::ErrorKind::BrokenPipe,
                    "Data channel closed before opening",
                )));
            }
            break;
        };
        match event {
            ChannelEvent::Open => {
                if let Some(opened_tx) = opened_tx.take() {
                    let _ = opened_tx.send(Ok(()));
                }
            }
            ChannelEvent::Data(bytes) => {
                if read_tx.send(ReadEvent::Data(bytes.freeze())).await.is_err() {
                    break;
                }
            }
            ChannelEvent::Close => {
                let _ = read_tx.send(ReadEvent::Eof).await;
                if let Some(opened_tx) = opened_tx.take() {
                    let _ = opened_tx.send(Err(StoredError::new(
                        io::ErrorKind::BrokenPipe,
                        "Data channel closed before opening",
                    )));
                }
                break;
            }
            ChannelEvent::Error => {
                let error = StoredError::new(io::ErrorKind::ConnectionReset, "Data channel error");
                set_terminal_error(&terminal_error, error.clone());
                let _ = read_tx.send(ReadEvent::Error(error.clone())).await;
                if let Some(opened_tx) = opened_tx.take() {
                    let _ = opened_tx.send(Err(error));
                }
                break;
            }
        }
    }
}

async fn write_data_channel(
    transport: Arc<dyn DataChannelTransport>,
    mut write_rx: mpsc::Receiver<WriteCommand>,
    read_tx: mpsc::Sender<ReadEvent>,
    terminal_error: Arc<Mutex<Option<StoredError>>>,
) {
    let mut shutdown = false;
    while let Some(command) = write_rx.recv().await {
        match command {
            WriteCommand::Data(data) => {
                if let Err(error) = transport.send(data).await {
                    let error = StoredError::new(error.kind(), error.to_string());
                    set_terminal_error(&terminal_error, error.clone());
                    let _ = read_tx.try_send(ReadEvent::Error(error));
                    break;
                }
            }
            WriteCommand::Flush(reply) => {
                let _ = reply.send(Ok(()));
            }
            WriteCommand::Shutdown(reply) => {
                shutdown = true;
                let result = transport.close().await.map_err(|error| {
                    let error = StoredError::new(error.kind(), error.to_string());
                    set_terminal_error(&terminal_error, error.clone());
                    error
                });
                let _ = reply.send(result);
                break;
            }
        }
    }

    if !shutdown {
        let _ = transport.close().await;
    }
}

fn set_terminal_error(terminal_error: &Mutex<Option<StoredError>>, error: StoredError) {
    let mut terminal_error = terminal_error.lock().expect("terminal_error");
    if terminal_error.is_none() {
        *terminal_error = Some(error);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use tokio::io::AsyncReadExt as _;
    use tokio::io::AsyncWriteExt as _;
    use tokio::sync::Semaphore;
    use tokio::time::Duration;

    use super::*;

    struct MockDataChannel {
        event_rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<ChannelEvent>>,
        sent_tx: mpsc::UnboundedSender<BytesMut>,
        send_permits: Semaphore,
        closes: AtomicUsize,
    }

    impl MockDataChannel {
        fn new(
            send_permits: usize,
        ) -> (
            Arc<Self>,
            mpsc::UnboundedSender<ChannelEvent>,
            mpsc::UnboundedReceiver<BytesMut>,
        ) {
            let (event_tx, event_rx) = mpsc::unbounded_channel();
            let (sent_tx, sent_rx) = mpsc::unbounded_channel();
            (
                Arc::new(Self {
                    event_rx: tokio::sync::Mutex::new(event_rx),
                    sent_tx,
                    send_permits: Semaphore::new(send_permits),
                    closes: AtomicUsize::new(0),
                }),
                event_tx,
                sent_rx,
            )
        }
    }

    #[async_trait]
    impl DataChannelTransport for MockDataChannel {
        async fn send(&self, data: BytesMut) -> io::Result<()> {
            self.send_permits
                .acquire()
                .await
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "send gate closed"))?
                .forget();
            self.sent_tx
                .send(data)
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "sent receiver closed"))
        }

        async fn poll(&self) -> Option<ChannelEvent> {
            self.event_rx.lock().await.recv().await
        }

        async fn close(&self) -> io::Result<()> {
            self.closes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn reads_across_message_and_caller_buffer_boundaries() {
        let (channel, event_tx, _sent_rx) = MockDataChannel::new(0);
        let io = DataChannelIo::from_transport(channel);
        event_tx.send(ChannelEvent::Open).unwrap();
        let mut io = io.wait_open().await.unwrap();
        event_tx
            .send(ChannelEvent::Data(BytesMut::from(&b"ab"[..])))
            .unwrap();
        event_tx
            .send(ChannelEvent::Data(BytesMut::from(&b"cde"[..])))
            .unwrap();

        let mut first = [0; 1];
        io.read_exact(&mut first).await.unwrap();
        assert_eq!(b"a", &first);
        let mut rest = [0; 4];
        io.read_exact(&mut rest).await.unwrap();
        assert_eq!(b"bcde", &rest);
    }

    #[tokio::test]
    async fn fragments_writes_and_flushes_after_send() {
        let (channel, event_tx, mut sent_rx) = MockDataChannel::new(3);
        let io = DataChannelIo::from_transport(channel);
        event_tx.send(ChannelEvent::Open).unwrap();
        let mut io = io.wait_open().await.unwrap();
        let payload = vec![7; DATA_CHANNEL_FRAME_SIZE * 2 + 5];
        io.write_all(&payload).await.unwrap();
        io.flush().await.unwrap();

        assert_eq!(DATA_CHANNEL_FRAME_SIZE, sent_rx.recv().await.unwrap().len());
        assert_eq!(DATA_CHANNEL_FRAME_SIZE, sent_rx.recv().await.unwrap().len());
        assert_eq!(5, sent_rx.recv().await.unwrap().len());
    }

    #[tokio::test]
    async fn bounded_write_queue_applies_backpressure() {
        let (channel, event_tx, mut sent_rx) = MockDataChannel::new(0);
        let io = DataChannelIo::from_transport(channel.clone());
        event_tx.send(ChannelEvent::Open).unwrap();
        let mut io = io.wait_open().await.unwrap();
        let payload = vec![9; DATA_CHANNEL_FRAME_SIZE * (WRITE_QUEUE_CAPACITY + 2)];

        assert!(
            tokio::time::timeout(Duration::from_millis(20), io.write_all(&payload))
                .await
                .is_err()
        );
        channel
            .send_permits
            .add_permits(WRITE_QUEUE_CAPACITY * 3 + 4);
        tokio::time::timeout(Duration::from_secs(1), async {
            io.write_all(&payload).await.unwrap();
            io.flush().await.unwrap();
        })
        .await
        .unwrap();
        assert!(sent_rx.recv().await.is_some());
    }

    #[tokio::test]
    async fn close_becomes_eof() {
        let (channel, event_tx, _sent_rx) = MockDataChannel::new(0);
        let io = DataChannelIo::from_transport(channel);
        event_tx.send(ChannelEvent::Open).unwrap();
        let mut io = io.wait_open().await.unwrap();
        event_tx.send(ChannelEvent::Close).unwrap();

        let mut output = Vec::new();
        assert_eq!(0, io.read_to_end(&mut output).await.unwrap());
    }

    #[tokio::test]
    async fn simultaneous_remote_and_local_close_is_safe() {
        let (channel, event_tx, _sent_rx) = MockDataChannel::new(0);
        let io = DataChannelIo::from_transport(channel.clone());
        event_tx.send(ChannelEvent::Open).unwrap();
        let mut io = io.wait_open().await.unwrap();
        event_tx.send(ChannelEvent::Close).unwrap();
        io.shutdown().await.unwrap();
        io.shutdown().await.unwrap();
        assert_eq!(1, channel.closes.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn data_channel_error_reaches_reader_and_writer() {
        let (channel, event_tx, _sent_rx) = MockDataChannel::new(1);
        let io = DataChannelIo::from_transport(channel);
        event_tx.send(ChannelEvent::Open).unwrap();
        let mut io = io.wait_open().await.unwrap();
        event_tx.send(ChannelEvent::Error).unwrap();

        let mut byte = [0];
        let read_error = io.read_exact(&mut byte).await.unwrap_err();
        assert_eq!(io::ErrorKind::ConnectionReset, read_error.kind());
        let write_error = io.write_all(b"x").await.unwrap_err();
        assert_eq!(io::ErrorKind::ConnectionReset, write_error.kind());
    }

    #[tokio::test]
    async fn send_error_reaches_flush_and_later_writes() {
        let (channel, event_tx, sent_rx) = MockDataChannel::new(1);
        drop(sent_rx);
        let io = DataChannelIo::from_transport(channel);
        event_tx.send(ChannelEvent::Open).unwrap();
        let mut io = io.wait_open().await.unwrap();

        io.write_all(b"x").await.unwrap();
        assert_eq!(
            io::ErrorKind::BrokenPipe,
            io.flush().await.unwrap_err().kind()
        );
        assert_eq!(
            io::ErrorKind::BrokenPipe,
            io.write_all(b"y").await.unwrap_err().kind()
        );
    }
}
