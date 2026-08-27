//! Nothing on the audio path may allocate (`REQ-AIR-016`).
//!
//! nih-plug's `assert_process_allocs` catches this too, but **only when the
//! plugin is running inside a host in a debug build** — which is not something
//! `mise run check` does. A counting allocator makes it a test, and a test that
//! runs on every commit is the one that finds the regression.
//!
//! One test in its own binary on purpose: the allocator is global, so a second
//! test running beside it would have its allocations counted here.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use air_core::engine::{Engine, Shape};

struct Counting;

static WATCHING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if WATCHING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if WATCHING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const RATE: f32 = 48_000.0;
const BLOCK: usize = 512;

#[test]
fn neither_processing_nor_setting_the_shape_allocates() {
    // Building is the only place that may (`Engine::new`), and it happens
    // before anyone is watching.
    let mut engine = Engine::new(RATE, 1);
    let mut shape = Shape::default();
    let input: Vec<f32> = (0..BLOCK * 8)
        .map(|index| (index as f32 * 0.01).sin() * 0.4)
        .collect();

    WATCHING.store(true, Ordering::Relaxed);
    for (block, chunk) in input.chunks(BLOCK).enumerate() {
        // Moved every block, so the paths that rebuild coefficients are the
        // ones being watched rather than the early returns.
        let position = block as f32 / 8.0;
        shape.focus = position - 0.5;
        shape.character = position;
        shape.blend = position;
        shape.width = position;
        shape.depths = [position, 1.0 - position, position];
        shape.guard = position - 0.5;
        engine.set_shape(&shape);
        for sample in chunk {
            std::hint::black_box(engine.process((*sample, *sample), position, 1.0));
        }
    }
    let counted = ALLOCATIONS.load(Ordering::Relaxed);
    WATCHING.store(false, Ordering::Relaxed);

    assert_eq!(counted, 0, "the audio path allocated {counted} times");

    // **And the counter can count.** A watcher that never fires would pass the
    // assertion above without looking at anything (`VEL-10`).
    WATCHING.store(true, Ordering::Relaxed);
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    std::hint::black_box(vec![0.0f32; 16]);
    let after = ALLOCATIONS.load(Ordering::Relaxed);
    WATCHING.store(false, Ordering::Relaxed);
    assert!(
        after > before,
        "the counter never counted, so the test proves nothing"
    );
}
