use std::sync::{
    Arc, Mutex, Weak,
    atomic::{self, AtomicUsize},
};

use backtrace::BacktraceFrame;

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
}
