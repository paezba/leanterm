use rudder_message::Track;
use virtual_fs::VirtualFS;

use super::*;


impl RudderBatchMessage {
    fn unwrap_track(&self) -> &Track {
        match self {
            RudderBatchMessage::Track(track) => track,
            _ => panic!("Expected a track event"),
        }
    }
}
