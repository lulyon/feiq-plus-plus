pub mod udp;
pub mod tcp;
pub mod manager;
pub mod crypto;
pub mod relay;

use crate::protocol::types::Post;

/// Events from the network layer to the engine
/// (shared across UDP manager, relay client, and future transports)
#[derive(Debug)]
pub enum NetworkEvent {
    /// Raw parsed Post (for content processing)
    Message(Post),
    /// A new user came online (BR_ENTRY handled)
    FellowOnline(Post),
    /// A user went offline (BR_EXIT handled)
    FellowOffline(Post),
    /// Self online notification response (ANSENTRY handled)
    FellowAnsEntry(Post),
    /// Error in network processing
    Error(String),
}
