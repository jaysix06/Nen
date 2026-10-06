use crate::models::{Reminder, Settings};
use anyhow::{Result, anyhow};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use gpui_kit::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{collections::HashMap, ffi::c_void};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};
use windows::{
    Data::Xml::Dom::XmlDocument,
    Foundation::TypedEventHandler,
    UI::Notifications::*,
    Win32::{
        Foundation::*,
        Graphics::{Dwm::*, Gdi::*},
        System::{Threading::*, WinRT::*},
        UI::{Input::KeyboardAndMouse::ReleaseCapture, Shell::*, WindowsAndMessaging::*},
    },
    core::{HSTRING, Interface, w},
};
use winreg::{RegKey, enums::HKEY_CURRENT_USER};

pub const APP_ID: &str = "dev.still.notes";
pub fn startup_error(error: &str) {
    unsafe {
        let _ = MessageBoxW(
            None,
            &HSTRING::from(format!("Still couldn't start.\n\n{error}")),
            w!("Still"),
            MB_OK | MB_ICONERROR,
        );
    }
}
#[derive(Clone, Debug)]
pub enum PlatformEvent {
    Hotkey(u32),
    Command(String),
    Notification(String),
    Error(String),
    Wake,
    Appearance,
}

pub struct Desktop {
    hotkeys: GlobalHotKeyManager,
    bindings: HashMap<String, HotKey>,
    pub tray: Option<TrayIcon>,
    sender: async_channel::Sender<PlatformEvent>,
    notifier: ToastNotifier,
    // Keep objects alive for their activation handlers, bounded by one day of practical use.
    toasts: Vec<ToastNotification>,
    _activation: super::notification_activation::Registration,
}

