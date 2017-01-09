extern crate gfx_core as core;

use std::mem;
use core::dummy::DummyResources;
use core::buffer;
use core::memory::{Bind, Usage};
use core::handle::{Buffer, Manager, Producer};

fn mock_buffer<T>(len: usize) -> Buffer<DummyResources, T> {
    use core::memory::Typed;
    let mut handler = Manager::new();
    let raw = handler.make_buffer((), buffer::Info {
        role: buffer::Role::Vertex,
        usage: Usage::Data,
        size: mem::size_of::<T>() * len,
        stride: 0,
        bind: Bind::empty(),
    }, None);
    Typed::new(raw)
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
    let _ = man.make_shader(());
    let mut count = 0u8;
    man.clean_with(&mut count,
        |_,_| (),
        |b,_| { *b += 1; },
        |_,_| (),
        |_,_| (),
        |_,_| (),
        |_,_| (),
        |_,_| (),
        |_,_| (),
        |_,_| (),
        |_,_| (),
        |_,_| (),
        );
    assert_eq!(count, 1);
}
