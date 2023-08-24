use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub version: u8,
    pub messages: Vec<BroadcastFeedMessage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BroadcastFeedMessage {
    pub sequence_number: u64,
    pub message: MessageWithMetadata,
    pub signature: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageWithMetadata {
    pub message: L1IncomingMessageHeader,
    pub delayed_messages_read: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct L1IncomingMessageHeader {
    pub header: Header,
    #[serde(rename = "l2Msg")]
    pub l2msg: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    pub kind: u8,
    pub sender: String,
    pub block_number: u64,
    pub timestamp: u64,
    pub request_id: Value,
    pub base_fee_l1: Value,
}
