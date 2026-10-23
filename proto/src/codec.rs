use std::{convert::Infallible, str::Utf8Error};

pub enum DecodeError {
    InsufficientLen,
    InvalidPattern,
    Utf8(Utf8Error),
}

pub type DecodeResult<T> = Result<T, DecodeError>;

pub trait Decode<'de>: Sized {
    fn decode(data: &mut &'de [u8]) -> DecodeResult<Self>;
}

macro_rules! impl_decode_primitive {
    ($($primitives:ty),*) => {
        $(
            impl Decode<'_> for $primitives {
                fn decode(data: &mut &'_ [u8]) -> DecodeResult<Self> {
                    let bytes;
                    (bytes, *data) = data
                        .split_first_chunk()
                        .ok_or(DecodeError::InsufficientLen)?;
                    Ok(<$primitives>::from_le_bytes(*bytes))
                }
            }
        )*
    };
}

impl_decode_primitive!(u8, u16, u32, u64, i8, i16, i32, i64);

impl Decode<'_> for usize {
    fn decode(data: &mut &'_ [u8]) -> DecodeResult<Self> {
        Ok(u64::decode(data)? as usize)
    }
}

impl Decode<'_> for bool {
    fn decode(data: &mut &'_ [u8]) -> DecodeResult<Self> {
        match u8::decode(data)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(DecodeError::InvalidPattern),
        }
    }
}

impl<'de, T: Decode<'de>> Decode<'de> for Option<T> {
    fn decode(data: &mut &'de [u8]) -> DecodeResult<Self> {
        let is_some = bool::decode(data)?;
        Ok(if is_some {
            Some(T::decode(data)?)
        } else {
            None
        })
    }
}

// can't straightforwardly implement on &'de T because of alignment and internal decode logic
impl<'de> Decode<'de> for &'de [u8] {
    fn decode(data: &mut &'de [u8]) -> DecodeResult<Self> {
        let len = usize::decode(data)?;
        let val;
        (val, *data) = data
            .split_at_checked(len)
            .ok_or(DecodeError::InsufficientLen)?;
        Ok(val)
    }
}

impl<'de> Decode<'de> for &'de str {
    fn decode(data: &mut &'de [u8]) -> DecodeResult<Self> {
        let bytes = <&'de [u8]>::decode(data)?;
        std::str::from_utf8(bytes).map_err(DecodeError::Utf8)
    }
}

pub trait Write {
    type Err;

    fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Err>;
}

impl Write for Vec<u8> {
    type Err = Infallible;

    fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Err> {
        self.extend_from_slice(buf);
        Ok(())
    }
}

pub trait Encode {
    fn encode<W: Write>(&self, write: &mut W) -> Result<(), W::Err>;
}

macro_rules! impl_encode_primitive {
    ($($primitives:ty),*) => {
        $(
            impl Encode for $primitives {
                fn encode<W: Write>(&self, writer: &mut W) -> Result<(), W::Err> {
                    writer.write_all(&self.to_le_bytes())
                }
            }
        )*
    };
}

impl_encode_primitive!(u8, u16, u32, u64, i8, i16, i32, i64);

impl Encode for bool {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<(), W::Err> {
        (*self as u8).encode(writer)
    }
}

impl Encode for usize {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<(), W::Err> {
        (*self as u64).encode(writer)
    }
}

impl<T: Encode> Encode for Option<T> {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<(), W::Err> {
        if let Some(some) = self {
            true.encode(writer)?;
            some.encode(writer)
        } else {
            false.encode(writer)
        }
    }
}

impl Encode for &[u8] {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<(), W::Err> {
        self.len().encode(writer)?;
        writer.write_all(self)
    }
}

impl Encode for &str {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<(), W::Err> {
        self.as_bytes().encode(writer)
    }
}