impl Desktop {
    pub fn attach(&self, window: &Window) -> Result<()> {
        let Some(hwnd) = hwnd(window) else {
            return Ok(());
        };
        let sender = Box::into_raw(Box::new(self.sender.clone()));
        if !unsafe { SetWindowSubclass(hwnd, Some(window_messages), 0x5354494c, sender as usize) }
            .as_bool()
        {
            unsafe {
                drop(Box::from_raw(sender));
            }
            return Err(anyhow!("Couldn't register Windows lifecycle events."));
        }
        Ok(())
    }
    pub fn new(
        settings: &Settings,
    ) -> Result<(Self, async_channel::Receiver<PlatformEvent>, Vec<String>)> {
        let (sender, receiver) = async_channel::unbounded();
        let mut warnings = Vec::new();
        unsafe {
            let _ = RoInitialize(RO_INIT_SINGLETHREADED);
        }
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey(format!("Software\\Classes\\AppUserModelId\\{APP_ID}"))?
            .0;
        key.set_value("DisplayName", &"Still")?;
        let icon_path = crate::diagnostics::data_directory()?.join("app-icon-v010.png");
        if !icon_path.exists() {
            std::fs::write(&icon_path, include_bytes!("../../assets/still.png"))?;
        }
        key.set_value("IconUri", &icon_path.to_string_lossy().as_ref())?;
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))?;
        unsafe { SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(APP_ID)) }?;
        let activation = super::notification_activation::Registration::new(sender.clone())?;
        let hotkeys = GlobalHotKeyManager::new()?;
        let tx = sender.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state == HotKeyState::Pressed {
                let _ = tx.try_send(PlatformEvent::Hotkey(event.id));
            }
        }));
        let mut desktop = Self {
            hotkeys,
            bindings: HashMap::new(),
            tray: None,
            sender,
            notifier,
            toasts: Vec::new(),
            _activation: activation,
        };
        for action in ["toggle_float", "quick_note", "open_app"] {
            if let Some(binding) = settings.shortcuts.get(action)
                && let Err(e) = desktop.change_hotkey(action, binding)
            {
                warnings.push(e.to_string());
            }
        }
        match desktop.create_tray() {
            Ok(tray) => desktop.tray = Some(tray),
            Err(e) => warnings.push(format!("Couldn't create the tray icon: {e}")),
        }
        Ok((desktop, receiver, warnings))
    }
    fn create_tray(&self) -> Result<TrayIcon> {
        let menu = Menu::new();
        let mut ids = HashMap::new();
        for (label, command) in [
            ("Open Notes", "open_app"),
            ("Quick Note", "quick_note"),
            ("Show Floating Bar", "show_float"),
            ("Settings", "settings"),
            ("Quit", "quit"),
        ] {
            if command == "settings" {
                menu.append(&PredefinedMenuItem::separator())?;
            }
            let item = MenuItem::new(label, true, None);
            ids.insert(item.id().clone(), command.to_owned());
            menu.append(&item)?;
        }
        let tx = self.sender.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if let Some(command) = ids.get(&event.id) {
                let _ = tx.try_send(PlatformEvent::Command(command.clone()));
            }
        }));
        let mut pixels = vec![0u8; 32 * 32 * 4];
        for y in 3..29 {
            for x in 6..26 {
                let i = (y * 32 + x) * 4;
                let line = x == 6
                    || x == 25
                    || y == 3
                    || y == 28
                    || x == 10
                    || (y % 6 == 0 && x > 13 && x < 23);
                if line {
                    pixels[i..i + 4].copy_from_slice(&[166, 190, 160, 255]);
                }
            }
        }
        Ok(TrayIconBuilder::new()
            .with_tooltip("Still")
            .with_menu(Box::new(menu))
            .with_icon(Icon::from_rgba(pixels, 32, 32)?)
            .build()?)
    }
    pub fn action_for_hotkey(&self, id: u32) -> Option<&str> {
        self.bindings
            .iter()
            .find(|(_, key)| key.id() == id)
            .map(|(action, _)| action.as_str())
    }
    pub fn change_hotkey(&mut self, action: &str, binding: &str) -> Result<()> {
        let new = parse_hotkey(binding)?;
        let old = self.bindings.get(action).copied();
        if old == Some(new) {
            return Ok(());
        }
        if let Some(old) = old {
            self.hotkeys.unregister(old)?;
        }
        if let Err(error) = self.hotkeys.register(new) {
            if let Some(old) = old
                && let Err(restore) = self.hotkeys.register(old)
            {
                return Err(anyhow!(
                    "Shortcut unavailable; couldn't restore the previous binding: {restore}"
                ));
            }
            return Err(anyhow!(
                "{} is already in use or unavailable: {error}",
                binding.replace('-', " + ")
            ));
        }
        self.bindings.insert(action.to_owned(), new);
        Ok(())
    }
    pub fn replace_hotkeys(
        &mut self,
        bindings: &std::collections::BTreeMap<String, String>,
    ) -> Result<()> {
        let mut parsed = HashMap::new();
        for action in ["toggle_float", "quick_note", "open_app"] {
            if let Some(binding) = bindings.get(action) {
                parsed.insert(action.to_owned(), parse_hotkey(binding)?);
            }
        }
        let old = self.bindings.clone();
        self.hotkeys
            .unregister_all(&old.values().copied().collect::<Vec<_>>())?;
        let mut registered = Vec::new();
        for key in parsed.values() {
            if let Err(error) = self.hotkeys.register(*key) {
                let _ = self.hotkeys.unregister_all(&registered);
                let restore = self
                    .hotkeys
                    .register_all(&old.values().copied().collect::<Vec<_>>());
                return Err(anyhow!(
                    "Couldn't apply these shortcuts: {error}{}",
                    restore
                        .err()
                        .map(|e| format!(". Couldn't restore the previous bindings: {e}"))
                        .unwrap_or_default()
                ));
            }
            registered.push(*key);
        }
        self.bindings = parsed;
        Ok(())
    }
    pub fn notify(&mut self, reminder: &Reminder, sound: bool) -> Result<()> {
        let setting = self.notifier.Setting()?;
        if setting != NotificationSetting::Enabled {
            return Err(anyhow!(
                if setting == NotificationSetting::DisabledForUser {
                    "Windows notifications are turned off. Enable them in Windows notification settings."
                } else if setting == NotificationSetting::DisabledByGroupPolicy {
                    "Windows notification policy prevents reminders from being displayed."
                } else {
                    "Windows notifications are disabled for Still. Check Windows notification settings."
                }
            ));
        }
        let launch = format!("open:{}", reminder.note_id);
        let xml = format!(
            "<toast launch=\"{}\"><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual><actions><action content=\"Open\" arguments=\"{}\" activationType=\"foreground\"/><action content=\"Snooze\" arguments=\"snooze:{}\" activationType=\"foreground\"/><action content=\"Done\" arguments=\"done:{}\" activationType=\"foreground\"/></actions>{}</toast>",
            escape_xml(&launch),
            escape_xml(&reminder.title),
            escape_xml(&reminder.preview),
            escape_xml(&launch),
            reminder.id,
            reminder.id,
            if sound {
                ""
            } else {
                "<audio silent=\"true\"/>"
            }
        );
        let doc = XmlDocument::new()?;
        doc.LoadXml(&HSTRING::from(xml))?;
        let toast = ToastNotification::CreateToastNotification(&doc)?;
        toast.SetTag(&HSTRING::from(
            reminder.id.chars().take(16).collect::<String>(),
        ))?;
        let tx = self.sender.clone();
        toast.Activated(&TypedEventHandler::new(
            move |_, args: windows::core::Ref<windows::core::IInspectable>| {
                if let Some(args) = args.as_ref()
                    && let Ok(args) = args.cast::<ToastActivatedEventArgs>()
                    && let Ok(arguments) = args.Arguments()
                {
                    let _ = tx.try_send(PlatformEvent::Notification(arguments.to_string()));
                }
                Ok(())
            },
        ))?;
        let tx = self.sender.clone();
        toast.Failed(&TypedEventHandler::new(
            move |_, _: windows::core::Ref<ToastFailedEventArgs>| {
                let _ = tx.try_send(PlatformEvent::Error(
                    "Windows couldn't display a reminder. It is still in Reminders.".into(),
                ));
                Ok(())
            },
        ))?;
        self.notifier.Show(&toast)?;
        self.toasts.push(toast);
        if self.toasts.len() > 128 {
            self.toasts.remove(0);
        }
        Ok(())
    }
}

