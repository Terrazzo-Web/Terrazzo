//! Shared WebRTC peer-connection construction and ICE handling.

use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use webrtc::data_channel::DataChannel;
use webrtc::data_channel::RTCDataChannelInit;
use webrtc::peer_connection::PeerConnection as WebRtcPeerConnection;
use webrtc::peer_connection::PeerConnectionBuilder as WebRtcPeerConnectionBuilder;
use webrtc::peer_connection::PeerConnectionEventHandler;
use webrtc::peer_connection::RTCConfigurationBuilder;
use webrtc::peer_connection::RTCIceCandidateInit;
use webrtc::peer_connection::RTCIceGatheringState;
use webrtc::peer_connection::RTCIceServer;
use webrtc::peer_connection::RTCPeerConnectionIceEvent;
use webrtc::peer_connection::RTCSessionDescription;

use super::data_channel_io::DataChannelIo;
use super::protocol::IceCandidate;
use super::protocol::SessionDescription;

/// Label used for Terrazzo's reliable byte-stream data channel.
pub const DATA_CHANNEL_LABEL: &str = "terrazzo-http";

/// Per-channel outstanding-send limit used by the WebRTC implementation.
pub const DATA_CHANNEL_SEND_BUFFER_LIMIT: usize = 16 * 1024 * 1024;

const INCOMING_DATA_CHANNEL_CAPACITY: usize = 1;

/// STUN or TURN configuration for a peer connection.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IceServer {
    /// STUN or TURN URLs.
    pub urls: Vec<String>,

    /// TURN username, when required.
    pub username: String,

    /// TURN credential, when required.
    pub credential: String,
}

impl From<IceServer> for RTCIceServer {
    fn from(ice_server: IceServer) -> Self {
        Self {
            urls: ice_server.urls,
            username: ice_server.username,
            credential: ice_server.credential,
        }
    }
}

/// Local ICE events that callers relay through the signaling server.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalIceEvent {
    /// One locally gathered candidate.
    Candidate(IceCandidate),

    /// The local ICE agent finished gathering candidates.
    EndOfCandidates,

    /// A local candidate could not be converted for signaling.
    Error(String),
}

/// Builds Terrazzo peer connections with shared reliability and flow-control settings.
pub struct PeerConnectionBuilder {
    ice_servers: Vec<IceServer>,
    local_ice_tx: mpsc::Sender<LocalIceEvent>,
    udp_addrs: Vec<String>,
}

impl PeerConnectionBuilder {
    /// Creates a builder that reports gathered ICE candidates to `local_ice_tx`.
    pub fn new(ice_servers: Vec<IceServer>, local_ice_tx: mpsc::Sender<LocalIceEvent>) -> Self {
        Self {
            ice_servers,
            local_ice_tx,
            udp_addrs: vec!["0.0.0.0:0".to_owned()],
        }
    }

    /// Overrides local UDP bind addresses, primarily for hermetic tests.
    #[cfg(test)]
    pub fn with_udp_addrs(mut self, udp_addrs: Vec<String>) -> Self {
        self.udp_addrs = udp_addrs;
        self
    }

    /// Builds and starts the WebRTC peer connection.
    pub async fn build(self) -> Result<PeerConnection, PeerConnectionError> {
        let (incoming_data_channel_tx, incoming_data_channel_rx) =
            mpsc::channel(INCOMING_DATA_CHANNEL_CAPACITY);
        let handler = Arc::new(EventHandler {
            local_ice_tx: self.local_ice_tx,
            incoming_data_channel_tx,
        });
        let configuration = RTCConfigurationBuilder::new()
            .with_ice_servers(
                self.ice_servers
                    .into_iter()
                    .map(RTCIceServer::from)
                    .collect(),
            )
            .build();
        let peer_connection = WebRtcPeerConnectionBuilder::new()
            .with_configuration(configuration)
            .with_handler(handler)
            .with_udp_addrs(self.udp_addrs)
            .with_data_channel_send_buffer_limit(DATA_CHANNEL_SEND_BUFFER_LIMIT)
            .build()
            .await?;

        Ok(PeerConnection {
            peer_connection: Arc::new(peer_connection),
            incoming_data_channel_rx: Arc::new(Mutex::new(incoming_data_channel_rx)),
            remote_ice: Arc::new(Mutex::new(RemoteIceState::default())),
        })
    }
}

