use wincode::{SchemaRead, SchemaWrite, WriteResult, io::Writer};

use crate::codec::{Decode, DecodeResult, Encode};

mod codec;

// [tree][[frame1][symbol11...symbol1M][childTree1[...]]...[frameN][frameN1...frameNM][childTreeN[...]]]

#[derive(SchemaRead, SchemaWrite)]
struct TreeHeader {
    size: usize,
    count: usize,
    children: usize,
}

#[derive(SchemaRead, SchemaWrite)]
struct FrameHeader {
    ip: usize,
    symbol_address: usize,
    symbols: usize,
}

#[derive(SchemaRead, SchemaWrite)]
struct Symbol<'a> {
    name: Option<&'a str>,
    filename: Option<&'a [u8]>,
    colno: Option<u32>,
    lineno: Option<u32>,
}

impl Encode for TreeHeader {
    fn encode<W: codec::Write>(&self, buf: &mut W) -> Result<(), W::Err> {
        self.size.encode(buf)?;
        self.count.encode(buf)?;
        self.children.encode(buf)
    }
}

impl Decode<'_> for TreeHeader {
    fn decode(data: &mut &'_ [u8]) -> DecodeResult<Self> {
        Ok(Self {
            size: Decode::decode(data)?,
            count: Decode::decode(data)?,
            children: Decode::decode(data)?,
        })
    }
}

impl Encode for FrameHeader {
    fn encode<W: codec::Write>(&self, buf: &mut W) -> Result<(), W::Err> {
        self.ip.encode(buf)?;
        self.symbol_address.encode(buf)?;
        self.symbols.encode(buf)
    }
}

impl Decode<'_> for FrameHeader {
    fn decode(data: &mut &'_ [u8]) -> DecodeResult<Self> {
        Ok(Self {
            ip: Decode::decode(data)?,
            symbol_address: Decode::decode(data)?,
            symbols: Decode::decode(data)?,
        })
    }
}

impl Encode for Symbol<'_> {
    fn encode<W: codec::Write>(&self, buf: &mut W) -> Result<(), W::Err> {
        self.name.encode(buf)?;
        self.filename.encode(buf)?;
        self.colno.encode(buf)?;
        self.lineno.encode(buf)
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

/// (children_left, ..)
struct Ancestors(Option<(usize, Box<Self>)>);

pub struct TreeSerializer<'w, W> {
    wtr: &'w mut W,
    ancestors: Ancestors,
}

pub struct ChildrenSerializer<'w, W> {
    wtr: &'w mut W,
    children_left: usize,
    /// ancestors of same level tree serializer
    ancestors: Ancestors,
}

pub struct FrameSerializer<'w, W> {
    wtr: &'w mut W,
    symbols_left: usize,
    children_left: usize, // inherited from ChildrenSerializer
    ancestors: Ancestors,
}

impl<'w, W: Writer> TreeSerializer<'w, W> {
    pub fn new(wtr: &'w mut W) -> Self {
        Self {
            wtr,
            ancestors: Ancestors(None),
        }
    }

    pub fn begin_tree(
        mut self,
        size: usize,
        count: usize,
        children: usize,
    ) -> WriteResult<ChildrenSerializer<'w, W>> {
        let tree_repr = TreeHeader {
            size,
            count,
            children,
        };
        wincode::serialize_into(&mut self.wtr, &tree_repr)?;
        Ok(ChildrenSerializer {
            wtr: self.wtr,
            children_left: children,
            ancestors: self.ancestors,
        })
    }

    // TODO: check that tree is serialized
    pub fn finish(self) -> Option<ChildrenSerializer<'w, W>> {
        self.ancestors.0.map(|x| ChildrenSerializer {
            wtr: self.wtr,
            children_left: x.0,
            ancestors: *x.1,
        })
    }
}

impl<'w, W: Writer> ChildrenSerializer<'w, W> {
    pub fn begin_child(
        mut self,
        ip: usize,
        symbol_address: usize,
        symbols: usize,
    ) -> WriteResult<FrameSerializer<'w, W>> {
        let frame_repr = FrameHeader {
            ip,
            symbol_address,
            symbols,
        };
        wincode::serialize_into(&mut self.wtr, &frame_repr)?;
        Ok(FrameSerializer {
            wtr: self.wtr,
            symbols_left: symbols,
            children_left: self.children_left - 1, // TODO: check children
            ancestors: self.ancestors,
        })
    }

    /// Returns back original tree serializer
    pub fn finish(self) -> Result<TreeSerializer<'w, W>, Self> {
        if self.children_left == 0 {
            Ok(TreeSerializer {
                wtr: self.wtr,
                ancestors: self.ancestors,
            })
        } else {
            Err(self)
        }
    }
}

