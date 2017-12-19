use std::os::raw::c_int;

extern "C" {
    fn emscripten_get_canvas_size(
        width: *mut c_int,
        height: *mut c_int,
        is_fullscreen: *mut c_int,
    );
}

pub fn get_canvas_size() -> (u16, u16) {
    let (mut width, mut height, mut fullscreen) = (0, 0, 0);
    unsafe {
        emscripten_get_canvas_size(&mut width, &mut height, &mut fullscreen);
    }
    (width as u16, height as u16)
}