/// A WebRTC peer connection with ordered remote-ICE application.
#[derive(Clone)]
pub struct PeerConnection {
    peer_connection: Arc<dyn WebRtcPeerConnection>,
    incoming_data_channel_rx: Arc<Mutex<mpsc::Receiver<Arc<dyn DataChannel>>>>,
    remote_ice: Arc<Mutex<RemoteIceState>>,
}

impl PeerConnection {
    /// Creates the initiator's ordered, fully reliable data channel.
    ///
    /// Call [`PendingDataChannel::wait_open`] after exchanging the offer and
    /// answer. It cannot resolve before WebRTC reports the channel open.
    pub async fn create_reliable_data_channel(
        &self,
    ) -> Result<PendingDataChannel, PeerConnectionError> {
        let data_channel = self
            .peer_connection
            .create_data_channel(
                DATA_CHANNEL_LABEL,
                Some(RTCDataChannelInit {
                    ordered: true,
                    max_packet_life_time: None,
                    max_retransmits: None,
                    ..Default::default()
                }),
            )
            .await?;
        validate_reliable(&data_channel).await?;
        Ok(PendingDataChannel(DataChannelIo::new(data_channel)))
    }

    /// Waits for the remote peer to create a data channel and validates that it
    /// is ordered and fully reliable before returning it.
    pub async fn accept_reliable_data_channel(
        &self,
    ) -> Result<PendingDataChannel, PeerConnectionError> {
        let data_channel = self
            .incoming_data_channel_rx
            .lock()
            .await
            .recv()
            .await
            .ok_or(PeerConnectionError::PeerClosed)?;
        validate_reliable(&data_channel).await?;
        Ok(PendingDataChannel(DataChannelIo::new(data_channel)))
    }

    /// Creates and installs a local SDP offer.
    pub async fn create_offer(&self) -> Result<SessionDescription, PeerConnectionError> {
        let offer = self.peer_connection.create_offer(None).await?;
        self.peer_connection
            .set_local_description(offer.clone())
            .await?;
        Ok(SessionDescription::Offer(offer.sdp))
    }

    /// Creates and installs a local SDP answer.
    pub async fn create_answer(&self) -> Result<SessionDescription, PeerConnectionError> {
        let answer = self.peer_connection.create_answer(None).await?;
        self.peer_connection
            .set_local_description(answer.clone())
            .await?;
        Ok(SessionDescription::Answer(answer.sdp))
    }

    /// Installs the remote SDP and then applies every queued remote ICE event in
    /// the order it was received.
    pub async fn set_remote_description(
        &self,
        description: SessionDescription,
    ) -> Result<(), PeerConnectionError> {
        let mut remote_ice = self.remote_ice.lock().await;
        let description = match description {
            SessionDescription::Offer(sdp) => RTCSessionDescription::offer(sdp)?,
            SessionDescription::Answer(sdp) => RTCSessionDescription::answer(sdp)?,
        };
        self.peer_connection
            .set_remote_description(description)
            .await?;
        remote_ice.description_set = true;
        while let Some(event) = remote_ice.pending.pop_front() {
            apply_remote_ice(&self.peer_connection, event).await?;
        }
        Ok(())
    }

    /// Queues or immediately applies a remote ICE candidate without reordering it.
    pub async fn add_remote_candidate(
        &self,
        candidate: IceCandidate,
    ) -> Result<(), PeerConnectionError> {
        self.apply_remote_ice(RemoteIceEvent::Candidate(candidate))
            .await
    }

    /// Records the remote peer's explicit end-of-candidates marker.
    pub async fn end_remote_candidates(&self) -> Result<(), PeerConnectionError> {
        self.apply_remote_ice(RemoteIceEvent::EndOfCandidates).await
    }

    /// Closes the WebRTC connection and all of its data channels.
    pub async fn close(&self) -> Result<(), PeerConnectionError> {
        self.peer_connection.close().await?;
        Ok(())
    }

