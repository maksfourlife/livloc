use std::{
    borrow::Cow,
    sync::{
        Arc, Mutex, Weak,
        atomic::{self, AtomicUsize},
    },
};

use backtrace::BacktraceFrame;

#[derive(Debug)]
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
            // TODO: filter by symbol name if filter provided

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

    pub fn to_proto_tree(&self) -> livloc_proto::Tree<'static> {
        let children = self.children.lock().unwrap();

        let children: Vec<_> = children
            .iter()
            .map(|(frame, child)| {
                let symbols = frame
                    .symbols()
                    .iter()
                    .map(|symbol| livloc_proto::Symbol {
                        name: symbol.name().map(|x| Cow::Owned(x.to_string())),
                        filename: symbol
                            .filename()
                            .map(|x| Cow::Owned(x.as_os_str().as_encoded_bytes().to_vec())),
                        colno: symbol.colno(),
                        lineno: symbol.lineno(),
                    })
                    .collect();

                let frame = livloc_proto::Frame {
                    ip: frame.ip() as usize,
                    symbol_address: frame.symbol_address() as usize,
                    symbols,
                };

                (frame, child.to_proto_tree())
            })
            .collect();

        livloc_proto::Tree {
            size: self.size.load(atomic::Ordering::Relaxed),
            count: self.count.load(atomic::Ordering::Relaxed),
            children,
        }
    }
}
