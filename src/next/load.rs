use crate::codec_orchid::OrchidCodec;
use crate::codec_rskip107::Rskip107Codec;
use crate::node_ref::TrieNode;

pub fn decode_persisted_node(payload: &[u8]) -> Result<TrieNode, String> {
    if OrchidCodec::is_orchid_message(payload) {
        return OrchidCodec::decode_node(payload);
    }

    Rskip107Codec::decode_node(payload)
}