impl<'w, W: Writer> FrameSerializer<'w, W> {
    pub fn serialize_symbol(
        &mut self,
        name: Option<&str>,
        filename: Option<&[u8]>,
        lineno: Option<u32>,
        colno: Option<u32>,
    ) -> WriteResult<()> {
        // TODO: or error
        if let Some(next_symbols_left) = self.symbols_left.checked_sub(1) {
            let symbol_repr = Symbol {
                name,
                filename,
                colno,
                lineno,
            };
            wincode::serialize_into(&mut self.wtr, &symbol_repr)?;
            self.symbols_left = next_symbols_left;
        }
        Ok(())
    }

    /// Creates tree serializer for child
    pub fn finish(self) -> Result<TreeSerializer<'w, W>, Self> {
        if self.symbols_left == 0 {
            Ok(TreeSerializer {
                wtr: self.wtr,
                ancestors: Ancestors(Some((self.children_left, Box::new(self.ancestors)))),
            })
        } else {
            Err(self)
        }
    }
}

struct TreeDeserializer<'a> {
    data: &'a [u8],
    header: TreeHeader,
}

struct FrameDeserializer<'a> {
    data: &'a [u8],
    header: FrameHeader,
}

impl<'a> TreeDeserializer<'a> {
    pub fn new(mut data: &'a [u8]) -> DecodeResult<Self> {
        let header = TreeHeader::decode(&mut data)?;
        Ok(Self { data, header })
    }

    pub fn size(&self) -> usize {
        self.header.size
    }

    pub fn count(&self) -> usize {
        self.header.count
    }

    pub fn begin_frame(mut self) -> DecodeResult<FrameDeserializer<'a>> {
        // TODO: check children left
        let header = FrameHeader::decode(&mut self.data)?;
        Ok(FrameDeserializer {
            data: self.data,
            header,
        })
    }
}

impl<'a> FrameDeserializer<'a> {
    pub fn ip(&self) -> usize {
        self.header.ip
    }

    pub fn symbol_address(&self) -> usize {
        self.header.symbol_address
    }

    pub fn deserialize_symbol(&mut self) -> DecodeResult<Option<Symbol<'a>>> {
        let Some(next_symbols) = self.header.symbols.checked_sub(1) else {
            return Ok(None);
        };
        let symbol = Symbol::decode(&mut self.data)?;
        self.header.symbols = next_symbols;
        Ok(Some(symbol))
    }

    pub fn finish(self) -> DecodeResult<TreeDeserializer<'a>> {
        // TODO: check all symbols deserialized
        TreeDeserializer::new(self.data)
    }
}

#[cfg(test)]
mod test {
    use wincode::{WriteResult, io::Writer};

    use crate::TreeSerializer;

    struct MockTree<'a> {
        size: usize,
        count: usize,
        children: Vec<(MockFrame<'a>, Self)>,
    }

    struct MockFrame<'a> {
        ip: usize,
        symbol_address: usize,
        symbols: Vec<MockSymbol<'a>>,
    }

    struct MockSymbol<'a> {
        name: Option<&'a str>,
        filename: Option<&'a [u8]>,
        lineno: Option<u32>,
        colno: Option<u32>,
    }

    impl MockTree<'_> {
        fn serialize<'w, W: Writer>(
            &self,
            tree_ser: TreeSerializer<'w, W>,
        ) -> WriteResult<TreeSerializer<'w, W>> {
            let mut children_ser =
                tree_ser.begin_tree(self.size, self.count, self.children.len())?;

            for (frame, child) in &self.children {
                let mut frame_ser = children_ser.begin_child(
                    frame.ip,
                    frame.symbol_address,
                    frame.symbols.len(),
                )?;

                for symbol in &frame.symbols {
                    frame_ser.serialize_symbol(
                        symbol.name,
                        symbol.filename,
                        symbol.lineno,
                        symbol.colno,
                    )?;
                }

                let tree_ser = frame_ser.finish().ok().expect("all symbols serialized");

                let tree_ser = child.serialize(tree_ser)?;

                children_ser = tree_ser.finish().expect("child tree serializer has parent");
            }

            let tree_ser = children_ser.finish().ok().expect("all children serialized");

            Ok(tree_ser)
        }
    }

    #[test]
    fn test_serialize_tree() {
        let tree = MockTree {
            size: 100,
            count: 10,
            children: vec![(
                MockFrame {
                    ip: 125,
                    symbol_address: 110,
                    symbols: vec![],
                },
                MockTree {
                    size: 50,
                    count: 7,
                    children: vec![],
                },
            )],
        };
        let mut out = vec![];
        let ser = TreeSerializer::new(&mut out);
        tree.serialize(ser).unwrap();
        dbg!(out);
    }
}
