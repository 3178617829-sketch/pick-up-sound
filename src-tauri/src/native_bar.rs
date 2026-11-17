//! 原生 GDI 任务栏播放条。
//!
//! 为什么不继续用 WebView2：贴在任务栏上的 WebView2 窗口在系统面板（快速设置 /
//! 通知中心）打开后会被判定为完全遮挡，Chromium 停止合成，窗口还在、位置也对、
//! 没被 cloak，但画面永久空白。在**那个 WebView2 窗口上**，启动参数、z-order 反复
//! 切换、hide/show、detach/attach、整窗销毁重建都试过，最好也只是"面板关掉后 220ms
//! 回来"。这里换成纯 GDI 绘制 + 分层窗口，不依赖浏览器的合成链路。
//!
//! 宿主模型参考 btsouth/ceiling 的任务栏 widget：窗口**保留 WS_POPUP 样式**再用
//! SetParent 挂到 `Shell_TrayWnd`。换成 `WS_CHILD` 反而会被任务栏的合成绘制盖住
//! （Start / 搜索 / 小组件打开时必现）。
//!
//! ⚠️ 这里的 SetParent 是有效的，和上面 WebView2 时代那些失败尝试不是一回事：
//! 关键在于「挂成任务栏的子窗口」而不是「挂上去之后继续用浏览器合成」。
//! 实测对照：独立顶层窗口在快速设置打开时被盖成一片空白（像素 20,22,44 →
//! 221,221,234），挂进任务栏后不会被盖。

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Emitter, Manager};

use crate::bar_layout::{self, button_hit_radius, button_x, Placement, RectI, BAR_LOGICAL_H};
use crate::main_window::MAIN_WINDOW_LABEL;

use windows_sys::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreatePen,
    CreateRoundRectRgn, CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, Ellipse, EndPaint,
    FillRect, GetDIBits, GetMonitorInfoW, GetStockObject, InvalidateRect, MonitorFromWindow,
    RoundRect, SelectClipRgn, SelectObject, SetBkMode, SetBrushOrgEx, SetStretchBltMode,
    SetTextColor, StretchBlt, UpdateWindow, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    CLEARTYPE_QUALITY, DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, FF_DONTCARE, FW_SEMIBOLD,
    HALFTONE, HBITMAP, HDC, HFONT, HGDIOBJ, MONITORINFO, MONITOR_DEFAULTTONEAREST, NULL_BRUSH,
    PAINTSTRUCT, PS_SOLID, SRCCOPY, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetAncestor, GetClientRect,
    GetWindowRect, IsWindow, LoadCursorW, MsgWaitForMultipleObjects, PeekMessageW, PostMessageW,
    RegisterClassExW, SetLayeredWindowAttributes, SetParent, SetTimer, SetWindowPos, ShowWindow,
    TranslateMessage, CS_HREDRAW, CS_VREDRAW, GA_PARENT, IDC_ARROW, LWA_COLORKEY, MSG, PM_REMOVE,
    QS_ALLINPUT, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SW_SHOWNOACTIVATE, WM_DESTROY, WM_LBUTTONUP,
    WM_PAINT, WM_TIMER, WNDCLASSEXW, WS_CLIPSIBLINGS, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
};

const CLASS_NAME: &str = "PickUpSoundNativeBar";
const WINDOW_TITLE: &str = "Pick Up Sound Bar";
/// 透明色键：洋红，实际绘制不会用到这个颜色。
const TRANSPARENT_KEY: COLORREF = 0x00FF00FF;
/// 重绘周期：进度条要靠它推进。
/// 跨进程窗口操作必须回到 native-bar 线程执行：在 tokio worker 上直接
/// 调 SetWindowPos / ShowWindow 会同步等 Explorer，把整条 command 队列堵死，
/// 前端看到的现象就是「所有 invoke 永不 resolve」。
const WM_APP_SHOW: u32 = 0x8001;
const WM_APP_HIDE: u32 = 0x8002;
const TIMER_ID: usize = 1;
const TIMER_MS: u32 = 100;

/// 条 → 主窗口的控制事件名。前端只监听这一种形状。
pub const CONTROL_EVENT: &str = "bar-control";

static APP: OnceLock<AppHandle> = OnceLock::new();
static HWND_STORE: AtomicIsize = AtomicIsize::new(0);
static VISIBLE: AtomicBool = AtomicBool::new(false);
static STATE: Mutex<BarView> = Mutex::new(BarView::new());
/// 上一次真正应用到窗口上的 (x, y, w, h)。任务栏布局变了（居中 / 托盘宽度 /
/// 分辨率 / DPI）才重设位置，否则每 100ms 一次 SetWindowPos 会把任务栏动画拖抖。
static LAST_PLACE: Mutex<Option<Placement>> = Mutex::new(None);
/// 上一次采样到的任务栏矩形：连续两次相同才算「停稳」，用于避开自动隐藏动画。
static TRAY_LAST: Mutex<Option<RectI>> = Mutex::new(None);

/// 条上要显示的东西，由主窗口通过 `push_bar_state` 推过来。
pub struct BarView {
    title: String,
    uploader: String,
    /// 当前歌词行；第二行优先显示它，空串时退回上传者。
    lyric: String,
    playing: bool,
    progress: f64,
    /// 当前播放位置（秒），用于底部进度线；主窗口没推就是 0。
    position: f64,
    duration: f64,
    /// 三处共用的配色（主窗口 / 小组件 / 任务栏条同一套）：强调色、底色、文字色。
    accent: [u8; 3],
    bar_bg: [u8; 3],
    bar_text: [u8; 3],
}

