extern crate gfx_core;

use std::mem;
use gfx_core::{BufferRole, BufferInfo, BufferUsage};
use gfx_core::dummy::DummyResources;
use gfx_core::handle::{Buffer, Manager, Producer};

fn mock_buffer<T>(len: usize) -> Buffer<DummyResources, T> {
    use gfx_core::Phantom;
    let mut handler = Manager::new();
    let raw = handler.make_buffer((), BufferInfo {
        role: BufferRole::Vertex,
        usage: BufferUsage::Static,
        size: mem::size_of::<T>() * len,
    });
    Phantom::new(raw)
}

#[test]
fn test_buffer_len() {
    assert_eq!(mock_buffer::<u8>(8).len(), 8);
    assert_eq!(mock_buffer::<u16>(8).len(), 8);
}

#[test]
#[should_panic]
fn test_buffer_zero_len() {
    let _ = mock_buffer::<()>(0).len();
}

#[test]
fn test_cleanup() {
    let mut man: Manager<DummyResources> = Manager::new();
    let _ = man.make_frame_buffer(());
    let mut count = 0u8;
    man.clean_with(&mut count,
        |_,_| (),
        |_,_| (),
        |_,_| (),
        |_,_| (),
        |_,_| (),
        |_,_| (),
        |_,_| (),
        |_,_| (),
        |b,_| { *b += 1; },
        |_,_| (),
        |_,_| (),
        |_,_| (),
        |_,_| (),
        |_,_| (),
        |_,_| ()
        );
    assert_eq!(count, 1);
}
