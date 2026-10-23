use std::{
    alloc::System, collections::VecDeque, error::Error, fs::File, io::Read, mem::MaybeUninit,
    time::Duration,
};

use livloc_alloc::LivlocAlloc;
use tokio::time::interval;

#[global_allocator]
static ALLOC: LivlocAlloc<System> = LivlocAlloc::new(System);

/// Maximum size of a single allocation
const MAX_SIZE: usize = 256;
/// Maximum number of allocations
const MAX_ALLOCS: usize = 50;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let serve = ALLOC.serve("127.0.0.1:3000").await?;
    let _handle = tokio::spawn(serve.into_future());

    let mut allocs = VecDeque::new();

    let mut interval = interval(Duration::from_secs(5));

    let mut rand_reader = File::open("/dev/random")?;

    loop {
        interval.tick().await;

        let mut bytes = [0; 8];
        rand_reader.read_exact(&mut bytes)?;
        let rand_size = usize::from_le_bytes(bytes) % MAX_SIZE;

        dbg!(rand_size);

        let alloc = Box::<[MaybeUninit<u8>]>::new_uninit_slice(rand_size);
        allocs.push_front(alloc);

        if allocs.len() > MAX_ALLOCS {
            allocs.pop_back();
        }
    }
}
