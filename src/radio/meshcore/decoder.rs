use super::header::MeshcoreHeader;
use heapless::Vec;

pub enum DecoderError { 
    Empty,
    UnknownPayloadType(u8),
}

pub trait MeshcoreDecoder {
    fn decode_meshcore_packet(&self, packet: &[u8]) -> Option<MeshcoreHeader>;
}


pub fn decode_meshcore_packet(packet: Vec<u8, 255>) -> Option<MeshcoreHeader> {
    let header_raw = packet.first().copied().ok_or(DecoderError::Empty).ok()?;
    MeshcoreHeader::parse(header_raw).ok()
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::radio::meshcore::header::*;

    #[test]
    fn test_decode_packet() {
        let packet = Vec::<u8, 255>::from_slice(&[0x0E, 0x01, 0x02, 0x03]).unwrap();
        let header = decode_meshcore_packet(packet).unwrap();
        assert_eq!(header.route_type(), MeshcoreRouteType::Direct);
        assert_eq!(header.payload_type(), MeshcorePayloadType::Ack); 
        assert_eq!(header.payload_version(), MeshcorePayloadVersion::Ver1);
    }
}