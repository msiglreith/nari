use gpu::vk;
use nari_gpu as gpu;
use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, Win32WindowHandle,
    WindowsDisplayHandle,
};
use std::{
    cell::{Cell, RefCell},
    ffi::OsStr,
    iter::once,
    mem::MaybeUninit,
    os::windows::ffi::OsStrExt,
    ptr,
    rc::Rc,
};
use windows_sys::Win32::{
    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::{RedrawWindow, ValidateRect, RDW_INTERNALPAINT},
    System::SystemServices::IMAGE_DOS_HEADER,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW,
        GetWindowLongPtrW, LoadCursorW, RegisterClassExW, SetWindowLongPtrW, ShowWindow,
        TranslateMessage, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, GWL_USERDATA,
        HTCLIENT, IDC_ARROW, SW_SHOW, WM_CREATE, WM_DESTROY, WM_PAINT, WM_SETCURSOR,
        WM_SIZE, WNDCLASSEXW, WS_EX_APPWINDOW, WS_OVERLAPPEDWINDOW,
    },
};

fn encode_wide(string: impl AsRef<OsStr>) -> Vec<u16> {
    string.as_ref().encode_wide().chain(once(0)).collect()
}

fn get_instance_handle() -> HINSTANCE {
    extern "C" {
        static __ImageBase: IMAGE_DOS_HEADER;
    }
    unsafe { &__ImageBase as *const _ as _ }
}

#[inline(always)]
const fn loword(x: u32) -> u16 {
    (x & 0xFFFF) as u16
}

#[inline(always)]
const fn hiword(x: u32) -> u16 {
    ((x >> 16) & 0xFFFF) as u16
}

unsafe extern "system" fn window_proc(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let event_loop = {
        let ptr = GetWindowLongPtrW(window, GWL_USERDATA) as *mut EventLoop;
        if ptr.is_null() {
            return match msg {
                WM_CREATE => {
                    let createstruct = &mut *(lparam as *mut CREATESTRUCTW);
                    SetWindowLongPtrW(window, GWL_USERDATA, createstruct.lpCreateParams as _);
                    0
                }
                _ => DefWindowProcW(window, msg, wparam, lparam),
            };
        }

        &mut *ptr
    };

    match msg {
        WM_PAINT => {
            event_loop.send(Event::Paint);
            ValidateRect(window, ptr::null());

            0
        }
        WM_SIZE => {
            let width = loword(lparam as u32) as u32;
            let height = hiword(lparam as u32) as u32;
            event_loop.send(Event::Resize(Extent { width, height }));

            0
        }
        WM_DESTROY => {
            event_loop.control_flow.set(ControlFlow::Exit);

            0
        }
        _ => DefWindowProcW(window, msg, wparam, lparam),
    }
}

#[derive(Copy, Clone, Debug)]
enum ControlFlow {
    Continue,
    Exit,
}

enum Event {
    Paint,
    Resize(Extent),
}

struct EventLoop {
    control_flow: Cell<ControlFlow>,
    event_callback: RefCell<Box<dyn FnMut(Event) -> ControlFlow>>,
}

impl EventLoop {
    fn send(&self, event: Event) {
        let mut callback = self.event_callback.borrow_mut();
        let control_flow = callback(event);
        self.control_flow.set(control_flow);
    }
}

pub struct Extent {
    pub width: u32,
    pub height: u32,
}

struct Platform {
    hwnd: HWND,
    event_loop: Rc<EventLoop>,
}

impl Platform {
    pub fn new() -> Self {
        let event_loop = Rc::new(EventLoop {
            control_flow: Cell::new(ControlFlow::Continue),
            event_callback: RefCell::new(Box::new(|_| ControlFlow::Continue)),
        });

        unsafe {
            let hinstance = get_instance_handle();
            let class_name = encode_wide("nari::win32::class");
            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance,
                hIcon: 0,
                hCursor: LoadCursorW(0, IDC_ARROW),
                hbrBackground: 0,
                lpszMenuName: ptr::null(),
                lpszClassName: class_name.as_ptr(),
                hIconSm: 0,
            };
            RegisterClassExW(&class);

            let title = encode_wide("nari");
            let style = WS_OVERLAPPEDWINDOW;
            let style_ex = WS_EX_APPWINDOW;

            let userdata = event_loop.clone();

            let hwnd = CreateWindowExW(
                style_ex,
                class_name.as_ptr(),
                title.as_ptr(),
                style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                0,
                0,
                hinstance,
                Rc::as_ptr(&userdata) as _,
            );

            ShowWindow(hwnd, SW_SHOW);

            Platform { hwnd, event_loop }
        }
    }

    pub fn surface_size(&self) -> Extent {
        let mut rect = MaybeUninit::uninit();
        unsafe {
            GetClientRect(self.hwnd, rect.as_mut_ptr());
        }
        let rect = unsafe { rect.assume_init() };
        Extent {
            width: (rect.right - rect.left) as u32,
            height: (rect.bottom - rect.top) as u32,
        }
    }

    pub fn run<F: FnMut(Event) -> ControlFlow + 'static>(self, callback: F) {
        let _ = self.event_loop.event_callback.replace(Box::new(callback));
        'main: loop {
            unsafe {
                let mut msg = std::mem::MaybeUninit::uninit();
                let ret = GetMessageW(msg.as_mut_ptr(), 0, 0, 0);
                let msg = msg.assume_init();

                if ret != false.into() {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                } else {
                    break 'main;
                }

                if let ControlFlow::Exit = self.event_loop.control_flow.get() {
                    break 'main;
                }
            }
        }
    }
}

