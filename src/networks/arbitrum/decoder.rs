use crate::networks::arbitrum::types::L1IncomingMessageHeader;
use base64::{engine::general_purpose, Engine as _};
use ethers::{
    types::{Transaction, H160},
    utils::rlp::{self, DecoderError, Rlp},
};

const MAX_L2_MESSAGE_SIZE: usize = 256 * 1024;

enum L2MessageKind {
    UnsignedUserTx,
    ContractTx,
    NonMutatingCall,
    Batch,
    SignedTx,
    // 5 is reserved
    Heartbeat, // deprecated
    SignedCompressedTx,
}

impl From<u8> for L2MessageKind {
    fn from(v: u8) -> Self {
        match v {
            0 => L2MessageKind::UnsignedUserTx,
            1 => L2MessageKind::ContractTx,
            2 => L2MessageKind::NonMutatingCall,
            3 => L2MessageKind::Batch,
            4 => L2MessageKind::SignedTx,
            6 => L2MessageKind::Heartbeat,
            7 => L2MessageKind::SignedCompressedTx,
            _ => panic!("L2MessageKind {} is not supported", v),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Create,
    Call(H160),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedMsg {
    DecodedBatch(Vec<Transaction>),
    DecodedSignedTx(Transaction),
}

impl rlp::Decodable for Action {
    /// Decodes an RLP-encoded `Action` object and returns a `Result` containing the decoded object or a `DecoderError`.
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        if rlp.is_empty() {
            if rlp.is_data() {
                Ok(Action::Create)
            } else {
                Err(DecoderError::RlpExpectedToBeData)
            }
        } else {
            Ok(Action::Call(rlp.as_val()?))
        }
    }
}

impl L1IncomingMessageHeader {
    /// Decodes the L2 message and returns a `DecodedMsg` if successful.
    /// Returns `None` if the L2 message length exceeds `MAX_L2_MESSAGE_SIZE`.
    pub fn decode(&self) -> Option<DecodedMsg> {
        if self.l2msg.len() > MAX_L2_MESSAGE_SIZE {
            return None;
        }

        let l2_bytes = general_purpose::STANDARD
            .decode(&self.l2msg)
            .unwrap_or_default();

        get_decoded_msg(l2_bytes)
    }
}

/// Decodes an L2 message from the given bytes and returns the decoded message.
///
/// # Arguments
///
/// * `l2_bytes` - A vector of bytes representing the L2 message to be decoded.
///
/// # Returns
///
/// An `Option` containing the decoded message if the decoding was successful, or `None` otherwise.
///
/// # Example
///
/// ```
/// use sequencer_feed_reader::networks::arbitrum::decoder::get_decoded_msg;
/// use sequencer_feed_reader::networks::arbitrum::DecodedMsg;
///
/// let l2_bytes = vec![0x01, 0x02, 0x03];
/// let decoded_msg = get_decoded_msg(l2_bytes);
///
/// match decoded_msg {
///     Some(DecodedMsg::DecodedBatch(vec_tx)) => {
///         // Do something with the batch of transactions
///     },
///     Some(DecodedMsg::DecodedSignedTx(tx)) => {
///         // Do something with the signed transaction
///     },
///     None => {
///         // Handle the case where decoding failed
///     }
/// }
/// ```
fn get_decoded_msg(l2_bytes: Vec<u8>) -> Option<DecodedMsg> {
    match L2MessageKind::from(l2_bytes[0]) {
        L2MessageKind::Batch => {
            let vec_tx = parse_batch_transactions(&l2_bytes[1..]);
            Some(DecodedMsg::DecodedBatch(vec_tx))
        }
        L2MessageKind::SignedTx => {
            let tx = ethers::utils::rlp::decode(&l2_bytes[1..]).unwrap();
            Some(DecodedMsg::DecodedSignedTx(tx))
        }
        _ => None,
    }
}

/// Parses a batch of transactions from a byte slice.
///
/// # Arguments
///
/// * `data` - A byte slice containing the batch of transactions.
///
/// # Example
///
/// ```
/// # use crate::networks::arbitrum::Transaction;
/// let data = vec![0x00, 0x00, 0x00, 0x0A, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
/// let transactions = parse_batch_transactions(&data);
/// assert_eq!(transactions.len(), 1);
/// ```
fn parse_batch_transactions(data: &[u8]) -> Vec<Transaction> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < data.len() - 8 {
        let size_bytes = &data[i..i + 8];
        let size = u64::from_be_bytes(size_bytes.try_into().unwrap()) as usize;
        let msg = &data[i + 8..i + 8 + size];
        result.push(ethers::utils::rlp::decode(&msg[1..]).unwrap());
        i += 8 + size;
    }

    result
}
