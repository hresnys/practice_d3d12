
use std::ptr::null;

use windows::{
    Win32::{
        UI::{
            WindowsAndMessaging::*,
            Input::KeyboardAndMouse::SetFocus
        }, 
        Foundation::{HINSTANCE, HWND, LRESULT, WPARAM, LPARAM, RECT}, 
        System::LibraryLoader::GetModuleHandleW, Graphics::Gdi::{GetSysColorBrush, UpdateWindow},
    }, 
    core::{PCWSTR, Error}
};

const CLASS_NAME : &str = "SampleWindowClass";

#[allow(dead_code)]
pub struct App {
    hinstance: HINSTANCE,
    hwnd: HWND,
    width: u32,
    height: u32,
}

impl App {
    pub fn new(width: u32, height: u32) -> Result<Self, Error> {
        unsafe {
            let h_instance = GetModuleHandleW(PCWSTR::default())?;
            let default_icon = HICON(LoadImageW(HINSTANCE::default(),PCWSTR(OIC_SAMPLE as _), IMAGE_ICON, 0, 0, LR_SHARED | LR_DEFAULTCOLOR | LR_DEFAULTSIZE)?.0);
            let wc = WNDCLASSEXW {
                cbSize: core::mem::size_of::<WNDCLASSEXW>() as _,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(Self::wnd_proc),
                hIcon: default_icon,
                hCursor: HCURSOR(LoadImageW(HINSTANCE::default(),PCWSTR(OCR_NORMAL.0 as _), IMAGE_CURSOR, 0, 0, LR_SHARED | LR_DEFAULTCOLOR | LR_DEFAULTSIZE)?.0),
                hbrBackground: GetSysColorBrush(COLOR_BACKGROUND.0 as _),
                lpszMenuName: PCWSTR::default(),
                lpszClassName: PCWSTR(CLASS_NAME.encode_utf16().chain([0]).collect::<Vec<u16>>().as_ptr()),
                hIconSm: default_icon,
                ..Default::default()
            };

            if RegisterClassExW(&wc) == 0 {
                return Err(windows::core::Error::from_win32());
            }

            let mut rc = RECT {
                right: width as _,
                bottom: height as _,
                ..Default::default()
            };

            let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU;
            AdjustWindowRect(&mut rc, style, false);

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0), 
                CLASS_NAME, 
                "Sample", 
                style,
                CW_USEDEFAULT, CW_USEDEFAULT,
                rc.right - rc.left,
                rc.bottom - rc.top,
                None, 
                None, 
                h_instance, 
                null());

            if hwnd.0 == 0 {
                return Err(Error::from_win32());
            }

            ShowWindow(hwnd, SW_SHOWNORMAL);
            UpdateWindow(hwnd);
            SetFocus(hwnd);

            Ok(Self {
                hinstance: h_instance,
                hwnd,
                width,
                height
            })
        }
    }

    pub fn run(self) {
        self.mainloop();
        self.term();
    }

    fn term(self) {
        self.term_wnd()
    }


    fn term_wnd(self) {
        if !self.hinstance.is_invalid() {
            unsafe {
                UnregisterClassW(CLASS_NAME, self.hinstance);
            }
        }
    }

    fn mainloop(&self) {
        let mut msg = MSG::default();
        
        while WM_QUIT != msg.message {
            unsafe {
                if PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool() {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }

    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WM_DESTROY => {
                PostQuitMessage(0);
            },
            _ => {},
        }

        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}