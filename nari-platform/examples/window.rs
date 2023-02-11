use std::{cell::{Cell, RefCell}, ffi::OsStr, iter::once, os::windows::ffi::OsStrExt, ptr, rc::Rc};
use windows_sys::Win32::{
    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM, ERROR_WMI_SET_FAILURE},
    System::SystemServices::IMAGE_DOS_HEADER,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassExW,
        SetWindowLongPtrW, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT,
        SW_SHOW, WM_QUIT, WNDCLASSEXW, WS_EX_APPWINDOW, WS_OVERLAPPEDWINDOW, GWL_USERDATA,
        GetWindowLongPtrW, WM_DESTROY, WM_CREATE, CREATESTRUCTW, WM_PAINT, IDC_ARROW, LoadCursorW, WM_SETCURSOR,
        HTCLIENT, SetCursor,
    },
    Graphics::Gdi::{RedrawWindow, ValidateRect, RDW_INTERNALPAINT},
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, WindowsDisplayHandle, Win32WindowHandle};

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
        let ptr = GetWindowLongPtrW(window,GWL_USERDATA) as *mut Rc<EventLoop>;
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

    // trigger redraw on events
    if msg != WM_PAINT {
        RedrawWindow(
            window,
            ptr::null(),
            0,
            RDW_INTERNALPAINT,
        );
    }

    match msg {
        WM_PAINT => {
            event_loop.send(Event::Paint);
            ValidateRect(window, ptr::null());
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
}

struct EventLoop {
    control_flow: Cell<ControlFlow>,
    event_callback: RefCell<Box<dyn FnMut(Event) -> ControlFlow>>,
}

impl EventLoop {
    fn send(&self, event: Event) {
        let control_flow = (self.event_callback.borrow_mut())(event);
        self.control_flow.set(control_flow);
    }
}

struct Platform {
    hwnd: HWND,
    event_loop: Rc<EventLoop>,
}

impl Platform {
    pub fn new() -> Self {
        let mut event_loop = Rc::new(EventLoop {
            control_flow: Cell::new(ControlFlow::Continue),
            event_callback: RefCell::new(Box::new(|_| ControlFlow::Exit)),
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

            let mut user_data = event_loop.clone();

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
                &mut user_data as *const _ as _,
            );

            ShowWindow(hwnd, SW_SHOW);

            Platform {
                hwnd,
                event_loop,
            }
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

impl HasRawDisplayHandle for Platform {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Windows(WindowsDisplayHandle)
    }
}

impl HasRawWindowHandle for Platform {
    fn raw_window_handle(&self) -> RawWindowHandle {
        RawWindowHandle::Win32(Win32WindowHandle {
            hwnd: self.hwnd,
            hinstance: get_instance_handle(),
        })
    }
}

fn main() {
    let platform = Platform::new();
    platform.run(|event| {
        match event {
            Event::Paint => println!("paint"),
        }

        ControlFlow::Continue
    })
}
