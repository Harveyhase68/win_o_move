#![windows_subsystem = "windows"]

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    TrayIconBuilder,
};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    MonitorFromWindow, RedrawWindow, RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME, RDW_INVALIDATE,
    RDW_UPDATENOW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_LEFT, VK_LSHIFT, VK_LWIN, VK_RIGHT, VK_RSHIFT, VK_RWIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetAncestor, GetForegroundWindow, GetMessageW,
    GetWindowLongW, GetWindowRect, GetWindowThreadProcessId, SetWindowPos, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, GA_ROOT, GWL_EXSTYLE, GWL_STYLE, HHOOK,
    KBDLLHOOKSTRUCT, MSG, SWP_FRAMECHANGED, SWP_NOZORDER, WH_KEYBOARD_LL, WM_KEYDOWN, WS_CAPTION,
    WS_EX_TOOLWINDOW,
};
use windows::Win32::System::Threading::GetCurrentProcessId;

thread_local! {
    static HOOK_HANDLE: RefCell<HHOOK> = RefCell::new(HHOOK::default());
}

fn main() {
    println!("WinOMove gestartet - Win+Shift+Left/Right zum Fenster verschieben");
    println!("Tray-Icon zum Beenden verwenden");

    let running = Arc::new(AtomicBool::new(true));

    // Keyboard Hook installieren
    unsafe {
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0)
            .expect("Konnte Keyboard Hook nicht installieren");

        HOOK_HANDLE.with(|h| {
            *h.borrow_mut() = hook;
        });
    }

    // Tray-Icon erstellen
    let menu = Menu::new();
    let quit_item = MenuItem::new("Beenden", true, None);
    menu.append(&quit_item).unwrap();

    let icon = create_simple_icon();

    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("WinOMove - Win+Shift+Arrow zum Fenster verschieben")
        .with_icon(icon)
        .build()
        .unwrap();

    // Message Loop
    unsafe {
        let mut msg = MSG::default();
        while running.load(Ordering::Relaxed) {
            // Menu-Events prüfen
            if let Ok(event) = MenuEvent::receiver().try_recv() {
                if event.id == quit_item.id() {
                    running.store(false, Ordering::Relaxed);
                    break;
                }
            }

            // Windows Messages verarbeiten (non-blocking mit timeout)
            if GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        // Cleanup: Hook entfernen
        HOOK_HANDLE.with(|h| {
            let hook = *h.borrow();
            if !hook.is_invalid() {
                let _ = UnhookWindowsHookEx(hook);
            }
        });
    }

    println!("WinOMove beendet");
}

unsafe extern "system" fn keyboard_hook_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 && wparam.0 as u32 == WM_KEYDOWN {
        let kb_struct = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk_code = kb_struct.vkCode;

        // Prüfen ob Win+Shift gedrückt ist
        let win_pressed =
            GetAsyncKeyState(VK_LWIN.0 as i32) < 0 || GetAsyncKeyState(VK_RWIN.0 as i32) < 0;
        let shift_pressed =
            GetAsyncKeyState(VK_LSHIFT.0 as i32) < 0 || GetAsyncKeyState(VK_RSHIFT.0 as i32) < 0;

        if win_pressed && shift_pressed {
            if vk_code == VK_LEFT.0 as u32 {
                move_window_to_monitor(Direction::Left);
                return LRESULT(1); // Event konsumieren
            } else if vk_code == VK_RIGHT.0 as u32 {
                move_window_to_monitor(Direction::Right);
                return LRESULT(1); // Event konsumieren
            }
        }
    }

    HOOK_HANDLE.with(|h| CallNextHookEx(*h.borrow(), code, wparam, lparam))
}

#[derive(Clone, Copy)]
enum Direction {
    Left,
    Right,
}

/// Prüft ob ein Fenster sicher verschoben werden kann (kein Desktop, keine Shell, etc.)
fn is_movable_window(hwnd: HWND) -> bool {
    unsafe {
        if hwnd.0.is_null() {
            return false;
        }

        // Root-Fenster holen (falls Child-Window)
        let root = GetAncestor(hwnd, GA_ROOT);
        let check_hwnd = if root.0.is_null() { hwnd } else { root };

        // Fenster-Style prüfen
        let style = GetWindowLongW(check_hwnd, GWL_STYLE) as u32;
        let ex_style = GetWindowLongW(check_hwnd, GWL_EXSTYLE) as u32;

        // Muss eine Titelleiste haben (normale Fenster)
        if (style & WS_CAPTION.0) == 0 {
            return false;
        }

        // Tool-Windows ignorieren
        if (ex_style & WS_EX_TOOLWINDOW.0) != 0 {
            return false;
        }

        // Eigenes Fenster nicht verschieben
        let mut window_pid = 0u32;
        GetWindowThreadProcessId(check_hwnd, Some(&mut window_pid));
        if window_pid == GetCurrentProcessId() {
            return false;
        }

        // Fenster muss eine vernünftige Größe haben (nicht Desktop)
        let mut rect = RECT::default();
        if GetWindowRect(check_hwnd, &mut rect).is_err() {
            return false;
        }

        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;

        // Zu kleine Fenster ignorieren
        if width < 100 || height < 50 {
            return false;
        }

        true
    }
}

