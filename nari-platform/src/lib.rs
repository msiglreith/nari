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
    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Gdi::{ScreenToClient, ValidateRect},
    System::SystemServices::IMAGE_DOS_HEADER,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW,
        GetWindowLongPtrW, LoadCursorW, RegisterClassExW, SetWindowLongPtrW, ShowWindow,
        TranslateMessage, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, GWL_USERDATA,
        HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTLEFT, HTRIGHT, HTTOP,
        HTTOPLEFT, HTTOPRIGHT, IDC_ARROW, NCCALCSIZE_PARAMS, SW_SHOW, WM_CREATE, WM_DESTROY,
        WM_NCCALCSIZE, WM_NCCREATE, WM_NCHITTEST, WM_PAINT, WM_SIZE, WNDCLASSEXW, WS_EX_APPWINDOW,
        WS_SYSMENU, WS_CAPTION
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
    let surface = Surface { hwnd: window };

    match msg {
        WM_NCCREATE => println!("WM_NCCREATE"),
        WM_NCCALCSIZE => println!("WM_NCCALCSIZE"),
        WM_CREATE => println!("WM_CREATE"),
        _ => println!("{}", msg),
    }

    let event_loop = {
        let ptr = GetWindowLongPtrW(window, GWL_USERDATA) as *mut EventLoop;
        if ptr.is_null() {
            return match msg {
                WM_NCCREATE => {
                    let createstruct = &mut *(lparam as *mut CREATESTRUCTW);
                    SetWindowLongPtrW(window, GWL_USERDATA, createstruct.lpCreateParams as _);
                    DefWindowProcW(window, msg, wparam, lparam)
                }
                _ => DefWindowProcW(window, msg, wparam, lparam),
            };
        }

        &mut *ptr
    };

    match msg {
        WM_NCCALCSIZE => {
            if wparam == false.into() {
                return DefWindowProcW(window, msg, wparam, lparam);
            }

            let params = &mut *(lparam as *mut NCCALCSIZE_PARAMS);
            params.rgrc[0].top += 1;
            params.rgrc[0].bottom += 1;

            0
        }

        WM_NCHITTEST => {
            let x = loword(lparam as u32) as i32;
            let y = hiword(lparam as u32) as i32;

            let mut point = POINT { x, y };
            ScreenToClient(window, &mut point);

            let mut area = SurfaceArea::Client;
            event_loop.send(
                surface,
                Event::Hittest {
                    x: point.x,
                    y: point.y,
                    area: &mut area,
                },
            );

            let wm_area = match area {
                SurfaceArea::Client => HTCLIENT,
                SurfaceArea::Top => HTTOP,
                SurfaceArea::Bottom => HTBOTTOM,
                SurfaceArea::Left => HTLEFT,
                SurfaceArea::Right => HTRIGHT,
                SurfaceArea::BottomLeft => HTBOTTOMLEFT,
                SurfaceArea::BottomRight => HTBOTTOMRIGHT,
                SurfaceArea::TopLeft => HTTOPLEFT,
                SurfaceArea::TopRight => HTTOPRIGHT,
                SurfaceArea::Caption => HTCAPTION,
            };

            wm_area as LRESULT
        }

        WM_PAINT => {
            event_loop.send(surface, Event::Paint);
            ValidateRect(window, ptr::null());

            0
        }
        WM_SIZE => {
            let width = loword(lparam as u32) as u32;
            let height = hiword(lparam as u32) as u32;
            event_loop.send(surface, Event::Resize(Extent { width, height }));

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
pub enum ControlFlow {
    Continue,
    Exit,
}

pub enum SurfaceArea {
    Client,
    Left,
    TopLeft,
    BottomLeft,
    Right,
    TopRight,
    BottomRight,
    Bottom,
    Top,
    Caption,
}

pub enum Event<'a> {
    Paint,
    Resize(Extent),
    Hittest {
        x: i32,
        y: i32,
        area: &'a mut SurfaceArea,
    },
}

struct EventLoop {
    control_flow: Cell<ControlFlow>,
    event_callback: RefCell<Box<dyn FnMut(Surface, Event) -> ControlFlow>>,
}

impl EventLoop {
    fn send(&self, surface: Surface, event: Event) {
        let mut callback = self.event_callback.borrow_mut();
        let control_flow = callback(surface, event);
        self.control_flow.set(control_flow);
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Extent {
    pub width: u32,
    pub height: u32,
}

pub struct Surface {
    hwnd: HWND,
}

impl Surface {
    pub fn show(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_SHOW);
        }
    }

    pub fn extent(&self) -> Extent {
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
}
pub struct Platform {
    pub surface: Surface,
    event_loop: Rc<EventLoop>,
}

impl Platform {
    pub fn new() -> Self {
        let event_loop = Rc::new(EventLoop {
            control_flow: Cell::new(ControlFlow::Continue),
            event_callback: RefCell::new(Box::new(|_, _| ControlFlow::Continue)),
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

            use windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_ACCEPTFILES;
            use windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_WINDOWEDGE;
            use windows_sys::Win32::UI::WindowsAndMessaging::{WS_MINIMIZEBOX, WS_SIZEBOX, WS_MAXIMIZEBOX};

            let title = encode_wide("nari");

            // style required to support aero behavior
            let style = WS_SYSMENU | WS_SIZEBOX | WS_CAPTION | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
            let style_ex = WS_EX_APPWINDOW | WS_EX_WINDOWEDGE | WS_EX_ACCEPTFILES;

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

            Platform {
                surface: Surface { hwnd },
                event_loop,
            }
        }
    }

    pub fn run<F: FnMut(Surface, Event) -> ControlFlow + 'static>(self, callback: F) {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOREPOSITION,
            SWP_NOSIZE, SWP_NOZORDER,
        };

        let _ = self.event_loop.event_callback.replace(Box::new(callback));

        // force recalc to trigger WM_NCCALCSIZE otherwise the frame will be still seen
        unsafe {
            SetWindowPos(
                self.surface.hwnd,
                0,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED
                    | SWP_NOSIZE
                    | SWP_NOZORDER
                    | SWP_NOREPOSITION
                    | SWP_NOMOVE
                    | SWP_NOACTIVATE,
            );
        }

        self.surface.show();

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

unsafe impl HasRawDisplayHandle for Surface {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Windows(WindowsDisplayHandle::empty())
    }
}

unsafe impl HasRawWindowHandle for Surface {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = Win32WindowHandle::empty();
        handle.hwnd = self.hwnd as _;
        handle.hinstance = get_instance_handle() as _;

        RawWindowHandle::Win32(handle)
    }
}
