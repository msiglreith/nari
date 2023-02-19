use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, Win32WindowHandle,
    WindowsDisplayHandle,
};
use std::{
    cell::{Cell, RefCell},
    ffi::OsStr,
    iter::once,
    mem::{self, MaybeUninit},
    os::windows::ffi::OsStrExt,
    ptr,
    rc::Rc,
};
use windows_sys::Win32::{
    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM},
    Graphics::{
        Dwm::{DwmExtendFrameIntoClientArea, DwmFlush},
        Gdi::{
            GetMonitorInfoW, MonitorFromRect, RedrawWindow, ScreenToClient, ValidateRect,
            MONITORINFOEXW, MONITOR_DEFAULTTONULL, RDW_INTERNALPAINT,
        },
    },
    System::SystemServices::{IMAGE_DOS_HEADER, MK_LBUTTON, MK_RBUTTON},
    UI::{
        Controls::{HOVER_DEFAULT, MARGINS, WM_MOUSELEAVE},
        Input::KeyboardAndMouse::{
            GetKeyState, ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TME_NONCLIENT,
            TRACKMOUSEEVENT, VK_CONTROL, VK_MENU, VK_SHIFT, MAPVK_VK_TO_CHAR, MapVirtualKeyW
        },
        WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW,
            GetWindowLongPtrW, GetWindowPlacement, LoadCursorW, PostMessageW, RegisterClassExW,
            SetWindowLongPtrW, ShowWindow, TranslateMessage, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW,
            CW_USEDEFAULT, GWL_USERDATA, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION,
            HTCLIENT, HTCLOSE, HTLEFT, HTMAXBUTTON, HTMINBUTTON, HTRIGHT, HTTOP, HTTOPLEFT,
            HTTOPRIGHT, IDC_ARROW, NCCALCSIZE_PARAMS, SC_CLOSE, SC_MAXIMIZE, SC_MINIMIZE,
            SC_RESTORE, SW_MAXIMIZE, SW_SHOW, WINDOWPLACEMENT, WM_CHAR, WM_CREATE, WM_DESTROY,
            WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCALCSIZE,
            WM_NCCREATE, WM_NCHITTEST, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE,
            WM_NCMOUSEMOVE, WM_PAINT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SIZE, WM_SYSCOMMAND,
            WNDCLASSEXW, WS_CAPTION, WS_EX_ACCEPTFILES, WS_EX_APPWINDOW, WS_EX_WINDOWEDGE,
            WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SIZEBOX, WS_SYSMENU,
        },
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

    // match msg {
    //     WM_NCCREATE => println!("WM_NCCREATE"),
    //     WM_NCCALCSIZE => println!("WM_NCCALCSIZE"),
    //     WM_CREATE => println!("WM_CREATE"),
    //     WM_MOUSEMOVE => println!("WM_MOUSEMOVE"),
    //     WM_NCMOUSEMOVE => println!("WM_NCMOUSEMOVE"),
    //     WM_MOUSELEAVE => println!("WM_MOUSELEAVE"),
    //     WM_NCMOUSELEAVE => println!("WM_NCMOUSELEAVE"),
    //     WM_PAINT => println!("WM_PAINT"),
    //     _ => (), // println!("{}", msg),
    // }

    let user_data = {
        let ptr = GetWindowLongPtrW(window, GWL_USERDATA) as *mut UserData;
        if ptr.is_null() {
            match msg {
                WM_NCCREATE => {
                    let createstruct = &mut *(lparam as *mut CREATESTRUCTW);
                    SetWindowLongPtrW(window, GWL_USERDATA, createstruct.lpCreateParams as _);

                    let user_data = createstruct.lpCreateParams as *mut UserData;
                    (*user_data).surface.set(surface);

                    // add window shadow effect
                    let margins = MARGINS {
                        cxLeftWidth: 0,
                        cxRightWidth: 0,
                        cyTopHeight: 1,
                        cyBottomHeight: 0,
                    };
                    DwmExtendFrameIntoClientArea(window, &margins);
                }
                _ => (),
            };

            return DefWindowProcW(window, msg, wparam, lparam);
        }

        &mut *ptr
    };

    match msg {
        WM_NCCALCSIZE => {
            if wparam == false.into() {
                return DefWindowProcW(window, msg, wparam, lparam);
            }

            let params = &mut *(lparam as *mut NCCALCSIZE_PARAMS);

            if surface.is_maximized() {
                // limit to current monitor, otherwise window gets too large
                let monitor = MonitorFromRect(&params.rgrc[0], MONITOR_DEFAULTTONULL);
                let mut monitor_info: MONITORINFOEXW = mem::zeroed();
                monitor_info.monitorInfo.cbSize = mem::size_of::<MONITORINFOEXW>() as u32;
                GetMonitorInfoW(monitor, &mut monitor_info as *mut _ as *mut _);
                params.rgrc[0] = monitor_info.monitorInfo.rcWork;
            }

            // Sync with DWM here giving us maximum amount of time to redraw the screen.
            DwmFlush();
            0
        }

        WM_NCHITTEST => {
            let x = loword(lparam as u32) as i32;
            let y = hiword(lparam as u32) as i32;

            let mut point = POINT { x, y };
            ScreenToClient(window, &mut point);

            let mut area = SurfaceArea::Client;
            user_data.send(Event::Hittest {
                x: point.x,
                y: point.y,
                area: &mut area,
            });

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
                SurfaceArea::Minimize => HTMINBUTTON,
                SurfaceArea::Maximize => HTMAXBUTTON,
                SurfaceArea::Close => HTCLOSE,
            };

            wm_area as LRESULT
        }

        WM_NCLBUTTONDOWN => {
            user_data.keydown_area.set(wparam);
            match wparam as u32 {
                // Prevent windows from drawing ugly legacy buttons on button down..
                // But we manually have to send the SYSCOMMANDs now
                HTMINBUTTON | HTMAXBUTTON | HTCLOSE => 0,
                _ => DefWindowProcW(window, msg, wparam, lparam),
            }
        }

        WM_NCLBUTTONUP => {
            let prev_hit = user_data.keydown_area.get();
            if prev_hit == wparam {
                match wparam as u32 {
                    HTMINBUTTON => {
                        PostMessageW(window, WM_SYSCOMMAND, SC_MINIMIZE as _, lparam);
                    }
                    HTMAXBUTTON => {
                        let action = if surface.is_maximized() {
                            SC_RESTORE
                        } else {
                            SC_MAXIMIZE
                        };
                        PostMessageW(window, WM_SYSCOMMAND, action as _, lparam);
                    }
                    HTCLOSE => {
                        PostMessageW(window, WM_SYSCOMMAND, SC_CLOSE as _, lparam);
                    }
                    _ => {}
                }
            }

            DefWindowProcW(window, msg, wparam, lparam)
        }

        WM_CHAR => {
            if let Some(high_surrogate) = user_data.u16_surrogate.take() {
                let is_low_surrogate = (0xDC00..=0xDFFF).contains(&wparam);
                if is_low_surrogate {
                    if let Some(Ok(c)) = char::decode_utf16([high_surrogate, wparam as u16]).next()
                    {
                        if !c.is_control() {
                            user_data.send(Event::Char(c));
                        }
                    }
                }
            }

            let is_high_surrogate = (0xDC00..=0xDFFF).contains(&wparam);
            if is_high_surrogate {
                user_data.u16_surrogate.set(Some(wparam as u16));
            } else if let Some(c) = char::from_u32(wparam as u32) {
                if !c.is_control() {
                    user_data.send(Event::Char(c));
                }
            }

            0
        }

        WM_KEYDOWN => {
            user_data.on_key(wparam, KeyState::Down);
            0
        }

        WM_KEYUP => {
            user_data.on_key(wparam, KeyState::Up);
            0
        }

        WM_LBUTTONDOWN => {
            user_data.on_button(wparam, MouseButtons::LEFT, KeyState::Down);
            0
        }
        WM_LBUTTONUP => {
            user_data.on_button(wparam, MouseButtons::LEFT, KeyState::Up);
            0
        }
        WM_RBUTTONDOWN => {
            user_data.on_button(wparam, MouseButtons::RIGHT, KeyState::Down);
            0
        }
        WM_RBUTTONUP => {
            user_data.on_button(wparam, MouseButtons::RIGHT, KeyState::Up);
            0
        }

        WM_MOUSEMOVE => {
            let x = loword(lparam as u32) as i16 as i32;
            let y = hiword(lparam as u32) as i16 as i32;

            // Track to get `WM_MOUSELEAVE` events
            TrackMouseEvent(&mut TRACKMOUSEEVENT {
                cbSize: mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: window,
                dwHoverTime: HOVER_DEFAULT,
            });

            user_data.mouse_position.set(Some((x, y)));
            user_data.send(Event::MouseMove);

            0
        }

        WM_NCMOUSEMOVE => {
            match wparam as u32 {
                HTCAPTION | HTMINBUTTON | HTMAXBUTTON | HTCLOSE => {
                    // Track to get `WM_NCMOUSELEAVE` events
                    TrackMouseEvent(&mut TRACKMOUSEEVENT {
                        cbSize: mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE | TME_NONCLIENT,
                        hwndTrack: window,
                        dwHoverTime: HOVER_DEFAULT,
                    });

                    let x = loword(lparam as u32) as i16 as i32;
                    let y = hiword(lparam as u32) as i16 as i32;

                    let mut pt = POINT { x, y };
                    ScreenToClient(window, &mut pt);
                    user_data.mouse_position.set(Some((pt.x, pt.y)));
                }
                _ => {
                    // handled by system, considered out of user area
                    user_data.mouse_position.set(None);
                }
            }
            surface.redraw();

            0
        }

        WM_MOUSELEAVE | WM_NCMOUSELEAVE => {
            user_data.mouse_position.set(None);
            surface.redraw();

            0
        }

        WM_PAINT => {
            user_data.send(Event::Paint);
            ValidateRect(window, ptr::null());

            DefWindowProcW(window, msg, wparam, lparam)
        }
        WM_SIZE => {
            let width = loword(lparam as u32) as u32;
            let height = hiword(lparam as u32) as u32;
            user_data.send(Event::Resize(Extent { width, height }));

            0
        }
        WM_DESTROY => {
            user_data.control_flow.set(ControlFlow::Exit);

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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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

    // Buttons
    Minimize,
    Maximize,
    Close,
}

