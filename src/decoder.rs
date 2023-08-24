use crate::types::L1IncomingMessageHeader;
use base64::{engine::general_purpose, Engine as _};
use ethers::types::{Transaction, H160};
use ethers::utils::rlp::{self, DecoderError, Rlp};

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
    pub fn decode(&self) -> Option<DecodedMsg> {
        if self.l2msg.len() > MAX_L2_MESSAGE_SIZE {
            return None;
        }

        let l2_bytes = general_purpose::STANDARD.decode(&self.l2msg).unwrap();

        match L2MessageKind::from(l2_bytes[0]) {
            L2MessageKind::Batch => {
                let vec_tx = Self::parse_batch_transactions(&l2_bytes[1..]);
                Some(DecodedMsg::DecodedBatch(vec_tx))
            }
            L2MessageKind::SignedTx => {
                let tx = ethers::utils::rlp::decode(&l2_bytes[1..]).unwrap();
                Some(DecodedMsg::DecodedSignedTx(tx))
            }
            _ => None,
        }
    }

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
}
