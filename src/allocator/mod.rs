use std::{
    alloc::{GlobalAlloc, Layout, LayoutError},
    ptr,
    sync::{Arc, LazyLock},
};

use backtrace::Backtrace;

use crate::allocator::{alloc_guard::AllocGuard, tree::Tree};

mod alloc_guard;
mod tree;

pub struct TrackingAllocator<T> {
    inner: T,
    tree: LazyLock<Arc<Tree>>,
}

impl<T> TrackingAllocator<T> {
    pub const fn new(inner: T) -> Self {
        Self {
            inner,
            tree: LazyLock::new(|| Arc::new(Tree::new())),
        }
    }
}

unsafe impl<T> GlobalAlloc for TrackingAllocator<T>
where
    T: GlobalAlloc,
{
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Ok((alloc_layout, offset)) = get_alloc_layout(layout) else {
            return ptr::null_mut();
        };

        let base = unsafe { self.inner.alloc(alloc_layout) };

        if base.is_null() {
            return base;
        }

        // Default: allocation is not tracked.
        unsafe {
            base.cast::<*const Tree>().write(ptr::null());
        }

        // Recursion only affects tracking, not layout.
        if let Some(_guard) = AllocGuard::enter() {
            let bt = Backtrace::new();

            let node = self.tree.clone().alloc(bt.frames(), layout.size());

            let node_ptr = Arc::into_raw(node);

            unsafe {
                base.cast::<*const Tree>().write(node_ptr);
            }
        }

        unsafe { base.add(offset) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let (alloc_layout, offset) = get_alloc_layout(layout).expect("invalid allocation layout");

        let base = unsafe { ptr.sub(offset) };

        let tree_ptr = unsafe { base.cast::<*const Tree>().read() };

        if !tree_ptr.is_null() {
            let node = unsafe { Arc::from_raw(tree_ptr) };
            node.dealloc(layout.size());
        }

        unsafe {
            self.inner.dealloc(base, alloc_layout);
        }
    }
}

fn get_alloc_layout(inner: Layout) -> Result<(Layout, usize), LayoutError> {
    Layout::new::<*const Tree>()
        .extend(inner)
        .map(|x| (x.0.pad_to_align(), x.1))
}