unsafe impl HasRawDisplayHandle for Platform {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Windows(WindowsDisplayHandle::empty())
    }
}

unsafe impl HasRawWindowHandle for Platform {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = Win32WindowHandle::empty();
        handle.hwnd = self.hwnd as _;
        handle.hinstance = get_instance_handle() as _;

        RawWindowHandle::Win32(handle)
    }
}

fn main() -> anyhow::Result<()> {
    unsafe {
        let platform = Platform::new();
        let instance = gpu::Instance::new(&platform)?;
        let mut gpu = gpu::Gpu::new(&instance, std::path::Path::new(""))?;

        let mut size = platform.surface_size();
        let mut wsi = gpu::Swapchain::new(
            &instance,
            &gpu,
            size.width,
            size.height,
            vk::PresentModeKHR::IMMEDIATE,
        )?;

        platform.run(move |event| {
            match event {
                Event::Resize(extent) => {
                    size = extent;
                    wsi.resize(&gpu, size.width, size.height).unwrap();
                }
                Event::Paint => {
                    let frame = wsi.acquire().unwrap();
                    let pool = gpu.acquire_pool().unwrap();

                    gpu.cmd_barriers(
                        pool,
                        &[],
                        &[gpu::ImageBarrier {
                            image: wsi.frame_images[frame.id].aspect(vk::ImageAspectFlags::COLOR),
                            src: gpu::ImageAccess {
                                access: gpu::Access::NONE,
                                stage: gpu::Stage::NONE,
                                layout: gpu::ImageLayout::UNDEFINED,
                            },
                            dst: gpu::ImageAccess {
                                access: gpu::Access::COLOR_ATTACHMENT_WRITE,
                                stage: gpu::Stage::COLOR_ATTACHMENT_OUTPUT,
                                layout: gpu::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                            },
                        }],
                    );

                    let area = vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: vk::Extent2D {
                            width: size.width,
                            height: size.height,
                        },
                    };
                    gpu.cmd_set_viewport(
                        pool.cmd_buffer,
                        0,
                        &[vk::Viewport {
                            x: 0.0,
                            y: 0.0,
                            width: size.width as _,
                            height: size.height as _,
                            min_depth: 0.0,
                            max_depth: 1.0,
                        }],
                    );
                    gpu.cmd_set_scissor(pool.cmd_buffer, 0, &[area]);
                    gpu.cmd_graphics_begin(
                        pool,
                        area,
                        &[gpu::GraphicsAttachment {
                            image_view: wsi.frame_rtvs[frame.id],
                            layout: gpu::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                            load: vk::AttachmentLoadOp::CLEAR,
                            store: vk::AttachmentStoreOp::STORE,
                            clear: vk::ClearValue {
                                color: vk::ClearColorValue { float32: [0.2; 4] },
                            },
                        }],
                    );

                    gpu.cmd_graphics_end(pool);

                    gpu.cmd_barriers(
                        pool,
                        &[],
                        &[gpu::ImageBarrier {
                            image: wsi.frame_images[frame.id].aspect(vk::ImageAspectFlags::COLOR),
                            src: gpu::ImageAccess {
                                access: gpu::Access::COLOR_ATTACHMENT_WRITE,
                                stage: gpu::Stage::COLOR_ATTACHMENT_OUTPUT,
                                layout: gpu::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                            },
                            dst: gpu::ImageAccess {
                                access: gpu::Access::NONE,
                                stage: gpu::Stage::NONE,
                                layout: gpu::ImageLayout::PRESENT_SRC_KHR,
                            },
                        }],
                    );

                    gpu.submit_pool(
                        pool,
                        gpu::Submit {
                            waits: &[gpu::SemaphoreSubmit {
                                semaphore: frame.acquire,
                                stage: gpu::Stage::COLOR_ATTACHMENT_OUTPUT,
                            }],
                            signals: &[gpu::SemaphoreSubmit {
                                semaphore: frame.present,
                                stage: gpu::Stage::COLOR_ATTACHMENT_OUTPUT,
                            }],
                        },
                    )
                    .unwrap();

                    wsi.present(&gpu, frame).unwrap();
                }
            }

            ControlFlow::Continue
        });

        Ok(())
    }
}
