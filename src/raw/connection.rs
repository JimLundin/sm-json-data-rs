//! Connection files under `connection/`.

use crate::raw::*;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;

#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionFile {
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    pub connections: Vec<Connection>,
}

/// A link between nodes in two rooms.
#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Connection {
    pub connection_type: ConnectionType,
    pub direction: ConnectionDirection,
    pub description: Option<String>,
    pub nodes: [ConnectionNode; 2],
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}



#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectionNode {
    pub area: String,
    pub subarea: String,
    #[serde(rename = "roomid")]
    pub room_id: RoomId,
    pub room_name: String,
    #[serde(rename = "nodeid")]
    pub node_id: NodeId,
    pub node_name: String,
    pub position: ConnectionPosition,
    pub note: Option<Note>,
    pub dev_note: Option<Note>,
}