bitflags::bitflags! {
    pub struct Modifiers: u32 {
        const ALT     = 0b001;
        const CONTROL = 0b010;
        const SHIFT   = 0b100;
    }

    pub struct MouseButtons: u32 {
        const LEFT    = 0b01;
        const RIGHT   = 0b10;
    }
}

impl Modifiers {
    unsafe fn query() -> Self {
        let mut modifiers = Modifiers::empty();
        if GetKeyState(VK_MENU as i32) & 0x80 != 0 {
            modifiers |= Modifiers::ALT;
        }
        if GetKeyState(VK_CONTROL as i32) & 0x80 != 0 {
            modifiers |= Modifiers::CONTROL;
        }
        if GetKeyState(VK_SHIFT as i32) & 0x80 != 0 {
            modifiers |= Modifiers::SHIFT;
        }
        modifiers
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum KeyState {
    Down,
    Up,
}

pub enum Event<'a> {
    Paint,
    Resize(Extent),

    // Should be only used for hotkey like semantics.
    Key {
        key: char,
        state: KeyState,
        modifiers: Modifiers,
    },
    MouseButton {
        button: MouseButtons,
        state: KeyState,
        modifiers: Modifiers,
    },
    MouseMove,

    /// Character input for text processing.
    Char(char),

    /// Window Area hittest.
    Hittest {
        x: i32,
        y: i32,
        area: &'a mut SurfaceArea,
    },
}

struct UserData {
    surface: Cell<Surface>,
    control_flow: Cell<ControlFlow>,
    event_callback: RefCell<Box<dyn FnMut(EventLoop, Event) -> ControlFlow>>,
    mouse_position: Cell<Option<(i32, i32)>>,
    mouse_buttons: Cell<MouseButtons>,
    keydown_area: Cell<WPARAM>,       // WM_NCLBUTTON
    u16_surrogate: Cell<Option<u16>>, // WM_CHAR
}

impl UserData {
    fn send(&self, event: Event) {
        let event_loop = EventLoop {
            surface: self.surface.get(),
            mouse_position: self.mouse_position.get(),
            mouse_buttons: self.mouse_buttons.get(),
        };
        let mut callback = self.event_callback.borrow_mut();
        let control_flow = callback(event_loop, event);
        self.control_flow.set(control_flow);
    }

