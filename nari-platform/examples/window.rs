use core::num::NonZeroU32;
use nari_platform::{ControlFlow, Event, Platform, SurfaceArea};

fn main() {
    let platform = Platform::new();
    let surface = &platform.surface;
    let context = unsafe { softbuffer::Context::new(surface) }.unwrap();
    let mut surface = unsafe { softbuffer::Surface::new(&context, surface) }.unwrap();

    platform.run(move |event_loop, event| {
        match event {
            Event::Resize(extent) => {
                surface
                    .resize(
                        NonZeroU32::new(extent.width as u32).unwrap(),
                        NonZeroU32::new(extent.height as u32).unwrap(),
                    )
                    .unwrap();

                event_loop.surface.redraw();
            }

            Event::Paint => {
                let extent = event_loop.surface.extent();
                let width = extent.width as u32;
                let height = extent.height as u32;

                let mut buffer = surface.buffer_mut().unwrap();
                for index in 0..(width * height) {
                    buffer[index as usize] = 0;
                }

                buffer.present().unwrap();
            }
            _ => (),
        }
        ControlFlow::Continue
    });
}
