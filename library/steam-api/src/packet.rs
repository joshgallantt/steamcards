//! One message on the wire, as a CM server's WebSocket carries it: each
//! binary frame is one message, with no length prefix (that's for Steam's TCP
//! connections).
//!
//! ```text
//! u32 LE   EMsg, with the top bit set for a protobuf message
//! u32 LE   header length
//! …        CMsgProtoBufHeader
//! …        the body
//! ```
//!
//! A few old messages aren't protobufs. steamcards reads none of them, so
//! they're skipped.

use std::io::Read;

use anyhow::{anyhow, bail};
use flate2::read::GzDecoder;
use prost::Message;

use crate::proto::{self, Header, Multi};

const PROTOBUF: u32 = 0x8000_0000;

/// A protobuf message: its number, header and undecoded body.
#[derive(Debug, Clone)]
pub(crate) struct Packet {
    pub(crate) emsg: u32,
    pub(crate) header: Header,
    pub(crate) body: Vec<u8>,
}

impl Packet {
    /// The frame for `body`, sent as `emsg` with `header`.
    pub(crate) fn encode(emsg: u32, header: &Header, body: &impl Message) -> Vec<u8> {
        Self::encode_bytes(emsg, header, &body.encode_to_vec())
    }

    /// The frame for a body that's already encoded.
    pub(crate) fn encode_bytes(emsg: u32, header: &Header, body: &[u8]) -> Vec<u8> {
        let header = header.encode_to_vec();
        let mut frame = Vec::with_capacity(8 + header.len() + body.len());
        frame.extend((emsg | PROTOBUF).to_le_bytes());
        frame.extend((header.len() as u32).to_le_bytes());
        frame.extend(header);
        frame.extend(body);
        frame
    }

    /// Reads a frame; `None` when it's one of the old messages that aren't
    /// protobufs.
    pub(crate) fn decode(frame: &[u8]) -> anyhow::Result<Option<Packet>> {
        let (Some(raw), Some(len)) = (le_u32(frame, 0), le_u32(frame, 4)) else {
            bail!("a message too short to read ({} bytes)", frame.len());
        };
        if raw & PROTOBUF == 0 {
            return Ok(None);
        }
        let start = 8;
        let end = start + len as usize;
        let header = frame
            .get(start..end)
            .ok_or_else(|| anyhow!("a message header longer than its message"))?;
        Ok(Some(Packet {
            emsg: raw & !PROTOBUF,
            header: proto::decode("message header", header)?,
            body: frame[end..].to_vec(),
        }))
    }
}

/// The messages inside a `Multi`, which may be gzipped, in order. A `Multi`
/// can hold another; the caller unpacks those in turn.
pub(crate) fn unpack_multi(body: &[u8]) -> anyhow::Result<Vec<Vec<u8>>> {
    let multi: Multi = proto::decode("Multi", body)?;
    let packed = multi.message_body.unwrap_or_default();
    let data = match multi.size_unzipped {
        Some(size) if size > 0 => {
            let mut out = Vec::with_capacity(size as usize);
            GzDecoder::new(packed.as_slice())
                .read_to_end(&mut out)
                .map_err(|e| anyhow!("a Multi that doesn't unzip: {e}"))?;
            out
        }
        _ => packed,
    };
    let mut frames = Vec::new();
    let mut at = 0;
    while at < data.len() {
        let len = le_u32(&data, at).ok_or_else(|| anyhow!("a Multi cut short"))? as usize;
        let frame = data
            .get(at + 4..at + 4 + len)
            .ok_or_else(|| anyhow!("a Multi cut short"))?;
        frames.push(frame.to_vec());
        at += 4 + len;
    }
    Ok(frames)
}

fn le_u32(bytes: &[u8], at: usize) -> Option<u32> {
    let b = bytes.get(at..at + 4)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Packs frames into a `Multi`, gzipped, as a CM server does. For the fake
/// server in tests.
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn pack_multi(frames: &[Vec<u8>]) -> Multi {
    use std::io::Write;

    let mut data = Vec::new();
    for f in frames {
        data.extend((f.len() as u32).to_le_bytes());
        data.extend(f);
    }
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&data).expect("gzip into memory");
    Multi {
        size_unzipped: Some(data.len() as u32),
        message_body: Some(gz.finish().expect("gzip into memory")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{ClientHello, ClientLoggedOff, emsg};

    #[test]
    fn a_message_reads_back_as_it_was_sent() {
        let header = Header {
            steamid: Some(76_561_197_960_287_930),
            client_sessionid: Some(42),
            jobid_source: Some(7),
            target_job_name: Some("Authentication.PollAuthSessionStatus#1".into()),
            ..Default::default()
        };
        let body = ClientHello {
            protocol_version: Some(65580),
        };
        let frame = Packet::encode(emsg::CLIENT_HELLO, &header, &body);
        assert_eq!(frame[3] & 0x80, 0x80, "flagged as a protobuf");

        let packet = Packet::decode(&frame).unwrap().unwrap();
        assert_eq!(packet.emsg, emsg::CLIENT_HELLO);
        assert_eq!(packet.header, header);
        assert_eq!(ClientHello::decode(&packet.body[..]).unwrap(), body);
    }

    #[test]
    fn old_messages_are_skipped_and_broken_ones_said_so() {
        let mut old = 751u32.to_le_bytes().to_vec();
        old.extend([0; 32]);
        assert!(Packet::decode(&old).unwrap().is_none());
        assert!(Packet::decode(&[1, 2, 3]).is_err());

        let mut long_header = (751 | PROTOBUF).to_le_bytes().to_vec();
        long_header.extend(100u32.to_le_bytes());
        assert!(Packet::decode(&long_header).is_err());
    }

    #[test]
    fn a_multi_unpacks_into_its_messages_in_order() {
        let frames: Vec<Vec<u8>> = [1, 6]
            .map(|eresult| {
                Packet::encode(
                    emsg::CLIENT_LOGGED_OFF,
                    &Header::default(),
                    &ClientLoggedOff {
                        eresult: Some(eresult),
                    },
                )
            })
            .to_vec();
        let multi = pack_multi(&frames);
        assert_eq!(unpack_multi(&multi.encode_to_vec()).unwrap(), frames);

        let plain = Multi {
            size_unzipped: None,
            message_body: Some([4u8, 0, 0, 0, 9, 9, 9, 9].to_vec()),
        };
        assert_eq!(
            unpack_multi(&plain.encode_to_vec()).unwrap(),
            [vec![9, 9, 9, 9]]
        );

        let cut = Multi {
            size_unzipped: None,
            message_body: Some([8u8, 0, 0, 0, 9].to_vec()),
        };
        assert!(unpack_multi(&cut.encode_to_vec()).is_err());
    }
}