fn move_window_to_monitor(direction: Direction) {
    unsafe {
        let hwnd = GetForegroundWindow();

        // Sicherheitsprüfung: Nur "normale" Fenster verschieben
        if !is_movable_window(hwnd) {
            return;
        }

        // Aktuelle Fensterposition
        let mut window_rect = RECT::default();
        if GetWindowRect(hwnd, &mut window_rect).is_err() {
            return;
        }

        // Aktueller Monitor
        let current_monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut monitor_info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        if !GetMonitorInfoW(current_monitor, &mut monitor_info).as_bool() {
            return;
        }

        // Fensterposition relativ zum Monitor berechnen
        let window_width = window_rect.right - window_rect.left;
        let window_height = window_rect.bottom - window_rect.top;
        let rel_x = window_rect.left - monitor_info.rcMonitor.left;
        let rel_y = window_rect.top - monitor_info.rcMonitor.top;

        // Zielmonitor finden
        let target_monitor = find_adjacent_monitor(current_monitor, direction);

        let mut target_info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        if !GetMonitorInfoW(target_monitor, &mut target_info).as_bool() {
            return;
        }

        // Wenn gleicher Monitor, nichts tun
        if target_info.rcMonitor == monitor_info.rcMonitor {
            return;
        }

        // Neue Position berechnen (relative Position beibehalten)
        let new_x = target_info.rcMonitor.left + rel_x;
        let new_y = target_info.rcMonitor.top + rel_y;

        // Fenster verschieben
        let _ = SetWindowPos(
            hwnd,
            HWND::default(),
            new_x,
            new_y,
            window_width,
            window_height,
            SWP_NOZORDER | SWP_FRAMECHANGED,
        );

        // Kurz warten damit Windows die Position verarbeiten kann
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Fenster neu zeichnen lassen
        let _ = RedrawWindow(
            hwnd,
            None,
            None,
            RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN | RDW_UPDATENOW,
        );

        // Nochmal kurz warten und ein zweites Mal neu zeichnen (für hartnäckige Fenster)
        std::thread::sleep(std::time::Duration::from_millis(50));
        let _ = RedrawWindow(
            hwnd,
            None,
            None,
            RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN | RDW_UPDATENOW,
        );
    }
}

fn find_adjacent_monitor(current: HMONITOR, direction: Direction) -> HMONITOR {
    let monitors = get_all_monitors();

    if monitors.is_empty() {
        return current;
    }

    let mut current_info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };

    unsafe {
        if !GetMonitorInfoW(current, &mut current_info).as_bool() {
            return current;
        }
    }

    // Monitore mit Info sammeln und nach X-Position sortieren
    let mut monitors_with_info: Vec<(HMONITOR, MONITORINFO)> = monitors
        .iter()
        .filter_map(|&m| {
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            unsafe {
                if GetMonitorInfoW(m, &mut info).as_bool() {
                    Some((m, info))
                } else {
                    None
                }
            }
        })
        .collect();

    monitors_with_info.sort_by_key(|(_, info)| info.rcMonitor.left);

    // Aktuellen Monitor-Index finden
    let current_idx = monitors_with_info
        .iter()
        .position(|(_, info)| info.rcMonitor == current_info.rcMonitor);

    if let Some(idx) = current_idx {
        let target_idx = match direction {
            Direction::Left => {
                if idx > 0 {
                    idx - 1
                } else {
                    idx
                }
            }
            Direction::Right => {
                if idx < monitors_with_info.len() - 1 {
                    idx + 1
                } else {
                    idx
                }
            }
        };

        monitors_with_info[target_idx].0
    } else {
        current
    }
}

fn get_all_monitors() -> Vec<HMONITOR> {
    let mut monitors: Vec<HMONITOR> = Vec::new();

    unsafe extern "system" fn enum_callback(
        monitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> windows::Win32::Foundation::BOOL {
        let monitors = &mut *(data.0 as *mut Vec<HMONITOR>);
        monitors.push(monitor);
        windows::Win32::Foundation::BOOL(1)
    }

    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(enum_callback),
            LPARAM(&mut monitors as *mut _ as isize),
        );
    }

    monitors
}

fn create_simple_icon() -> tray_icon::Icon {
    let size = 16u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;

            // Hintergrund: Blau
            rgba[idx] = 30;
            rgba[idx + 1] = 144;
            rgba[idx + 2] = 255;
            rgba[idx + 3] = 255;

            // Weißer Doppelpfeil (links-rechts)
            // Linker Pfeil <
            if y >= 5 && y <= 10 {
                let arrow_x = (y as i32 - 7).abs();
                if x >= 2 && x <= (4 - arrow_x) as u32 {
                    rgba[idx] = 255;
                    rgba[idx + 1] = 255;
                    rgba[idx + 2] = 255;
                }
            }
            // Rechter Pfeil >
            if y >= 5 && y <= 10 {
                let arrow_x = (y as i32 - 7).abs();
                if x >= (11 + arrow_x) as u32 && x <= 13 {
                    rgba[idx] = 255;
                    rgba[idx + 1] = 255;
                    rgba[idx + 2] = 255;
                }
            }
            // Verbindungslinie
            if y >= 7 && y <= 8 && x >= 4 && x <= 11 {
                rgba[idx] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
            }
        }
    }

    tray_icon::Icon::from_rgba(rgba, size, size).unwrap()
}
