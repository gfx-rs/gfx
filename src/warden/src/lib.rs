//! Data-driven reference test framework for warding
//! against breaking changes.

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;

pub mod gpu;
pub mod raw;

#[cfg(feature = "gl")]
pub fn init_gl_surface() -> gfx_backend_gl::Surface {
    use gfx_backend_gl::glutin;

    let events_loop = glutin::event_loop::EventLoop::new();
    let windowed_context = glutin::ContextBuilder::new()
        .with_gl_profile(glutin::GlProfile::Core)
        .build_windowed(glutin::window::WindowBuilder::new(), &events_loop)
        .unwrap();
    let (context, _window) = unsafe {
        windowed_context
            .make_current()
            .expect("Unable to make window current")
            .split()
    };

    gfx_backend_gl::Surface::from_context(context)
}

#[cfg(feature = "gl-ci")]
pub fn init_gl_on_ci() -> gfx_backend_gl::Headless {
    use gfx_backend_gl::glutin;

    let events_loop = glutin::event_loop::EventLoop::new();
    let context;
    #[cfg(all(unix, not(target_vendor = "apple")))]
    {
        use gfx_backend_gl::glutin::platform::unix::HeadlessContextExt as _;
        context = glutin::ContextBuilder::new().build_surfaceless(&events_loop);
    }
    #[cfg(any(not(unix), target_vendor = "apple"))]
    {
        context = glutin::ContextBuilder::new()
            .build_headless(&events_loop, glutin::dpi::PhysicalSize::new(0, 0));
    }
    let current_context =
        unsafe { context.unwrap().make_current() }.expect("Unable to make context current");

    gfx_backend_gl::Headless::from_context(current_context)
}