    async fn apply_remote_ice(&self, event: RemoteIceEvent) -> Result<(), PeerConnectionError> {
        let mut remote_ice = self.remote_ice.lock().await;
        if remote_ice.end_of_candidates {
            return Err(PeerConnectionError::CandidateAfterEnd);
        }
        if matches!(event, RemoteIceEvent::EndOfCandidates) {
            remote_ice.end_of_candidates = true;
        }
        if remote_ice.description_set {
            apply_remote_ice(&self.peer_connection, event).await
        } else {
            remote_ice.pending.push_back(event);
            Ok(())
        }
    }
}

/// A reliable channel that may still be negotiating.
pub struct PendingDataChannel(DataChannelIo);

impl PendingDataChannel {
    /// Waits until the channel is open and ready for byte-stream I/O.
    pub async fn wait_open(self) -> Result<DataChannelIo, PeerConnectionError> {
        Ok(self.0.wait_open().await?)
    }
}

/// Tracks remote ICE signaling state and queues events until the remote SDP is set.
#[derive(Default)]
struct RemoteIceState {
    description_set: bool,
    end_of_candidates: bool,
    pending: VecDeque<RemoteIceEvent>,
}

enum RemoteIceEvent {
    Candidate(IceCandidate),
    EndOfCandidates,
}

async fn apply_remote_ice(
    peer_connection: &Arc<dyn WebRtcPeerConnection>,
    event: RemoteIceEvent,
) -> Result<(), PeerConnectionError> {
    if let RemoteIceEvent::Candidate(candidate) = event {
        peer_connection
            .add_ice_candidate(RTCIceCandidateInit {
                candidate: candidate.candidate,
                sdp_mid: candidate.sdp_mid,
                sdp_mline_index: candidate.sdp_mline_index,
                username_fragment: candidate.username_fragment,
                url: candidate.url,
            })
            .await?;
    }
    Ok(())
}

async fn validate_reliable(data_channel: &Arc<dyn DataChannel>) -> Result<(), PeerConnectionError> {
    let ordered = data_channel.ordered().await?;
    let max_packet_life_time = data_channel.max_packet_life_time().await?;
    let max_retransmits = data_channel.max_retransmits().await?;
    if !ordered || max_packet_life_time.is_some() || max_retransmits.is_some() {
        return Err(PeerConnectionError::UnreliableDataChannel {
            ordered,
            max_packet_life_time,
            max_retransmits,
        });
    }
    Ok(())
}

struct EventHandler {
    local_ice_tx: mpsc::Sender<LocalIceEvent>,
    incoming_data_channel_tx: mpsc::Sender<Arc<dyn DataChannel>>,
}

#[async_trait]
impl PeerConnectionEventHandler for EventHandler {
    async fn on_ice_candidate(&self, event: RTCPeerConnectionIceEvent) {
        let message = match event.candidate.to_json() {
            Ok(mut candidate) => {
                candidate.url = (!event.url.is_empty()).then_some(event.url);
                LocalIceEvent::Candidate(IceCandidate {
                    candidate: candidate.candidate,
                    sdp_mid: candidate.sdp_mid,
                    sdp_mline_index: candidate.sdp_mline_index,
                    username_fragment: candidate.username_fragment,
                    url: candidate.url,
                })
            }
            Err(error) => LocalIceEvent::Error(error.to_string()),
        };
        let _ = self.local_ice_tx.send(message).await;
    }

    async fn on_ice_gathering_state_change(&self, state: RTCIceGatheringState) {
        if state == RTCIceGatheringState::Complete {
            let _ = self.local_ice_tx.send(LocalIceEvent::EndOfCandidates).await;
        }
    }

    async fn on_data_channel(&self, data_channel: Arc<dyn DataChannel>) {
        let _ = self.incoming_data_channel_tx.send(data_channel).await;
    }
}

