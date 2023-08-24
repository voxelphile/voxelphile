use windows::{Win32::{UI::WindowsAndMessaging::*, Foundation::{HWND, WPARAM, LPARAM, HINSTANCE, LRESULT}}, core::PCSTR};
use std::{mem, ffi::CString, sync::{Arc, Mutex}};

pub unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    DefWindowProcA(hwnd, msg, wparam, lparam)
}

static mut INSTANCE: Option<Window> = None;

pub struct Inner {
    hwnd: HWND
}

#[derive(Clone)]
pub struct Window {
    inner: Arc<Mutex<Inner>>,
}

impl super::WindowInterface for Window {
    fn open() -> Self {
        if unsafe { INSTANCE.is_some() } {
            panic!("A window is already open!");
        }

        let inner = Arc::new(Mutex::new(Inner { hwnd: HWND(0) }));

        let mut wnd_class = unsafe { 
            mem::zeroed::<WNDCLASSA>()
        };

        let class_name = CString::new("XENOTECH").expect("failed to create class name");
        let window_name = CString::new("Xenotech").expect("failed to create class name");

        wnd_class.lpfnWndProc = Some(wnd_proc);
        wnd_class.lpszClassName = PCSTR::from_raw(class_name.as_c_str().as_ptr() as *const _);

        unsafe { RegisterClassA(&wnd_class) };

        let hwnd = unsafe { CreateWindowExA(
            WINDOW_EX_STYLE(0),
            PCSTR::from_raw(class_name.as_c_str().as_ptr() as *const _),
            PCSTR::from_raw(window_name.as_c_str().as_ptr() as *const _),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT,
            HWND(0),
            HMENU(0),
            HINSTANCE(0),
            Some(Arc::as_ptr(&inner) as *const _) 
        ) };

        inner.lock().unwrap().hwnd = hwnd;

        unsafe { ShowWindow(hwnd, SHOW_WINDOW_CMD(5)) };

        unsafe { INSTANCE = Some(Self { inner }) };

        unsafe { INSTANCE.clone().unwrap() }
    }

    fn poll(&self) {
        unsafe { 
            let mut msg = mem::zeroed::<MSG>();
            GetMessageA(&mut msg, self.inner.lock().unwrap().hwnd, 0, 0);
            TranslateMessage(&msg);
            DispatchMessageA(&msg);
        }
    }

    fn close(self) {

    }
}