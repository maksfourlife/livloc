use std::{
    borrow::Cow,
    io,
    sync::{
        Arc, Mutex, Weak,
        atomic::{self, AtomicUsize},
    },
};

use backtrace::BacktraceFrame;
use borsh::{BorshDeserialize, BorshSerialize};

pub struct Tree {
    size: AtomicUsize,
    count: AtomicUsize,
    parent: Option<Weak<Self>>,
    children: Mutex<Vec<(BacktraceFrame, Arc<Self>)>>,
}

impl Tree {
    pub const fn new() -> Self {
        Self {
            size: AtomicUsize::new(0),
            count: AtomicUsize::new(0),
            parent: None,
            children: Mutex::new(Vec::new()),
        }
    }

    pub fn alloc(self: Arc<Self>, frames: &[BacktraceFrame], size: usize) -> Arc<Self> {
        let mut curr = self;

        for frame in frames {
            curr.size.fetch_add(size, atomic::Ordering::Relaxed);
            curr.count.fetch_add(1, atomic::Ordering::Relaxed);

            let mut children = curr.children.lock().unwrap();
            let search = children
                .iter()
                .find(|x| x.0.symbol_address() == frame.symbol_address());

            match search {
                Some((_, found)) => {
                    let found = found.clone();
                    drop(children);
                    curr = found;
                }
                None => {
                    let child = Arc::new(Self {
                        size: AtomicUsize::new(0),
                        count: AtomicUsize::new(0),
                        parent: Some(Arc::downgrade(&curr)),
                        children: Mutex::new(vec![]),
                    });
                    children.push((frame.clone(), child.clone()));
                    drop(children);
                    curr = child;
                }
            }
        }

        curr
    }

    pub fn dealloc(self: Arc<Self>, size: usize) {
        let mut curr = Some(self);
        while let Some(node) = curr {
            node.size.fetch_sub(size, atomic::Ordering::Relaxed);
            node.count.fetch_sub(1, atomic::Ordering::Relaxed);
            curr = node.parent.as_ref().and_then(Weak::upgrade);
        }
    }

    pub fn serialize(&self, writer: &mut impl io::Write) -> io::Result<()> {
        let size = self.size.load(atomic::Ordering::Relaxed);
        let count = self.count.load(atomic::Ordering::Relaxed);
        let children = self.children.lock().unwrap();

        writer.write_all(&size.to_le_bytes())?;
        writer.write_all(&count.to_le_bytes())?;

        let tree_repr = TreeRepr {
            size,
            count,
            children: children.len(),
        };
        borsh::to_writer(&mut *writer, &tree_repr)?;

        for (frame, child) in &*children {
            let frame_repr = FrameRepr {
                ip: frame.ip() as usize,
                symbol_address: frame.symbol_address() as usize,
                symbols: frame.symbols().len(),
            };
            borsh::to_writer(&mut *writer, &frame_repr)?;

            for symbol in frame.symbols() {
                let symbol_repr = SymbolRepr {
                    name: symbol.name().map(|x| x.to_string()),
                    filename: symbol
                        .filename()
                        .map(|x| Cow::Borrowed(x.as_os_str().as_encoded_bytes())),
                    colno: symbol.colno(),
                    lineno: symbol.lineno(),
                };
                borsh::to_writer(&mut *writer, &symbol_repr)?;
            }

            child.serialize(&mut *writer)?;
        }

        Ok(())
    }
}

#[derive(BorshSerialize, BorshDeserialize)]
struct TreeRepr {
    size: usize,
    count: usize,
    children: usize,
}

#[derive(BorshSerialize)]
struct FrameRepr {
    ip: usize,
    symbol_address: usize,
    symbols: usize,
}

#[derive(BorshSerialize, BorshDeserialize)]
struct SymbolRepr<'a> {
    name: Option<String>,
    filename: Option<Cow<'a, [u8]>>,
    colno: Option<u32>,
    lineno: Option<u32>,
}