    fn on_key(&self, wparam: WPARAM, state: KeyState) {
        let c = unsafe { MapVirtualKeyW(wparam as u32, MAPVK_VK_TO_CHAR) };
        if c == 0 {
            return;
        }

        if let Some(key) = char::from_u32(c) {
            let modifiers = unsafe { Modifiers::query() };
            self.send(Event::Key {
                key,
                state,
                modifiers,
            });
        }
    }

    fn on_button(&self, wparam: WPARAM, button: MouseButtons, state: KeyState) {
        let buttons = {
            let mut buttons = MouseButtons::empty();
            let wparam = wparam as u32;
            if wparam & MK_LBUTTON != 0 {
                buttons |= MouseButtons::LEFT;
            }
            if wparam & MK_RBUTTON != 0 {
                buttons |= MouseButtons::RIGHT;
            }
            buttons
        };
        self.mouse_buttons.set(buttons);

        // Mouse capture logic, required that we receive mouse events
        // once we leave the window.
        match (state, buttons.bits().count_ones()) {
            (KeyState::Down, 1) => unsafe {
                SetCapture(self.surface.get().hwnd);
            },
            (KeyState::Up, 0) => unsafe {
                ReleaseCapture();
            },
            _ => (),
        }

        let modifiers = unsafe { Modifiers::query() };

        self.send(Event::MouseButton {
            button,
            state,
            modifiers,
        });
    }
}

pub struct EventLoop {
    pub surface: Surface,
    pub mouse_position: Option<(i32, i32)>,
    pub mouse_buttons: MouseButtons,
}

#[derive(Copy, Clone, Debug)]
pub struct Extent {
    pub width: u32,
    pub height: u32,
}

#[derive(Copy, Clone)]
pub struct Surface {
    hwnd: HWND,
}

impl Surface {
    pub fn show(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_SHOW);
        }
    }

    pub fn hwnd(self) -> HWND {
        self.hwnd
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

    pub fn is_maximized(&self) -> bool {
        unsafe {
            let mut placement: WINDOWPLACEMENT = mem::zeroed();
            placement.length = mem::size_of::<WINDOWPLACEMENT>() as u32;
            GetWindowPlacement(self.hwnd, &mut placement);
            placement.showCmd == SW_MAXIMIZE
        }
    }

    pub fn redraw(&self) {
        unsafe {
            RedrawWindow(self.hwnd, ptr::null(), 0, RDW_INTERNALPAINT);
        }
    }
}
pub struct Platform {
    pub surface: Surface,
    user_data: Rc<UserData>,
}

