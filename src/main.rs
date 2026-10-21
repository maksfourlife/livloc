use std::alloc::System;

use crate::allocator::TrackingAllocator;

mod allocator;

#[global_allocator]
static ALLOC: TrackingAllocator<System> = TrackingAllocator::new(System);

fn main() {
    println!("Hello, world!");
}
