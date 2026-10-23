use std::cell::Cell;

thread_local! {
    static IN_ALLOCATOR: Cell<bool> = const { Cell::new(false) };
}

pub struct AllocGuard;

impl AllocGuard {
    pub fn enter() -> Option<Self> {
        IN_ALLOCATOR
            .try_with(|flag| {
                if flag.replace(true) {
                    None // Already inside allocator
                } else {
                    Some(Self)
                }
            })
            .ok()
            .flatten()
    }
}

impl Drop for AllocGuard {
    fn drop(&mut self) {
        // Restore the flag when leaving
        let _ = IN_ALLOCATOR.try_with(|flag| flag.set(false));
    }
}