/// Failure while constructing or driving a shared P2P connection.
#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum PeerConnectionError {
    /// The underlying WebRTC implementation failed.
    #[error("[{n}] WebRTC operation failed: {0}", n = self.name())]
    WebRtc(#[from] webrtc::error::Error),

    /// The peer closed before providing its data channel.
    #[error("[{n}] Peer closed before providing a data channel", n = self.name())]
    PeerClosed,

    /// The peer proposed a lossy or unordered channel.
    #[error(
        "[{n}] Data channel is not fully reliable: ordered={ordered}, max_packet_life_time={max_packet_life_time:?}, max_retransmits={max_retransmits:?}",
        n = self.name()
    )]
    UnreliableDataChannel {
        /// Whether delivery is ordered.
        ordered: bool,

        /// Configured packet lifetime.
        max_packet_life_time: Option<u16>,

        /// Configured retransmission limit.
        max_retransmits: Option<u16>,
    },

    /// A candidate arrived after the explicit end marker.
    #[error("[{n}] Received an ICE candidate after end-of-candidates", n = self.name())]
    CandidateAfterEnd,

    /// The data-channel stream failed.
    #[error("[{n}] Data channel I/O failed: {0}", n = self.name())]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncReadExt as _;
    use tokio::io::AsyncWriteExt as _;
    use tokio::time::Duration;

    use super::*;

    #[tokio::test]
    async fn opens_reliable_channel_with_candidates_arriving_before_sdp() {
        let (offer_ice_tx, mut offer_ice_rx) = mpsc::channel(16);
        let (answer_ice_tx, answer_ice_rx) = mpsc::channel(16);
        let offerer = PeerConnectionBuilder::new(vec![], offer_ice_tx)
            .with_udp_addrs(vec!["0.0.0.0:0".to_owned()])
            .build()
            .await
            .unwrap();
        let answerer = PeerConnectionBuilder::new(vec![], answer_ice_tx)
            .with_udp_addrs(vec!["0.0.0.0:0".to_owned()])
            .build()
            .await
            .unwrap();

        let answer_ice = relay_ice(answer_ice_rx, offerer.clone());
        let offer_pending = offerer.create_reliable_data_channel().await.unwrap();
        let offer = offerer.create_offer().await.unwrap();
        let first_candidate = tokio::time::timeout(Duration::from_secs(5), async {
            match offer_ice_rx.recv().await.unwrap() {
                LocalIceEvent::Candidate(candidate) => candidate,
                LocalIceEvent::EndOfCandidates => panic!("ICE ended without a host candidate"),
                LocalIceEvent::Error(error) => panic!("ICE candidate error: {error}"),
            }
        })
        .await
        .unwrap();
        answerer
            .add_remote_candidate(first_candidate)
            .await
            .unwrap();
        let offer_ice = relay_ice(offer_ice_rx, answerer.clone());
        answerer.set_remote_description(offer).await.unwrap();
        let answer = answerer.create_answer().await.unwrap();
        offerer.set_remote_description(answer).await.unwrap();
        let answer_pending = answerer.accept_reliable_data_channel().await.unwrap();

        let (offer_io, answer_io) = tokio::time::timeout(Duration::from_secs(10), async {
            tokio::try_join!(offer_pending.wait_open(), answer_pending.wait_open())
        })
        .await
        .unwrap()
        .unwrap();
        let (mut offer_io, mut answer_io) = (offer_io, answer_io);
        offer_io.write_all(b"hello over webrtc").await.unwrap();
        offer_io.flush().await.unwrap();
        let mut received = [0; 17];
        answer_io.read_exact(&mut received).await.unwrap();
        assert_eq!(b"hello over webrtc", &received);

        offerer.close().await.unwrap();
        answerer.close().await.unwrap();
        offer_ice.abort();
        answer_ice.abort();
    }

    fn relay_ice(
        mut ice_rx: mpsc::Receiver<LocalIceEvent>,
        remote: PeerConnection,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(event) = ice_rx.recv().await {
                match event {
                    LocalIceEvent::Candidate(candidate) => {
                        remote.add_remote_candidate(candidate).await.unwrap();
                    }
                    LocalIceEvent::EndOfCandidates => {
                        remote.end_remote_candidates().await.unwrap();
                        break;
                    }
                    LocalIceEvent::Error(error) => panic!("ICE candidate error: {error}"),
                }
            }
        })
    }
}