impl Platform {
    pub fn new() -> Self {
        let user_data = Rc::new(UserData {
            surface: Cell::new(Surface { hwnd: 0 }), // set during WM_NCCREATE
            control_flow: Cell::new(ControlFlow::Continue),
            event_callback: RefCell::new(Box::new(|_, _| ControlFlow::Continue)),
            mouse_position: Cell::new(None),
            mouse_buttons: Cell::new(MouseButtons::empty()),
            keydown_area: Cell::new(HTCLIENT as WPARAM),
            u16_surrogate: Cell::new(None),
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

            // style required to support aero behavior
            let style = WS_SYSMENU | WS_SIZEBOX | WS_CAPTION | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
            let style_ex = WS_EX_APPWINDOW | WS_EX_WINDOWEDGE | WS_EX_ACCEPTFILES;

            let hwnd = {
                let user_data = user_data.clone();
                CreateWindowExW(
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
                    Rc::as_ptr(&user_data) as _,
                )
            };

            let surface = Surface { hwnd };

            Platform { surface, user_data }
        }
    }

    pub fn run<F: FnMut(EventLoop, Event) -> ControlFlow + 'static>(self, callback: F) {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOREPOSITION,
            SWP_NOSIZE, SWP_NOZORDER,
        };

        let _ = self.user_data.event_callback.replace(Box::new(callback));

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

                if let ControlFlow::Exit = self.user_data.control_flow.get() {
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
