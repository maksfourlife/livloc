#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::{borrow::Cow, vec::Vec};

use crate::codec::{Decode, DecodeResult, Encode, Write};

pub mod codec;

#[derive(Debug, Clone)]
pub struct Tree<'a> {
    pub size: usize,
    pub count: usize,
    pub children: Vec<(Frame<'a>, Self)>,
}

#[derive(Debug, Clone)]
pub struct Frame<'a> {
    pub ip: usize,
    pub symbol_address: usize,
    pub symbols: Vec<Symbol<'a>>,
}

#[derive(Debug, Clone)]
pub struct Symbol<'a> {
    pub name: Option<Cow<'a, str>>,
    pub filename: Option<Cow<'a, [u8]>>,
    pub colno: Option<u32>,
    pub lineno: Option<u32>,
}

impl Encode for Tree<'_> {
    fn encode<W: Write>(&self, write: &mut W) -> Result<(), W::Err> {
        self.size.encode(write)?;
        self.count.encode(write)?;
        self.children.encode(write)?;
        Ok(())
    }
}

impl<'de> Decode<'de> for Tree<'de> {
    fn decode(data: &mut &'de [u8]) -> DecodeResult<Self> {
        Ok(Self {
            size: Decode::decode(data)?,
            count: Decode::decode(data)?,
            children: Decode::decode(data)?,
        })
    }
}

impl Encode for Frame<'_> {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<(), W::Err> {
        self.ip.encode(writer)?;
        self.symbol_address.encode(writer)?;
        self.symbols.encode(writer)?;
        Ok(())
    }
}

impl<'de> Decode<'de> for Frame<'de> {
    fn decode(data: &mut &'de [u8]) -> DecodeResult<Self> {
        Ok(Self {
            ip: Decode::decode(data)?,
            symbol_address: Decode::decode(data)?,
            symbols: Decode::decode(data)?,
        })
    }
}

impl Encode for Symbol<'_> {
    fn encode<W: Write>(&self, writer: &mut W) -> Result<(), W::Err> {
        self.name.encode(writer)?;
        self.filename.encode(writer)?;
        self.colno.encode(writer)?;
        self.lineno.encode(writer)?;
        Ok(())
    }
}

impl<'de> Decode<'de> for Symbol<'de> {
    fn decode(data: &mut &'de [u8]) -> DecodeResult<Self> {
        Ok(Self {
            name: Decode::decode(data)?,
            filename: Decode::decode(data)?,
            colno: Decode::decode(data)?,
            lineno: Decode::decode(data)?,
        })
    }
}
