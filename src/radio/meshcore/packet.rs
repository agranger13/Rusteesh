use heapless::Vec;
use core::fmt;
use super::header::{HeaderError, MeshcoreHeader, MeshcoreRouteType};

#[derive(Debug, PartialEq)]
pub enum DecoderError {
    /// Paquet vide : pas même un octet de header.
    Empty,
    /// Le header est invalide (payload type inconnu, ...).
    Header(HeaderError),
    /// Le paquet est plus court que ce qu'annoncent ses propres champs.
    Truncated,
}

// Permet à `?` de convertir automatiquement une HeaderError en DecoderError.
impl From<HeaderError> for DecoderError {
    fn from(e: HeaderError) -> Self {
        DecoderError::Header(e)
    }
}

impl fmt::Display for DecoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecoderError::Empty => f.write_str("paquet vide"),
            DecoderError::Header(e) => write!(f, "header invalide: {:?}", e),
            DecoderError::Truncated => f.write_str("paquet tronqué"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Packet {
    header: MeshcoreHeader,
    path_length: u8,
    path: Vec<u8, 255>,
    payload: Vec<u8, 255>,
    raw: Vec<u8, 255>,
}

impl Packet {
    pub fn header(&self) -> &MeshcoreHeader {
        &self.header
    }

    pub fn path_length(&self) -> u8 {
        self.path_length
    }

    pub fn path(&self) -> &[u8] {
        &self.path
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
}

impl TryFrom<&[u8]> for Packet {
    type Error = DecoderError;

    fn try_from(packet: &[u8]) -> Result<Self, Self::Error> {
        let header_raw = *packet.first().ok_or(DecoderError::Empty)?;
        let header = MeshcoreHeader::parse(header_raw)?;

        // Les routes Transport* insèrent 4 octets de transport codes avant path_length.
        let path_len_idx = match header.route_type() {
            MeshcoreRouteType::Flood | MeshcoreRouteType::Direct => 1,
            MeshcoreRouteType::TransportFlood | MeshcoreRouteType::TransportDirect => 5,
        };

        // path_length = (hash_size - 1) << 6 | hash_count
        let path_length = *packet.get(path_len_idx).ok_or(DecoderError::Truncated)?;
        let hash_size = ((path_length >> 6) + 1) as usize;
        let hash_count = (path_length & 0b0011_1111) as usize;

        let path_start = path_len_idx + 1;
        let path_end = path_start + hash_size * hash_count;
        let path = packet.get(path_start..path_end).ok_or(DecoderError::Truncated)?;
        let payload = packet.get(path_end..).ok_or(DecoderError::Truncated)?;

        Ok(Packet {
            header,
            path_length,
            // Ne peut pas échouer : un morceau d'un Vec<u8, 255> tient dans 255 octets.
            path: Vec::from_slice(path).map_err(|_| DecoderError::Truncated)?,
            payload: Vec::from_slice(payload).map_err(|_| DecoderError::Truncated)?,
            raw: Vec::from_slice(packet).map_err(|_| DecoderError::Truncated)?,
        })
    }
}


impl fmt::Display for Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?}/{:?} path=[",
            self.header.route_type(),
            self.header.payload_type(),
        )?;
        for (i, b) in self.path.iter().enumerate() {
            if i > 0 { f.write_str(" ")?; }
            write!(f, "{:02X}", b)?;
        }
        write!(f, "] payload={}o", self.payload.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::radio::meshcore::header::*;

    // Le paramètre typé `&[u8]` laisse un `&[u8; N]` se convertir tout seul en slice,
    // ce que `Packet::try_from(&[...])` ne ferait pas (inférence générique).
    fn decode(bytes: &[u8]) -> Result<Packet, DecoderError> {
        Packet::try_from(bytes)
    }

    #[test]
    fn test_decode_packet() {
        // header 0x0E = Direct + Ack + Ver1, path_length 0x01 = 1 hash d'1 octet
        let packet = decode(&[0x0E, 0x01, 0x02, 0x03]).unwrap();
        assert_eq!(packet.header().route_type(), MeshcoreRouteType::Direct);
        assert_eq!(packet.header().payload_type(), MeshcorePayloadType::Ack);
        assert_eq!(packet.header().payload_version(), MeshcorePayloadVersion::Ver1);
        assert_eq!(packet.path(), &[0x02]);
        assert_eq!(packet.payload(), &[0x03]);
    }

    #[test]
    fn test_decode_multi_byte_hashes() {
        // path_length 0x42 = 2 hash de 2 octets → 4 octets de path
        let packet = decode(&[0x0E, 0x42, 1, 2, 3, 4, 0xAA]).unwrap();
        assert_eq!(packet.path(), &[1, 2, 3, 4]);
        assert_eq!(packet.payload(), &[0xAA]);
    }

    #[test]
    fn test_decode_empty() {
        assert_eq!(decode(&[]).unwrap_err(), DecoderError::Empty);
    }

    #[test]
    fn test_decode_truncated_path() {
        // annonce 3 hash d'1 octet mais n'en contient que 2
        let err = decode(&[0x0E, 0x03, 0x01, 0x02]).unwrap_err();
        assert_eq!(err, DecoderError::Truncated);
    }

    #[test]
    fn test_decode_bad_header() {
        let err = decode(&[0x0E << 2, 0x00]).unwrap_err();
        assert_eq!(err, DecoderError::Header(HeaderError::UnknownPayloadType(0x0E)));
    }

    #[test]
    fn test_display() {
        assert_eq!(decode(&[0x0E, 0x01, 0x02, 0x03]).unwrap().to_string(), "Direct/Ack path=[02] payload=1o");
    }
}