impl BarView {
    const fn new() -> Self {
        Self {
            title: String::new(),
            uploader: String::new(),
            lyric: String::new(),
            playing: false,
            progress: 0.0,
            position: 0.0,
            duration: 0.0,
            accent: [150, 132, 112],
            bar_bg: [246, 240, 232],
            bar_text: [43, 38, 34],
        }
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

const fn zero_rect() -> RECT {
    RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    }
}

fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

fn mix(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> COLORREF {
    let t = t.clamp(0.0, 1.0);
    let r = a.0 as f32 + (b.0 as f32 - a.0 as f32) * t;
    let g = a.1 as f32 + (b.1 as f32 - a.1 as f32) * t;
    let bl = a.2 as f32 + (b.2 as f32 - a.2 as f32) * t;
    rgb(r as u8, g as u8, bl as u8)
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                paint(hwnd);
                0
            }
            WM_TIMER => {
                // 条挂在任务栏下面，面板打开时由合成层带着任务栏一起画，不会被盖。
                // 但 explorer 重启 / 窗口被 detach 之后要能自己挂回去，所以每个 tick
                // 校验一次「还挂在任务栏上吗」，不对就重挂（判据见 ensure_attached）。
                ensure_attached(hwnd);
                // 任务栏布局会变：图标改居中、托盘宽度变化、分辨率 / DPI 变化。
                // 每次都重算落位，但只在算出来的矩形真的变了才 SetWindowPos。
                apply_placement_if_changed(hwnd);
                // 自动隐藏动画进行中、或任务栏已经缩到屏幕边缘时不重绘，
                // 免得条在滑出滑入的半路上闪。
                if taskbar_settled() {
                    let _ = InvalidateRect(hwnd, std::ptr::null(), 0);
                }
                0
            }
            WM_LBUTTONUP => {
                let x = (lparam & 0xFFFF) as i16 as i32;
                click(hwnd, x);
                0
            }
            WM_APP_SHOW => {
                ensure_attached(hwnd);
                apply_placement(hwnd);
                ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                let _ = InvalidateRect(hwnd, std::ptr::null(), 0);
                0
            }
            WM_APP_HIDE => {
                ShowWindow(hwnd, 0);
                0
            }
            WM_DESTROY => {
                // 只记状态，不发 PostQuitMessage：条现在会被 explorer 重启反复销毁
                // 重建，用退出消息当信号会让队列里残留 WM_QUIT。消息循环靠
                // IsWindow / 父窗口变化自己发现窗口没了，并回外层重建。
                HWND_STORE.store(0, Ordering::SeqCst);
                VISIBLE.store(false, Ordering::SeqCst);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// 应用图标区（ReBarWindow32 / MSTaskSwWClass）的左右边界，物理像素、屏幕绝对坐标。
///
/// Win11 图标居中时这块被推到屏幕中间（实测 624–950），左边空出一整片；
/// 靠左时则紧贴开始按钮右侧（实测 55–382）。条要按它在哪种布局下选边站。
unsafe fn task_list_bounds(tray: HWND) -> Option<(i32, i32)> {
    let mut found: Vec<(i32, i32)> = Vec::new();
    for cls in ["ReBarWindow32", "MSTaskSwWClass"] {
        let c = wide(cls);
        let h = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowExW(
            tray,
            std::ptr::null_mut(),
            c.as_ptr(),
            std::ptr::null(),
        );
        if h.is_null() {
            continue;
        }
        let mut r = zero_rect();
        if windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(h, &mut r) != 0
            && r.right > r.left
        {
            found.push((r.left, r.right));
        }
    }
    bar_layout::merge_list_bounds(&found)
}

/// 把条挂到任务栏右侧、托盘左边。
/// 返回命名字段的目标矩形，不再用 (w, h, x, y) 元组——元组按位置传，改了顺序
/// 编译器一声不响，2026-10-31 就是这么把宽高和坐标传反、条变成 18x3 消失的。
unsafe fn placement(hwnd: HWND) -> Option<Placement> {
    let tray_cls = wide("Shell_TrayWnd");
    let tray = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(
        tray_cls.as_ptr(),
        std::ptr::null(),
    );
    if tray.is_null() {
        return None;
    }
    let tray_rect = taskbar_rect_of(tray)?;
    // 缩放取窗口 DPI，不要用「任务栏高 / 48」反推：任务栏在 150% 缩放下
    // 物理高仍是 48，反推会得到 scale=1，条被画成 480x44 的小条。
    let scale = {
        let dpi =
            windows::Win32::UI::HiDpi::GetDpiForWindow(windows::Win32::Foundation::HWND(hwnd));
        if dpi == 0 {
            1.0
        } else {
            dpi as f32 / 96.0
        }
    };

    // 托盘区左边界：贴它左边留 12 物理像素。
    let notify_cls = wide("TrayNotifyWnd");
    let notify = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowExW(
        tray,
        std::ptr::null_mut(),
        notify_cls.as_ptr(),
        std::ptr::null(),
    );
    let mut right_edge = tray_rect.right;
    if !notify.is_null() {
        let mut nr = zero_rect();
        if windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(notify, &mut nr) == 0
            || nr.right <= nr.left
        {
            // 任务栏刚被重建（explorer 重启 / 切对齐方式），托盘区还没布局完，
            // 这时它的矩形是 0 尺寸——按它算会得到负的 x，条直接跑到屏幕外。
            // 这次不落位，等下一个 tick（100ms 后）再算。
            return None;
        }
        right_edge = nr.left;
    }
    let list_bounds = task_list_bounds(tray);
    // 判断与算尺寸全在 bar_layout::bar_placement（纯函数，有单测）。
    let p = bar_layout::bar_placement(tray_rect, right_edge, list_bounds, scale)?;
    let _ = hwnd;
    Some(p)
}

/// 强制按当前任务栏布局重新落位（不管上次算出来的是多少）。
///
/// WM_APP_SHOW / 刚挂上任务栏时用：这两种情况下窗口可能被人为挪过，缓存的
/// LAST_PLACE 已经不可信。
unsafe fn apply_placement(hwnd: HWND) {
    invalidate_placement();
    apply_placement_if_changed(hwnd);
}

/// 只在落位矩形真的变了才 SetWindowPos。
///
/// 每 100ms 一次的 tick 都要跟着任务栏布局走（居中 / 托盘宽度 / 分辨率 / DPI），
/// 但无条件 SetWindowPos 会让任务栏的自动隐藏动画一直被打断，所以先比一遍。
unsafe fn apply_placement_if_changed(hwnd: HWND) {
    let Some(next) = placement(hwnd) else {
        return;
    };
    if let Ok(guard) = LAST_PLACE.lock() {
        if guard.as_ref() == Some(&next) {
            return;
        }
    }
    // insertAfter 传 0：条是任务栏的子窗口，z 序由父窗口决定，不需要也不该自己
    // 指定 HWND_TOPMOST / HWND_TOP。ceiling 的通过审核版本就是这么调的
    // （ref-ceiling-taskbar_widget.rs:1243 `SetWindowPos(hwnd, 0, x, y, w, h,
    // SWP_NOACTIVATE | SWP_NOOWNERZORDER)`）。
    SetWindowPos(
        hwnd,
        std::ptr::null_mut(),
        next.x,
        next.y,
        next.w,
        next.h,
        SWP_NOACTIVATE | SWP_NOOWNERZORDER,
    );
    if let Ok(mut guard) = LAST_PLACE.lock() {
        *guard = Some(next);
    }
}

fn invalidate_placement() {
    if let Ok(mut guard) = LAST_PLACE.lock() {
        *guard = None;
    }
}

/// 任务栏矩形（物理像素）。拿不到或尺寸无效返回 None。
unsafe fn taskbar_rect_of(tray: HWND) -> Option<RectI> {
    let mut tb = zero_rect();
    if GetWindowRect(tray, &mut tb) == 0 {
        return None;
    }
    let rect = RectI::new(tb.left, tb.top, tb.right, tb.bottom);
    rect.is_valid().then_some(rect)
}

/// 任务栏「可见且停稳」才返回 true。判据照 AF Media Bar 的 TaskbarMotionPolicy：
///
/// - 任务栏与所在显示器在交叉轴上可见的交集 ≤ 4 物理像素 → 已经滑到屏幕外
///   （自动隐藏状态），此时不该重绘，也不该按它的位置重新落位；
/// - 连续两次采样矩形完全一样才算动画停稳，避免在隐藏 / 显示动画的半路上重绘。
///
/// 参考：AF-Media-Bar/TaskbarMotionPolicy.cs（RequiredStableSamples = 2、
/// HiddenEdgePixels = 4），以及 TaskbarWindow.xaml.cs:445 的 ObserveTaskbarMotion。
unsafe fn taskbar_settled() -> bool {
    let tray_cls = wide("Shell_TrayWnd");
    let tray = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(
        tray_cls.as_ptr(),
        std::ptr::null(),
    );
    if tray.is_null() {
        return false;
    }
    let Some(cur) = taskbar_rect_of(tray) else {
        return false;
    };
    let monitor = MonitorFromWindow(tray, MONITOR_DEFAULTTONEAREST);
    if monitor.is_null() {
        return false;
    }
    let mut info: MONITORINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(monitor, &mut info) == 0 {
        return false;
    }
    let mon = info.rcMonitor;
    let mon_rect = RectI::new(mon.left, mon.top, mon.right, mon.bottom);
    let last = TRAY_LAST.lock().ok().and_then(|guard| *guard);
    let step = bar_layout::settle_step(cur, mon_rect, last);
    // 隐藏时 next_last 是 None（清掉基线，等它重新露出来）；停稳 / 变化时写回当前矩形。
    if let Ok(mut guard) = TRAY_LAST.lock() {
        *guard = step.next_last;
    }
    step.settled
}

/// 重新挂回任务栏。
///
/// ceiling 的注释（ref-ceiling-taskbar_widget.rs:1440-1443）写的是：
/// 「保留 popup 样式挂到 explorer 上。这是通过审核的任务栏构建用的挂载模型：
/// 它保持不激活，同时避免合成任务栏在开始菜单 / 搜索 / 小组件打开时盖住普通子窗口」。
/// 也就是说：条要挂在任务栏下面，面板打开时才不会被合成层盖掉。
unsafe fn ensure_attached(hwnd: HWND) {
    let tray_cls = wide("Shell_TrayWnd");
    let tray = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(
        tray_cls.as_ptr(),
        std::ptr::null(),
    );
    if tray.is_null() {
        return;
    }
    // 判据必须用 GetAncestor(GA_PARENT)，不能用 GetParent()：跨进程挂到 Explorer 的
    // 子窗口上，GetParent() 会返回 0（把任务栏当 owner 而不是 parent），恒判成「没挂上」，
    // 于是每个 tick 都重挂一次——任务栏被拖慢，主线程的 invoke 看起来永远不返回。
    let parent = GetAncestor(hwnd, GA_PARENT);
    if parent == tray {
        return;
    }
    // explorer 重启过 / 一开始就没挂上：重新挂
    SetParent(hwnd, tray);
    let _ = SetLayeredWindowAttributes(hwnd, TRANSPARENT_KEY, 255, LWA_COLORKEY);
    apply_placement(hwnd);
    // 重挂不等于「用户想让它出现」：用户关掉的条挂回去也得继续藏着
    if VISIBLE.load(Ordering::SeqCst) {
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
}

/// 注册窗口类：整个线程只注册一次，重建条窗口的时候不再重复注册。
unsafe fn register_class() {
    let instance = GetModuleHandleW(std::ptr::null());
    let class = wide(CLASS_NAME);
    let mut wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: std::ptr::null_mut(),
        hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: class.as_ptr(),
        hIconSm: std::ptr::null_mut(),
    };
    RegisterClassExW(&mut wc);
}

/// 在给定任务栏下面建出条窗口并挂上去。
unsafe fn create_bar(tray: HWND) -> HWND {
    let instance = GetModuleHandleW(std::ptr::null());
    let class = wide(CLASS_NAME);
    let title = wide(WINDOW_TITLE);

    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_NOACTIVATE,
        class.as_ptr(),
        title.as_ptr(),
        WS_POPUP | WS_VISIBLE | WS_CLIPSIBLINGS,
        // 初始位置放任务栏最左边、1x1 大小（ceiling 的做法：先建在左上角再移到右边）。
        0,
        0,
        1,
        1,
        // parent 直接传任务栏：条成为 Shell_TrayWnd 的子窗口。
        // ceiling 的注释（ref-ceiling-taskbar_widget.rs:1440-1443）说得很清楚：
        // 保留 popup 样式挂到 explorer 上，这是通过审核的任务栏挂载模型——它保持
        // 不激活，同时避免合成任务栏在开始菜单 / 搜索 / 小组件打开时盖住普通子窗口。
        // 实测对比：独立顶层窗口在快速设置打开时被盖成一片空白（像素 20,22,44 →
        // 221,221,234），挂进任务栏后不会被盖。
        tray,
        std::ptr::null_mut(),
        instance,
        std::ptr::null_mut(),
    );
    if hwnd.is_null() {
        return hwnd;
    }
    // CreateWindowExW 的 parent 已经传了 tray，这里再显式确认一次（ceiling 同款）。
    SetParent(hwnd, tray);
    let attached = GetAncestor(hwnd, GA_PARENT) == tray;
    if !attached {
        // 用 GetParent() 判定会误判（跨进程子窗口返回 0），所以这里用 GetAncestor。
        // 即使真挂不上也不销毁窗口：至少还能当独立顶层窗口显示，不至于整个条消失。
        eprintln!("[native-bar] attach to Shell_TrayWnd failed, staying top-level");
    }
    let _ = SetLayeredWindowAttributes(hwnd, TRANSPARENT_KEY, 255, LWA_COLORKEY);
    HWND_STORE.store(hwnd as isize, Ordering::SeqCst);
    invalidate_placement();
    apply_placement_if_changed(hwnd);
    // 重建时尊重用户上次的选择：VISIBLE 是「用户想不想看条」的意图位。
    // 以前这里无条件 ShowWindow + VISIBLE.store(true)，explorer 一重启，已经关掉的条
    // 就自己冒回来——用户关条这个动作等于白做。现在只按意图位决定，不写它。
    let visible = VISIBLE.load(Ordering::SeqCst);
    if visible {
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = UpdateWindow(hwnd);
    } else {
        // WS_VISIBLE 是创建样式，窗口一建出来就露脸；用户关过条，建完立刻藏回去
        ShowWindow(hwnd, 0);
    }
    SetTimer(hwnd, TIMER_ID, TIMER_MS, None);
    hwnd
}

/// 任务栏托盘区是否已经完成布局（矩形非零）。
///
/// explorer 重启后 Shell_TrayWnd 会先出现，但托盘区还要过一段时间才有真实尺寸；
/// 这中间建窗会算不出位置（见 placement 里的同一判断）。
unsafe fn notify_ready(tray: HWND) -> bool {
    let cls = wide("TrayNotifyWnd");
    let notify = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowExW(
        tray,
        std::ptr::null_mut(),
        cls.as_ptr(),
        std::ptr::null(),
    );
    if notify.is_null() {
        return false;
    }
    let mut nr = zero_rect();
    if windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(notify, &mut nr) == 0 {
        return false;
    }
    nr.right > nr.left && nr.bottom > nr.top
}

/// 等任务栏出现并停稳再建条。
///
/// explorer 重启后 Shell_TrayWnd 会先出现、再被反复改写尺寸（切对齐方式时尤其
/// 明显）。抢在它还没定下来的时候建窗，条会落在错的位置——实测居中时偏 32px。
unsafe fn wait_taskbar_stable() -> HWND {
    let tray_cls = wide("Shell_TrayWnd");
    let mut last: Option<RectI> = None;
    // 最多等 120 秒；等不到就返回空，外层下一轮再来。
    for _ in 0..240 {
        let tray = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(
            tray_cls.as_ptr(),
            std::ptr::null(),
        );
        if tray.is_null() {
            last = None;
        } else if let Some(cur) = taskbar_rect_of(tray) {
            // 托盘区还没布局完也算「还没稳」：保留采样值，等它好了立刻返回。
            if last == Some(cur) && notify_ready(tray) {
                return tray;
            }
            last = Some(cur);
        } else {
            last = None;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    std::ptr::null_mut()
}

/// 内层消息循环。返回 = 条窗口已经没了，或者挂着的任务栏换了，需要外层重建。
unsafe fn run_message_loop(hwnd: HWND) {
    let tray_cls = wide("Shell_TrayWnd");
    let mut msg: MSG = std::mem::zeroed();
    loop {
        // 带超时地等消息：跨进程子窗口在 explorer 重启时不一定收得到 WM_DESTROY，
        // 只靠 GetMessageW 会永远卡在这里——旧版就是这样，explorer 一重启条再也不回来。
        let r = MsgWaitForMultipleObjects(0, std::ptr::null(), 0, 500, QS_ALLINPUT);
        if r == 0 {
            // WAIT_OBJECT_0：有消息，先排空再回去体检。
            loop {
                if PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) == 0 {
                    break;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if IsWindow(hwnd) == 0 {
            return;
        }
        let cur = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(
            tray_cls.as_ptr(),
            std::ptr::null(),
        );
        if cur.is_null() || GetAncestor(hwnd, GA_PARENT) != cur {
            // 任务栏换了（explorer 重启）：旧条挂在已经死掉的父窗口下面，销毁重建。
            if IsWindow(hwnd) != 0 {
                DestroyWindow(hwnd);
            }
            return;
        }
    }
}

unsafe fn handle_window() {
    register_class();
    // 外层重建循环：explorer 重启（切换任务栏对齐方式、重启资源管理器）会把挂在
    // 任务栏下面的条一起销毁，而跨进程子窗口收不到可靠的 WM_DESTROY。消息循环里
    // 每 500ms 体检一次，发现条没了 / 任务栏换了就回到这里，等任务栏重新停稳再建。
    loop {
        let tray = wait_taskbar_stable();
        if tray.is_null() {
            continue;
        }
        let hwnd = create_bar(tray);
        if hwnd.is_null() {
            std::thread::sleep(std::time::Duration::from_millis(1000));
            continue;
        }
        run_message_loop(hwnd);
    }
}

pub fn install(app: AppHandle) {
    let _ = APP.set(app);
    std::thread::Builder::new()
        .name("native-bar".to_owned())
        .spawn(|| unsafe { handle_window() })
        .ok();
}

/// 主窗口推状态进来（和 WebView2 版的 `push_bar_state` 一一对应）。
pub fn update(
    title: String,
    uploader: String,
    lyric: String,
    playing: bool,
    progress: f64,
    accent: String,
    bar_bg: String,
    bar_text: String,
) {
    if let Ok(mut state) = STATE.lock() {
        state.title = title;
        state.uploader = uploader;
        state.lyric = lyric;
        state.playing = playing;
        state.progress = progress;
        if let Some(c) = parse_rgb(&accent) {
            state.accent = c;
        }
        if let Some(c) = parse_rgb(&bar_bg) {
            state.bar_bg = c;
        }
        if let Some(c) = parse_rgb(&bar_text) {
            state.bar_text = c;
        }
    }
    repaint();
}

/// 解析 "r,g,b" 形式的颜色串；格式不对返回 None，调用方保留旧值。
fn parse_rgb(value: &str) -> Option<[u8; 3]> {
    let parts: Vec<u8> = value
        .split(',')
        .filter_map(|x| x.trim().parse::<u8>().ok())
        .collect();
    if parts.len() == 3 {
        Some([parts[0], parts[1], parts[2]])
    } else {
        None
    }
}

/// 把条上的按钮动作转给主窗口。
///
/// 以前 `click` 和 `bar_commands::bar_control` 各拼一份事件名 + payload，改一处忘
/// 另一处就是静默断线（点按钮没反应，日志也不报）；现在只有这一个出口。
pub fn forward_control(action: &str) -> Result<(), String> {
    let app = APP
        .get()
        .ok_or_else(|| "application is not ready".to_owned())?;
    let main = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "main window is unavailable".to_owned())?;
    main.emit(CONTROL_EVENT, serde_json::json!({ "action": action }))
        .map_err(|error| error.to_string())
}

pub fn show() {
    let raw = HWND_STORE.load(Ordering::SeqCst);
    VISIBLE.store(true, Ordering::SeqCst);
    if raw == 0 {
        return;
    }
    unsafe {
        PostMessageW(raw as HWND, WM_APP_SHOW, 0, 0);
    }
}

pub fn hide() {
    let raw = HWND_STORE.load(Ordering::SeqCst);
    VISIBLE.store(false, Ordering::SeqCst);
    if raw != 0 {
        unsafe {
            PostMessageW(raw as HWND, WM_APP_HIDE, 0, 0);
        }
    }
}

pub fn visible() -> bool {
    VISIBLE.load(Ordering::SeqCst)
}

fn repaint() {
    let raw = HWND_STORE.load(Ordering::SeqCst);
    if raw != 0 {
        unsafe {
            let _ = InvalidateRect(raw as HWND, std::ptr::null(), 0);
        }
    }
}

// ---------------------------------------------------------------- 绘制

unsafe fn paint(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    let mut rc = zero_rect();
    GetClientRect(hwnd, &mut rc);
    let w = rc.right - rc.left;
    let h = rc.bottom - rc.top;
    if w > 0 && h > 0 {
        // 4 倍超采样：先按 4 倍尺寸画，再用 HALFTONE 缩回。GDI 本身不做抗锯齿，
        // 缩放这一步等效于给文字、圆角和封面边缘补上抗锯齿。
        const SS: i32 = 4;
        let bw = w * SS;
        let bh = h * SS;

        let big = CreateCompatibleDC(hdc);
        let big_bmp: HBITMAP = CreateCompatibleBitmap(hdc, bw, bh);
        let big_old: HGDIOBJ = SelectObject(big, big_bmp);
        draw(big, bw, bh);
        // 调试：直接把 4 倍位图存出来——draw() 就画在这上面，是最接近真凭实据的一帧。
        // GetDIBits 要求位图没被选进 DC，所以 dump 期间临时取消选中，dump 完立刻选回来。
        // ⚠️ 千万别在这里永久取消选中：下面 StretchBlt 的源是 DC 里**当前选中**的位图，
        // 少了它就会被换回原始 1x1 位图去读，整条画成纯黑——这就是「条在任务栏位置上
        // 但显示一块黑」的真正原因（dump 出来却是彩色的，因为 dump 发生在换出之前）。
        if std::env::var("BILI_BAR_DUMP").is_ok() {
            SelectObject(big, big_old);
            dump_bmp(big, big_bmp, bw, bh);
            SelectObject(big, big_bmp);
        }

        let mem = CreateCompatibleDC(hdc);
        let bmp: HBITMAP = CreateCompatibleBitmap(hdc, w, h);
        let old: HGDIOBJ = SelectObject(mem, bmp);
        SetStretchBltMode(mem, HALFTONE);
        let _ = SetBrushOrgEx(mem, 0, 0, std::ptr::null_mut());
        let _ = StretchBlt(mem, 0, 0, w, h, big, 0, 0, bw, bh, SRCCOPY);
        let _ = BitBlt(hdc, 0, 0, w, h, mem, 0, 0, SRCCOPY);
        SelectObject(mem, old);
        // big_bmp 到这一刻才换出：StretchBlt 已经读完了。
        SelectObject(big, big_old);
        let _ = DeleteObject(bmp);
        let _ = DeleteDC(mem);
        let _ = DeleteObject(big_bmp);
        let _ = DeleteDC(big);
    }
    let _ = EndPaint(hwnd, &ps);
}

/// 调试用：把内存位图存成 24 位 BMP。设 BILI_BAR_DUMP=1 时每次重绘都覆盖写一次。
/// 分层窗口（WS_EX_LAYERED）的内容在 DWM 的离屏表面里，PrintWindow 抓出来是全黑，
/// 只有从我们自己画的这块位图上取才准。
/// 产物落在系统临时目录——这是一次性排查转储，不是用户数据，不该进应用数据目录。
fn dump_bmp(mem: HDC, bmp: HBITMAP, w: i32, h: i32) {
    use std::io::Write;

    let stride = ((w * 3 + 3) / 4) * 4;
    let size = (stride * h) as usize;
    let mut pixels = vec![0u8; size];

    let mut bi: BITMAPINFO = unsafe { std::mem::zeroed() };
    bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bi.bmiHeader.biWidth = w;
    bi.bmiHeader.biHeight = h;
    bi.bmiHeader.biPlanes = 1;
    bi.bmiHeader.biBitCount = 24;
    bi.bmiHeader.biCompression = BI_RGB;
    bi.bmiHeader.biSizeImage = size as u32;

    let got = unsafe {
        GetDIBits(
            mem,
            bmp,
            0,
            h as u32,
            pixels.as_mut_ptr() as *mut std::ffi::c_void,
            &mut bi,
            DIB_RGB_COLORS,
        )
    };
    if got == 0 {
        return;
    }

    let mut header = [0u8; 54];
    header[0] = b'B';
    header[1] = b'M';
    header[2..6].copy_from_slice(&((54 + size) as u32).to_le_bytes());
    header[10..14].copy_from_slice(&54u32.to_le_bytes());
    header[14..18].copy_from_slice(&40u32.to_le_bytes());
    header[18..22].copy_from_slice(&w.to_le_bytes());
    header[22..26].copy_from_slice(&h.to_le_bytes());
    header[26..28].copy_from_slice(&1u16.to_le_bytes());
    header[28..30].copy_from_slice(&24u16.to_le_bytes());
    header[34..38].copy_from_slice(&(size as u32).to_le_bytes());

    let path = std::env::temp_dir().join("bfm-bar-dump.bmp");
    if let Ok(mut f) = std::fs::File::create(&path) {
        let _ = f.write_all(&header);
        let _ = f.write_all(&pixels);
    }
}

/// 任务栏条的整幅绘制：底板、封面圆盘、双行文字、右侧三个按钮都在这里画完。
/// 调用方 `draw()` 只负责准备 DC 和缓冲区。
///
/// - 尺寸全部从 `h`（条的物理高度）按 `BAR_LOGICAL_H`（逻辑高度 44）折算，
///   所以任务栏变高、DPI 缩放变化时不用改这里；
/// - 配色只从 `STATE` 取 `accent` / `bar_bg` / `bar_text` 三个值，
///   它们由前端按当前 palette 推过来（主窗口 / 小组件 / 任务栏条共用一套配色）。
///
/// 函数 260 多行是因为 GDI 是「建对象 → 选进 DC → 画 → 还原」的线性流程，
/// 拆成小函数要在好几处之间传递 HDC / HBRUSH / HPEN / RECT，反而更难读，所以保持一整块。
unsafe fn draw_paper(dc: HDC, w: i32, h: i32) {
    let s = h as f32 / BAR_LOGICAL_H; // 每逻辑像素对应的物理像素
    let cy = h / 2;

    // 三处共用配色：主窗口 / 小组件选了哪套 palette，任务栏条就跟哪套。
    let (accent, bar_bg, bar_text) = STATE
        .lock()
        .map(|st| (st.accent, st.bar_bg, st.bar_text))
        .unwrap_or(([150, 132, 112], [246, 240, 232], [43, 38, 34]));
    let bg_rgb = (bar_bg[0], bar_bg[1], bar_bg[2]);
    let fg_rgb = (bar_text[0], bar_text[1], bar_text[2]);
    let accent_rgb = (accent[0], accent[1], accent[2]);
    let muted = mix(fg_rgb, bg_rgb, 0.42);
    let edge_rgb = mix(bg_rgb, fg_rgb, 0.12);

    // 透明色键铺满，圆角外的四角透出去
    let key_brush = CreateSolidBrush(TRANSPARENT_KEY);
    let full = RECT {
        left: 0,
        top: 0,
        right: w,
        bottom: h,
    };
    FillRect(dc, &full, key_brush);
    let _ = DeleteObject(key_brush);

    let radius = ((26.0 * s) as i32).min(h / 2);
    let region = CreateRoundRectRgn(0, 0, w + 1, h + 1, radius, radius);
    SelectClipRgn(dc, region);

    // 底色：从左上轻微提亮到右下压暗，做成一道柔和的竖向渐变。
    // GDI 解析不了 CSS 的 linear-gradient，这里用「同色相的明暗变化」近似，
    // 效果是卡片/条看起来有光，而不是一整块死板的纯色。
    let base = (bar_bg[0], bar_bg[1], bar_bg[2]);
    let toward_white = |t: f32| -> (u8, u8, u8) {
        (
            (base.0 as f32 + (255.0 - base.0 as f32) * t) as u8,
            (base.1 as f32 + (255.0 - base.1 as f32) * t) as u8,
            (base.2 as f32 + (255.0 - base.2 as f32) * t) as u8,
        )
    };
    let c_top = toward_white(0.30);
    let c_bottom = (
        base.0.saturating_sub(34),
        base.1.saturating_sub(34),
        base.2.saturating_sub(34),
    );
    let rows = h.max(2);
    for y in 0..rows {
        let t = y as f32 / (rows - 1) as f32;
        let brush = CreateSolidBrush(mix(c_top, c_bottom, t));
        let strip = RECT {
            left: 0,
            top: y,
            right: w,
            bottom: y + 1,
        };
        FillRect(dc, &strip, brush);
        let _ = DeleteObject(brush);
    }

    let (prev_x, play_x, next_x) = button_x(w, h);

    // 圆形封面：黑盘 + 一圈暗金边 + 金色标签 + 中心孔。
    // 用户定稿的「③ 黑盘 + 金色标签」：盘身固定用近黑，靠暗金边把黑盘从深色底板里
    // 拉出来，标签用强调色。黑色主题、白色主题都用同一套画法。
    let cover_r = (13.0 * s) as i32;
    let cover_cx = (8.0 * s) as i32 + cover_r;
    let disc_ink: COLORREF = rgb(11, 9, 8);
    draw_circle(dc, cover_cx, cy, cover_r, mix(accent_rgb, bg_rgb, 0.55));
    draw_circle(
        dc,
        cover_cx,
        cy,
        (cover_r as f32 * 0.92) as i32,
        disc_ink,
    );
    draw_circle(
        dc,
        cover_cx,
        cy,
        (cover_r as f32 * 0.46) as i32,
        rgb(accent_rgb.0, accent_rgb.1, accent_rgb.2),
    );
    draw_circle(
        dc,
        cover_cx,
        cy,
        ((cover_r as f32 * 0.10) as i32).max(1),
        disc_ink,
    );

    // 状态快照
    let (title, sub, playing, progress) = match STATE.lock() {
        Ok(st) => {
            let p = if st.duration > 0.0 {
                (st.position / st.duration).clamp(0.0, 1.0)
            } else {
                st.progress.clamp(0.0, 1.0)
            };
            let sub = if st.lyric.is_empty() {
                st.uploader.clone()
            } else {
                st.lyric.clone()
            };
            (st.title.clone(), sub, st.playing, p)
        }
        Err(_) => (String::new(), String::new(), false, 0.0),
    };
    let title_text = if title.is_empty() {
        "尚未播放".to_owned()
    } else {
        title
    };
    let sub_text = if sub.is_empty() {
        "—".to_owned()
    } else {
        sub
    };

    // 两行文字
    let text_left = cover_cx + cover_r + (10.0 * s) as i32;
    let text_right = prev_x - (12.0 * s) as i32;

    let f_title = CreateFontW(
        -((13.0 * s * 1.30) as i32),
        0,
        0,
        0,
        FW_SEMIBOLD as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        0,
        0,
        CLEARTYPE_QUALITY as u32,
        (DEFAULT_PITCH | FF_DONTCARE) as u32,
        wide("Microsoft YaHei UI").as_ptr(),
    );
    let f_sub = CreateFontW(
        -((10.0 * s * 1.30) as i32),
        0,
        0,
        0,
        400,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        0,
        0,
        CLEARTYPE_QUALITY as u32,
        (DEFAULT_PITCH | FF_DONTCARE) as u32,
        wide("Microsoft YaHei UI").as_ptr(),
    );

    SetBkMode(dc, TRANSPARENT as i32);
    // DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX
    let flags = 0x0000_0004u32 | 0x0000_0020u32 | 0x0000_8000u32 | 0x0000_0800u32;

    let old_font = SelectObject(dc, f_title);
    SetTextColor(dc, rgb(bar_text[0], bar_text[1], bar_text[2]));
    let mut tr = RECT {
        left: text_left,
        top: (1.0 * s) as i32,
        right: text_right,
        bottom: cy,
    };
    let mut wt: Vec<u16> = title_text.encode_utf16().collect();
    DrawTextW(dc, wt.as_mut_ptr(), wt.len() as i32, &mut tr, flags);

    SelectObject(dc, f_sub);
    SetTextColor(dc, muted);
    let mut sr = RECT {
        left: text_left,
        top: cy - (1.0 * s) as i32,
        right: text_right,
        bottom: h - (1.0 * s) as i32,
    };
    let mut ws: Vec<u16> = sub_text.encode_utf16().collect();
    DrawTextW(dc, ws.as_mut_ptr(), ws.len() as i32, &mut sr, flags);

    SelectObject(dc, old_font);
    let _ = DeleteObject(f_title);
    let _ = DeleteObject(f_sub);

    // 三个按钮：用 Segoe MDL2 Assets 字形画，带 ClearType 抗锯齿，比 Polygon 平滑得多
    let dark = mix(fg_rgb, bg_rgb, 0.10);

    let mk_icon_logical = |logical: f32| -> i32 { -((logical * s * 1.22) as i32) };
    let f_icon = CreateFontW(
        mk_icon_logical(14.0),
        0,
        0,
        0,
        400,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        0,
        0,
        CLEARTYPE_QUALITY as u32,
        0,
        wide("Segoe MDL2 Assets").as_ptr(),
    );
    let f_icon_main = CreateFontW(
        mk_icon_logical(15.0),
        0,
        0,
        0,
        400,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        0,
        0,
        CLEARTYPE_QUALITY as u32,
        0,
        wide("Segoe MDL2 Assets").as_ptr(),
    );

    // 上一首 E892 / 下一首 E893
    draw_glyph(dc, f_icon, 0xE892, prev_x, cy, muted, (10.0 * s) as i32);
    draw_glyph(dc, f_icon, 0xE893, next_x, cy, muted, (10.0 * s) as i32);

    // 主按钮：28 逻辑实心强调色圆 + 播放 E768 / 暂停 E769
    let play_r = (14.0 * s) as i32;
    draw_circle(dc, play_x, cy, play_r, rgb(accent[0], accent[1], accent[2]));
    let glyph = if playing { 0xE769 } else { 0xE768 };
    draw_glyph(dc, f_icon_main, glyph, play_x, cy, dark, (11.0 * s) as i32);

    let _ = DeleteObject(f_icon);
    let _ = DeleteObject(f_icon_main);
    let _ = playing;

    // 底部进度线（1 逻辑像素）
    if progress > 0.0 {
        let line_h = (1.0 * s) as i32;
        fill_rect(
            dc,
            0,
            h - line_h,
            (w as f64 * progress) as i32,
            line_h,
            rgb(accent[0], accent[1], accent[2]),
        );
    }

    // 1px 描边：浅底配深边、深底配浅边，否则和任务栏糊成一片
    let edge_pen = CreatePen(PS_SOLID as i32, 1, edge_rgb);
    let old_pen = SelectObject(dc, edge_pen);
    let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
    RoundRect(dc, 0, 0, w, h, radius, radius);
    SelectObject(dc, old_brush);
    SelectObject(dc, old_pen);
    let _ = DeleteObject(edge_pen);

    SelectClipRgn(dc, std::ptr::null_mut());
    let _ = DeleteObject(region);
}

unsafe fn draw(dc: HDC, w: i32, h: i32) {
    draw_paper(dc, w, h);
}

unsafe fn fill_rect(dc: HDC, x: i32, y: i32, w: i32, h: i32, color: COLORREF) {
    if w <= 0 || h <= 0 {
        return;
    }
    let brush = CreateSolidBrush(color);
    let rc = RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    };
    FillRect(dc, &rc, brush);
    let _ = DeleteObject(brush);
}

unsafe fn draw_circle(dc: HDC, cx: i32, cy: i32, r: i32, color: COLORREF) {
    if r <= 0 {
        return;
    }
    let brush = CreateSolidBrush(color);
    let pen = CreatePen(PS_SOLID as i32, 1, color);
    let old_b = SelectObject(dc, brush);
    let old_p = SelectObject(dc, pen);
    Ellipse(dc, cx - r, cy - r, cx + r, cy + r);
    SelectObject(dc, old_b);
    SelectObject(dc, old_p);
    let _ = DeleteObject(brush);
    let _ = DeleteObject(pen);
}

/// 用字形画一个图标（Segoe MDL2 Assets）。比 Polygon 好的地方是带 ClearType 抗锯齿，
/// 边缘不会毛糙，而且在 1.5 倍缩放下也不会糊成一团。
unsafe fn draw_glyph(
    dc: HDC,
    font: HFONT,
    code: u32,
    cx: i32,
    cy: i32,
    color: COLORREF,
    half: i32,
) {
    let ch = char::from_u32(code).unwrap_or('?');
    let mut buf: Vec<u16> = ch.to_string().encode_utf16().collect();
    let old = SelectObject(dc, font);
    SetTextColor(dc, color);
    let mut rc = RECT {
        left: cx - half,
        top: cy - half,
        right: cx + half,
        bottom: cy + half,
    };
    // DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX
    DrawTextW(
        dc,
        buf.as_mut_ptr(),
        buf.len() as i32,
        &mut rc,
        0x0000_0001 | 0x0000_0004 | 0x0000_0020 | 0x0000_0800,
    );
    SelectObject(dc, old);
}

unsafe fn click(hwnd: HWND, x: i32) {
    let mut rc = zero_rect();
    GetClientRect(hwnd, &mut rc);
    let w = rc.right;
    let h = rc.bottom;
    let (prev_x, play_x, next_x) = button_x(w, h);
    let hit = button_hit_radius(h);
    let action = if (x - prev_x).abs() <= hit {
        "previous"
    } else if (x - play_x).abs() <= hit {
        "play_pause"
    } else if (x - next_x).abs() <= hit {
        "next"
    } else {
        return;
    };
    let _ = forward_control(action);
    // 乐观更新：先翻转图标，别等主窗口回推。
    if action == "play_pause" {
        if let Ok(mut state) = STATE.lock() {
            state.playing = !state.playing;
        }
        repaint();
    }
}
