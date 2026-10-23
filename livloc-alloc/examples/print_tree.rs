use std::alloc::System;

use livloc_alloc::LivlocAlloc;

#[global_allocator]
static ALLOC: LivlocAlloc<System> = LivlocAlloc::new(System);

fn main() {
    println!("{:?}", ALLOC.get_tree());
}
