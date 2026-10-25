use alloc::{
    borrow::{Cow, ToOwned},
    vec::Vec,
};
use core::str::Utf8Error;

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("insufficient len")]
    InsufficientLen,
    #[error("invalid bit pattern")]
    InvalidBitPattern,
    #[error(transparent)]
    Utf8(#[from] Utf8Error),
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

impl Decode<'_> for isize {
    fn decode(data: &mut &'_ [u8]) -> DecodeResult<Self> {
        Ok(i64::decode(data)? as isize)
    }
}

impl Decode<'_> for bool {
    fn decode(data: &mut &'_ [u8]) -> DecodeResult<Self> {
        match u8::decode(data)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(DecodeError::InvalidBitPattern),
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
        core::str::from_utf8(bytes).map_err(DecodeError::Utf8)
    }
}

impl<'de, T> Decode<'de> for Cow<'de, T>
where
    T: ?Sized + 'de + ToOwned,
    &'de T: Decode<'de>,
{
    fn decode(data: &mut &'de [u8]) -> DecodeResult<Self> {
        <&'de T>::decode(data).map(Self::Borrowed)
    }
}

impl<'de, T: Decode<'de>> Decode<'de> for Vec<T> {
    fn decode(data: &mut &'de [u8]) -> DecodeResult<Self> {
        let len = usize::decode(data)?;
        (0..len).map(|_| T::decode(data)).collect()
    }
}

macro_rules! impl_decode_tuples {
    ($g0:ident $(,$gs:ident)*) => {
        impl<'de, $g0, $($gs),*> Decode<'de> for ($g0, $($gs),*)
        where
            $g0: Decode<'de>,
            $($gs: Decode<'de>,)*
        {
            fn decode(data: &mut &'de [u8]) -> DecodeResult<Self> {
                Ok(( $g0::decode(data)?, $($gs::decode(data)?),* ))
            }
        }

        impl_decode_tuples!($($gs),*);
    };
    () => {};
}

impl_decode_tuples!(A, B, C, D, E, F, G);

pub trait Write {
    type Err;

    fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Err>;
}

// manual vector impl conflicts with std::io::Write
#[cfg(not(feature = "std"))]
impl Write for Vec<u8> {
    type Err = core::convert::Infallible;

    fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Err> {
        self.extend_from_slice(buf);
        Ok(())
    }
}

#[cfg(feature = "std")]
impl<T: std::io::Write> Write for T {
    type Err = std::io::Error;

    fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Err> {
        self.write_all(buf)
    }
}

pub trait Encode {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<(), W::Err>;
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

impl Encode for isize {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<(), W::Err> {
        (*self as i64).encode(writer)
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
        writer.write_all(self)?;
        Ok(())
    }
}

impl Encode for &str {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<(), W::Err> {
        self.as_bytes().encode(writer)
    }
}

impl<T: Encode> Encode for Vec<T> {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<(), W::Err> {
        self.len().encode(writer)?;
        self.iter().try_for_each(|item| item.encode(writer))?;
        Ok(())
    }
}

// impl<T: Encode> Encode for &T {
//     fn encode<W: Write>(&self, writer: &mut W) -> Result<(), W::Err> {
//         (*self).encode(writer)
//     }
// }

impl<'a, T> Encode for Cow<'a, T>
where
    T: ?Sized + 'a + ToOwned,
    for<'b> &'b T: Encode,
{
    fn encode<W: Write>(&self, writer: &mut W) -> Result<(), W::Err> {
        self.as_ref().encode(writer)
    }
}

macro_rules! impl_encode_tuples {
    ($v0:ident: $g0:ident $(,$vs:ident:$gs:ident)*) => {
        impl<$g0, $($gs),*> Encode for ($g0, $($gs),*)
        where
            $g0: Encode,
            $($gs: Encode,)*
        {
            fn encode<W: Write>(&self, writer: &mut W) -> Result<(), W::Err> {
                let ($v0 $(,$vs)*,) = self;
                $v0.encode(writer)?;
                $($vs.encode(writer)?;)*
                Ok(())
            }
        }

        impl_encode_tuples!($($vs:$gs),*);
    };
    () => {};
}

impl_encode_tuples!(a: A, b: B, c: C, d: D, e: E, f: F, g: G);
