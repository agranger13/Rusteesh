
#[derive(Debug)]
pub enum HeaderError { 
    Empty,
    UnknownPayloadType(u8),
}

#[repr(u8)] // garantit que la variante tient sur un u8, et permet `as u8`
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MeshcoreRouteType {
    TransportFlood = 0x00,
    Flood = 0x01,
    Direct = 0x02,
    TransportDirect = 0x03,
}

impl From<u8> for MeshcoreRouteType {
    fn from(b: u8) -> Self {
        match b & 0b11 { // on masque ici : il ne reste que 0..=3, le match est exhaustif
            0x00 => Self::TransportFlood,
            0x01 => Self::Flood,
            0x02 => Self::Direct,
            _    => Self::TransportDirect, // seule valeur restante : 3
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MeshcorePayloadType {
    Req = 0x00,
    Response = 0x01,
    TxtMsg = 0x02,
    Ack = 0x03,
    Advert = 0x04,
    GrpTxt = 0x05,
    GrpData = 0x06,
    AnonReq = 0x07,
    Path = 0x08,
    Trace = 0x09,
    Multipart = 0x0A,
    Control = 0x0B,
    RawCustom = 0x0F,
}

impl TryFrom<u8> for MeshcorePayloadType {
    type Error = HeaderError;

    fn try_from(b: u8) -> Result<Self, Self::Error> {
        Ok(match b {
            0x00 => Self::Req,
            0x01 => Self::Response,
            0x02 => Self::TxtMsg,
            0x03 => Self::Ack,
            0x04 => Self::Advert,
            0x05 => Self::GrpTxt,
            0x06 => Self::GrpData,
            0x07 => Self::AnonReq,
            0x08 => Self::Path,
            0x09 => Self::Trace,
            0x0A => Self::Multipart,
            0x0B => Self::Control,
            0x0F => Self::RawCustom,
            other => return Err(HeaderError::UnknownPayloadType(other)),
        })
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MeshcorePayloadVersion {
    Ver1  = 0x00,
    Ver2  = 0x01,
    Ver3  = 0x02,
    Ver4  = 0x03,
}

impl From<u8> for MeshcorePayloadVersion {
    fn from(b: u8) -> Self {
        match b & 0b11 { // on masque ici : il ne reste que 0..=3, le match est exhaustif
            0x00 => Self::Ver1,
            0x01 => Self::Ver2,
            0x02 => Self::Ver3,
            _    => Self::Ver4, // seule valeur restante : 3
        }
    }
}

#[derive(Debug)]
pub struct MeshcoreHeader {
    route_type: MeshcoreRouteType,
    payload_type: MeshcorePayloadType,
    payload_version: MeshcorePayloadVersion,
}

impl MeshcoreHeader {
    pub fn parse(b: u8) -> Result<Self, HeaderError> {
        Ok(Self {
            route_type:      MeshcoreRouteType::from(b),                      // bits 0-1
            payload_type:    MeshcorePayloadType::try_from((b >> 2) & 0x0F)?, // bits 2-5
            payload_version: MeshcorePayloadVersion::from(b >> 6),            // bits 6-7
        })
    }

    pub fn to_byte(&self) -> u8 {
        (self.route_type as u8)
            | ((self.payload_type as u8) << 2)
            | ((self.payload_version as u8) << 6)
    }


    pub fn route_type(&self) -> MeshcoreRouteType {
        self.route_type
    }

    pub fn payload_type(&self) -> MeshcorePayloadType {
        self.payload_type
    }

    pub fn payload_version(&self) -> MeshcorePayloadVersion {
        self.payload_version
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_parsing() {
        let header = MeshcoreHeader::parse(0x00).unwrap();
        assert_eq!(header.route_type, MeshcoreRouteType::TransportFlood);
        assert_eq!(header.payload_type, MeshcorePayloadType::Req);
        assert_eq!(header.payload_version, MeshcorePayloadVersion::Ver1);
    }

    #[test]
    fn test_header_parsing_error() {
        let err = MeshcoreHeader::parse(0x0E << 2).unwrap_err(); // panique si c'est Ok
        assert!(matches!(err, HeaderError::UnknownPayloadType(0x0E)));
    }
}