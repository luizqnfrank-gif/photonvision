use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PacketError {
    #[error("packet buffer underflow: requested {requested} bytes, remaining {remaining} bytes")]
    Underflow { requested: usize, remaining: usize },
    #[error("variable-length list exceeds u8 max (255): got {0}")]
    ListTooLong(usize),
    #[error("invalid boolean byte in packet: {0}")]
    InvalidBoolean(u8),
}

pub type PacketResult<T> = Result<T, PacketError>;

/// Trait matching `org.photonvision.common.dataflow.structures.PacketSerde<T>`.
pub trait PacketSerde: Sized {
    const INTERFACE_UUID: &'static str;
    const SCHEMA: &'static str;
    const TYPE_NAME: &'static str;

    fn pack(&self, packet: &mut Packet) -> PacketResult<()>;
    fn unpack(packet: &mut Packet) -> PacketResult<Self>;

    fn type_string() -> String {
        format!("photonstruct:{}:{}", Self::TYPE_NAME, Self::INTERFACE_UUID)
    }
}

/// Little-endian dynamic byte buffer matching `org.photonvision.common.dataflow.structures.Packet`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Packet {
    data: Vec<u8>,
    read_pos: usize,
}

impl Packet {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            read_pos: 0,
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            data: Vec::with_capacity(cap),
            read_pos: 0,
        }
    }

    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            data: bytes.into(),
            read_pos: 0,
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.read_pos)
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.read_pos = 0;
    }

    pub fn reset_read(&mut self) {
        self.read_pos = 0;
    }

    pub fn encode_u8(&mut self, val: u8) {
        self.data.push(val);
    }

    pub fn encode_bool(&mut self, val: bool) {
        self.data.push(u8::from(val));
    }

    pub fn encode_i16(&mut self, val: i16) {
        self.data.extend_from_slice(&val.to_le_bytes());
    }

    pub fn encode_i32(&mut self, val: i32) {
        self.data.extend_from_slice(&val.to_le_bytes());
    }

    pub fn encode_i64(&mut self, val: i64) {
        self.data.extend_from_slice(&val.to_le_bytes());
    }

    pub fn encode_f32(&mut self, val: f32) {
        self.data.extend_from_slice(&val.to_bits().to_le_bytes());
    }

    pub fn encode_f64(&mut self, val: f64) {
        self.data.extend_from_slice(&val.to_bits().to_le_bytes());
    }

    pub fn encode_bytes(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    pub fn encode_list<T: PacketSerde>(&mut self, items: &[T]) -> PacketResult<()> {
        if items.len() > u8::MAX as usize {
            return Err(PacketError::ListTooLong(items.len()));
        }
        self.encode_u8(items.len() as u8);
        for item in items {
            item.pack(self)?;
        }
        Ok(())
    }

    pub fn encode_i16_list(&mut self, items: &[i16]) -> PacketResult<()> {
        if items.len() > u8::MAX as usize {
            return Err(PacketError::ListTooLong(items.len()));
        }
        self.encode_u8(items.len() as u8);
        for &item in items {
            self.encode_i16(item);
        }
        Ok(())
    }

    pub fn encode_optional<T: PacketSerde>(&mut self, opt: &Option<T>) -> PacketResult<()> {
        match opt {
            Some(val) => {
                self.encode_bool(true);
                val.pack(self)?;
            }
            None => {
                self.encode_bool(false);
            }
        }
        Ok(())
    }

    fn take_bytes(&mut self, count: usize) -> PacketResult<&[u8]> {
        let rem = self.remaining();
        if rem < count {
            return Err(PacketError::Underflow {
                requested: count,
                remaining: rem,
            });
        }
        let start = self.read_pos;
        self.read_pos += count;
        Ok(&self.data[start..self.read_pos])
    }

    pub fn decode_u8(&mut self) -> PacketResult<u8> {
        Ok(self.take_bytes(1)?[0])
    }

    pub fn decode_bool(&mut self) -> PacketResult<bool> {
        match self.decode_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(PacketError::InvalidBoolean(other)),
        }
    }

    pub fn decode_i16(&mut self) -> PacketResult<i16> {
        let b = self.take_bytes(2)?;
        Ok(i16::from_le_bytes([b[0], b[1]]))
    }

    pub fn decode_i32(&mut self) -> PacketResult<i32> {
        let b = self.take_bytes(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn decode_i64(&mut self) -> PacketResult<i64> {
        let b = self.take_bytes(8)?;
        Ok(i64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    pub fn decode_f32(&mut self) -> PacketResult<f32> {
        let b = self.take_bytes(4)?;
        Ok(f32::from_bits(u32::from_le_bytes([b[0], b[1], b[2], b[3]])))
    }

    pub fn decode_f64(&mut self) -> PacketResult<f64> {
        let b = self.take_bytes(8)?;
        Ok(f64::from_bits(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ])))
    }

    pub fn decode_list<T: PacketSerde>(&mut self) -> PacketResult<Vec<T>> {
        let count = self.decode_u8()? as usize;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            out.push(T::unpack(self)?);
        }
        Ok(out)
    }

    pub fn decode_i16_list(&mut self) -> PacketResult<Vec<i16>> {
        let count = self.decode_u8()? as usize;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            out.push(self.decode_i16()?);
        }
        Ok(out)
    }

    pub fn decode_optional<T: PacketSerde>(&mut self) -> PacketResult<Option<T>> {
        if self.decode_bool()? {
            Ok(Some(T::unpack(self)?))
        } else {
            Ok(None)
        }
    }
}