pub fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn parse_hotkey(binding: &str) -> Result<HotKey> {
    let normalized = binding.replace("⊞", "super-");
    let mut rest = normalized.as_str();
    let mut parts = Vec::new();
    loop {
        let prefix = ["ctrl-", "alt-", "shift-", "super-"]
            .into_iter()
            .find(|prefix| rest.starts_with(prefix));
        if let Some(prefix) = prefix {
            parts.push(prefix.trim_end_matches('-'));
            rest = &rest[prefix.len()..];
        } else {
            break;
        }
    }
    parts.push(match rest {
        "-" => "Minus",
        "+" => "Equal",
        _ => rest,
    });
    Ok(parts.join("+").parse::<HotKey>()?)
}

fn hwnd(window: &Window) -> Option<HWND> {
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut c_void)),
        _ => None,
    }
}
pub fn hide(window: &Window, cx: &gpui_kit::App) {
    if let Some(hwnd) = hwnd(window) {
        cx.foreground_executor()
            .spawn(async move {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            })
            .detach();
    }
}
pub fn show(window: &mut Window, cx: &gpui_kit::App) {
    // GPUI applies its initial placement on first activation. Do that before
    // our visibility request so it cannot overwrite a freshly resized island.
    window.activate_window();
    if let Some(hwnd) = hwnd(window) {
        cx.foreground_executor()
            .spawn(async move {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_SHOW);
                    let _ = SetForegroundWindow(hwnd);
                }
            })
            .detach();
    }
}
pub fn floating_style(window: &Window, settings: &Settings, cx: &gpui_kit::App) {
    if let Some(hwnd) = hwnd(window) {
        let settings = settings.clone();
        cx.foreground_executor()
            .spawn(async move {
                unsafe {
                    let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                    let _ =
                        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_TOOLWINDOW.0 as isize);
                    let _ = SetWindowPos(
                        hwnd,
                        Some(if settings.floating_topmost {
                            HWND_TOPMOST
                        } else {
                            HWND_NOTOPMOST
                        }),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                    if settings.floating_opacity < 0.999 {
                        let _ = SetWindowLongPtrW(
                            hwnd,
                            GWL_EXSTYLE,
                            style | WS_EX_TOOLWINDOW.0 as isize | WS_EX_LAYERED.0 as isize,
                        );
                        let _ = SetLayeredWindowAttributes(
                            hwnd,
                            COLORREF(0),
                            (settings.floating_opacity.clamp(0.6, 1.) * 255.) as u8,
                            LWA_ALPHA,
                        );
                    } else {
                        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                        let _ = SetWindowLongPtrW(
                            hwnd,
                            GWL_EXSTYLE,
                            current & !(WS_EX_LAYERED.0 as isize),
                        );
                    }
                    let radius = 2i32;
                    let _ = DwmSetWindowAttribute(
                        hwnd,
                        DWMWA_WINDOW_CORNER_PREFERENCE,
                        &radius as *const _ as *const c_void,
                        4,
                    );
                }
            })
            .detach();
    }
}
pub fn work_area() -> (i32, i32, i32, i32) {
    unsafe {
        let monitor = MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            (
                info.rcWork.left,
                info.rcWork.top,
                info.rcWork.right,
                info.rcWork.bottom,
            )
        } else {
            (0, 0, 1920, 1080)
        }
    }
}
pub fn position_floating(
    window: &Window,
    width: f32,
    height: f32,
    settings: &Settings,
    anchor: (i32, i32, i32, i32),
    cx: &gpui_kit::App,
) {
    if let Some(hwnd) = hwnd(window) {
        let scale = window.scale_factor();
        let w = (width * scale).round() as i32;
        let h = (height * scale).round() as i32;
        let (l, t, r, b) = anchor;
        let margin = (24. * scale) as i32;
        let (x, y) = match settings.floating_position.as_str() {
            "Top left" => (l + margin, t + margin),
            "Top right" => (r - w - margin, t + margin),
            "Center" => (l + (r - l - w) / 2, t + (b - t - h) / 2),
            "Custom" => (
                settings.floating_x.unwrap_or(l + (r - l - w) / 2),
                settings.floating_y.unwrap_or(t + margin),
            ),
            _ => (l + (r - l - w) / 2, t + margin),
        };
        // Native resize/move messages synchronously call back into GPUI. Run
        // outside its borrowed render/update context, as GPUI's own resize does.
        cx.foreground_executor()
            .spawn(async move {
                unsafe {
                    let _ = SetWindowPos(
                        hwnd,
                        None,
                        x.clamp(l, (r - w).max(l)),
                        y.clamp(t, (b - h).max(t)),
                        w,
                        h,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            })
            .detach();
    }
}
pub fn reduced_motion() -> bool {
    unsafe {
        let mut enabled = 1u32;
        let _ = SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut enabled as *mut _ as *mut c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        enabled == 0
    }
}
pub fn set_startup(enabled: bool) -> Result<()> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")?
        .0;
    if enabled {
        let exe = std::env::current_exe()?;
        key.set_value("Still", &format!("\"{}\" --startup", exe.display()))?;
    } else {
        match key.delete_value("Still") {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

unsafe extern "system" fn window_messages(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    // The sender is owned by this subclass and released exactly once with the HWND.
    let sender = unsafe { &*(data as *const async_channel::Sender<PlatformEvent>) };
    if message == WM_TIMECHANGE || (message == WM_POWERBROADCAST && matches!(wparam.0, 7 | 18)) {
        let _ = sender.try_send(PlatformEvent::Wake);
    }
    if message == WM_SETTINGCHANGE {
        let _ = sender.try_send(PlatformEvent::Appearance);
    }
    if message == WM_QUERYENDSESSION {
        let _ = sender.try_send(PlatformEvent::Command("quit".into()));
    }
    if message == WM_APP + 0x410 {
        let _ = sender.try_send(PlatformEvent::Command(
            if wparam.0 == 1 { "quit" } else { "open_app" }.into(),
        ));
        return LRESULT(0);
    }
    if message == WM_NCDESTROY {
        unsafe {
            let _ = RemoveWindowSubclass(hwnd, Some(window_messages), id);
            drop(Box::from_raw(
                data as *mut async_channel::Sender<PlatformEvent>,
            ));
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
pub fn drag_floating(window: &Window, cx: &gpui_kit::App) -> gpui_kit::Task<Option<(i32, i32)>> {
    let hwnd = hwnd(window);
    // The native move loop dispatches frames and input synchronously. Keep
    // the App and view unborrowed throughout that loop.
    cx.foreground_executor().spawn(async move {
        let hwnd = hwnd?;
        unsafe {
            let _ = ReleaseCapture();
            let mut cursor = POINT::default();
            let _ = GetCursorPos(&mut cursor);
            let position = ((cursor.y as u32 & 0xffff) << 16) | (cursor.x as u32 & 0xffff);
            let _ = SendMessageW(
                hwnd,
                WM_NCLBUTTONDOWN,
                Some(WPARAM(HTCAPTION as usize)),
                Some(LPARAM(position as isize)),
            );
            let mut rect = RECT::default();
            GetWindowRect(hwnd, &mut rect).ok()?;
            Some((rect.left, rect.top))
        }
    })
}

pub struct Instance(HANDLE);
impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
pub fn instance(name: &str) -> Result<Option<Instance>> {
    unsafe {
        let handle = CreateMutexW(None, false, &HSTRING::from(name))?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            if let Ok(hwnd) = FindWindowW(None, w!("Still")) {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
            }
            return Ok(None);
        }
        Ok(Some(Instance(handle)))
    }
}
