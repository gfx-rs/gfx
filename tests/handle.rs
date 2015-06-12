extern crate gfx;

use std::mem;
use gfx::device::{BufferRole, BufferInfo, BufferUsage, Resources};
use gfx::handle::{Buffer, Manager, Producer};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum TestResources {}
impl gfx::Resources for TestResources {
    type Buffer = ();
    type ArrayBuffer = ();
    type Shader = ();
    type Program = ();
    type FrameBuffer = ();
    type Surface = ();
    type Texture = ();
    type Sampler = ();
}

fn mock_buffer<T>(len: usize) -> Buffer<TestResources, T> {
    let mut handler = Manager::new();
    Buffer::from_raw(
        handler.make_buffer((), BufferInfo {
            role: BufferRole::Vertex,
            usage: BufferUsage::Static,
            size: mem::size_of::<T>() * len,
        }),
    )
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
    let mut man: Manager<TestResources> = Manager::new();
    let _ = man.make_frame_buffer(());
    let mut count = 0u8;
    man.clean_with(&mut count,
        |_,_| (),
        |_,_| (),
        |_,_| (),
        |_,_| (),
        |b,_| { *b += 1; },
        |_,_| (),
        |_,_| (),
        |_,_| ()
        );
    assert_eq!(count, 1);
}
