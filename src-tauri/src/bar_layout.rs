//! 任务栏条的全部布局计算：只吃数字，不碰 HWND / GDI。
//!
//! 为什么单独拆出来：native_bar.rs 里真正难改的不是绘制，而是「图标居中还是
//! 靠左、宽度够不够、任务栏动画停稳没有」这几处判断。它们以前和 FindWindowW /
//! GetWindowRect 缠在同一个 unsafe 函数里，只能靠开着任务栏人眼实测，边界值
//! （150% 缩放、托盘区还没布局完、任务栏高度调到小）根本测不出来。
//!
//! 搬到纯函数之后，`cargo test` 可以直接喂假矩形覆盖这些情况；实测发现算错了，
//! 先补一条单测把期望值钉住，再改实现——比"再开一次任务栏看看"快得多。

/// 逻辑尺寸（100% 缩放下）：480x44。
pub const BAR_LOGICAL_W: f32 = 480.0;
pub const BAR_LOGICAL_H: f32 = 44.0;
/// 可用宽度小于这个值就不落位：条缩成一条缝既难看也点不准。
pub const BAR_MIN_LOGICAL_W: f32 = 200.0;
/// 条与任务栏上下边缘各留多少（逻辑像素）。
pub const BAR_INSET_LOGICAL: f32 = 2.0;
/// 条的最小逻辑高度。
pub const BAR_MIN_LOGICAL_H: f32 = 26.0;
/// 图标区左边空当超过这个逻辑宽，就认为图标是居中布局（靠左实测只有 55）。
const CENTERED_LEFT_SPACE_LOGICAL: f32 = 200.0;
/// 条离托盘区左边留多少（逻辑像素）。
const NOTIFY_GAP_LOGICAL: f32 = 12.0;
/// 按钮几何（逻辑像素）：普通按钮 24 宽、主按钮 28 宽、间隔 2、右内边距 10。
/// 绘制（button_x）和点击热区（button_hit_radius）都从这里取，别再各写一个数。
const BUTTON_SMALL_HALF_LOGICAL: f32 = 12.0;
const BUTTON_MAIN_HALF_LOGICAL: f32 = 14.0;
const BUTTON_GAP_LOGICAL: f32 = 2.0;
const BUTTON_RIGHT_PAD_LOGICAL: f32 = 10.0;

/// 物理像素矩形（左上右下）。和 Win32 RECT 同形，但可以从容构造、能进单测。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RectI {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl RectI {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub const fn width(&self) -> i32 {
        self.right - self.left
    }

    pub const fn height(&self) -> i32 {
        self.bottom - self.top
    }

    pub const fn is_valid(&self) -> bool {
        self.right > self.left && self.bottom > self.top
    }
}

/// 条的目标矩形（物理像素；x / y 相对任务栏客户区）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// 图标区候选窗口（ReBarWindow32 / MSTaskSwWClass）取并集：左边界最小、右边界最大。
pub fn merge_list_bounds(bounds: &[(i32, i32)]) -> Option<(i32, i32)> {
    bounds
        .iter()
        .copied()
        .reduce(|(l1, r1), (l2, r2)| (l1.min(l2), r1.max(r2)))
}

/// 一次「任务栏停稳」判定的结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettleResult {
    /// 本次采样是否已经停稳（连续两次矩形完全相同）。
    pub settled: bool,
    /// 下一次采样要用的基线；None 表示任务栏已缩到屏幕外、基线要清空。
    pub next_last: Option<RectI>,
}

/// 任务栏运动判定，判据照 AF Media Bar 的 TaskbarMotionPolicy：
///
/// - 任务栏与所在显示器在交叉轴上的可见交集 ≤ 4 物理像素 → 已经滑到屏幕外
///   （自动隐藏状态），清基线、不重绘；
/// - 连续两次采样矩形完全一样才算停稳，避开隐藏 / 显示动画的半路。
pub fn settle_step(cur: RectI, monitor: RectI, last: Option<RectI>) -> SettleResult {
    // 横向任务栏（占满宽度）看它还能露出多少高度；纵向任务栏看宽度。
    let visible_cross = if cur.height() <= cur.width() {
        cur.bottom
            .min(monitor.bottom)
            .saturating_sub(cur.top.max(monitor.top))
    } else {
        cur.right
            .min(monitor.right)
            .saturating_sub(cur.left.max(monitor.left))
    };
    if visible_cross <= 4 {
        return SettleResult {
            settled: false,
            next_last: None,
        };
    }
    SettleResult {
        settled: last == Some(cur),
        next_last: Some(cur),
    }
}

/// 三个按钮的 x 坐标（prev / play / next），和点击热区共用，保证画哪儿点哪儿。
///
/// 照搬 WebView2 版 `ui/bar.css`：普通按钮 24、主按钮 28、按钮间隔 2、
/// 右内边距 10（都是逻辑像素）。
pub fn button_x(w: i32, h: i32) -> (i32, i32, i32) {
    let s = h as f32 / BAR_LOGICAL_H;
    let pad_r = (BUTTON_RIGHT_PAD_LOGICAL * s) as i32;
    let half_small = (BUTTON_SMALL_HALF_LOGICAL * s) as i32;
    let half_main = (BUTTON_MAIN_HALF_LOGICAL * s) as i32;
    let gap = (BUTTON_GAP_LOGICAL * s) as i32;
    let next = w - pad_r - half_small;
    let play = next - half_small - gap - half_main;
    let prev = play - half_main - gap - half_small;
    (prev, play, next)
}

/// 点击热区半径（物理像素）。和 `button_x` 的 half_small 同源：按钮画多宽就能点多大。
pub fn button_hit_radius(h: i32) -> i32 {
    (BUTTON_SMALL_HALF_LOGICAL * (h as f32 / BAR_LOGICAL_H)) as i32
}

/// 把条挂到任务栏右侧、托盘左边。
///
/// - `tray`：任务栏屏幕矩形；
/// - `right_edge`：托盘区左边界（拿不到托盘时传 taskbar.right）；
/// - `list_bounds`：应用图标区左右边界（拿不到传 None）；
/// - `scale`：当前 DPI 缩放（GetDpiForWindow / 96）。缩放必须由调用方取窗口
///   DPI，不能用「任务栏高 / 48」反推——150% 下任务栏物理高仍是 48，反推得 1，
///   条会被画成 480x44 的小条。
pub fn bar_placement(
    tray: RectI,
    right_edge: i32,
    list_bounds: Option<(i32, i32)>,
    scale: f32,
) -> Option<Placement> {
    if !tray.is_valid() {
        return None;
    }
    let tb_h = tray.height();
    let mut w = (BAR_LOGICAL_W * scale).round() as i32;
    // 高度跟着任务栏走，不写死 44：用户把「任务栏大小」调成小之后固定 44 的条
    // 会顶出任务栏、被父窗口裁掉一截。绘制里所有尺寸都是 h / BAR_LOGICAL_H
    // 等比缩放的，所以这里给对 h，版式会自己缩。
    let inset = (BAR_INSET_LOGICAL * scale).round() as i32;
    let max_h = (BAR_LOGICAL_H * scale).round() as i32;
    let min_h = (BAR_MIN_LOGICAL_H * scale).round() as i32;
    let h = (tb_h - inset * 2).clamp(min_h, max_h.max(min_h));
    let gap = (NOTIFY_GAP_LOGICAL * scale).round() as i32;

    // 判错最坏结果只是条落在另一侧，不影响可用性，所以用几何判断而不读注册表。
    let centered = match list_bounds {
        Some((l, _)) => l - tray.left > (CENTERED_LEFT_SPACE_LOGICAL * scale) as i32,
        None => false,
    };

    let x = if centered {
        // 居中：图标在中间、左边一大片空着，条就放左侧空区。
        let (list_left, _) = list_bounds.unwrap_or((tray.left, tray.right));
        let avail = list_left - gap - tray.left - gap;
        if avail > 0 && avail < w {
            w = avail;
        }
        gap
    } else {
        // 靠左：图标紧跟在开始按钮后面，条只能占图标区到托盘之间那块，
        // 不够 480 就缩窄（绘制按宽度自适应，按钮始终贴右边）。
        if let Some((_, list_right)) = list_bounds {
            let avail = right_edge - gap - list_right - gap;
            if avail > 0 && avail < w {
                w = avail;
            }
        }
        right_edge - w - gap - tray.left
    };

    if w < (BAR_MIN_LOGICAL_W * scale).round() as i32 {
        return None;
    }
    // 条是任务栏的子窗口，坐标必须是「相对任务栏客户区」的，不是屏幕绝对坐标。
    let y = (tb_h - h) / 2;
    Some(Placement { x, y, w, h })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 100% 缩放、480 宽、44 高：按钮坐标应分别是 402 / 430 / 458。
    #[test]
    fn button_x_at_100_percent() {
        assert_eq!(button_x(480, 44), (402, 430, 458));
    }

    /// 150% 缩放（h = 66）：所有间距按 1.5 倍走，按钮不能按 100% 的绝对像素摆。
    #[test]
    fn button_x_scales_with_height() {
        assert_eq!(button_x(720, 66), (603, 645, 687));
    }

    /// 热区半径必须等于普通按钮的半宽——画 24 宽就只该认 12 半径，否则点了旁边的
    /// 空当也会触发。
    #[test]
    fn hit_radius_matches_small_button_half_width() {
        assert_eq!(button_hit_radius(44), 12);
        assert_eq!(button_hit_radius(66), 18);
        let (prev, _, _) = button_x(480, 44);
        assert_eq!(button_hit_radius(44) * 2, 24);
        assert_eq!(prev, 402);
    }

    /// 图标区两个候选窗口取并集；一个都拿不到时返回 None（调用方回退到屏幕右侧）。
    #[test]
    fn merge_list_bounds_takes_union() {
        assert_eq!(merge_list_bounds(&[]), None);
        assert_eq!(merge_list_bounds(&[(624, 950)]), Some((624, 950)));
        assert_eq!(merge_list_bounds(&[(624, 950), (55, 382)]), Some((55, 950)));
    }

    /// 正常可见的任务栏：第一次采样不算停稳，第二次同样的矩形才算。
    #[test]
    fn settle_needs_two_identical_samples() {
        let mon = RectI::new(0, 0, 1920, 1080);
        let cur = RectI::new(0, 1032, 1920, 1080);
        let first = settle_step(cur, mon, None);
        assert!(!first.settled);
        assert_eq!(first.next_last, Some(cur));
        let second = settle_step(cur, mon, first.next_last);
        assert!(second.settled);
    }

    /// 任务栏缩到屏幕边缘（可见高度 4px 及以下）时判定为隐藏，并清空基线。
    #[test]
    fn settle_reports_hidden_and_clears_baseline() {
        let mon = RectI::new(0, 0, 1920, 1080);
        let hidden = RectI::new(0, 1076, 1920, 1080);
        let r = settle_step(hidden, mon, None);
        assert!(!r.settled);
        assert_eq!(r.next_last, None);
    }

    /// 纵向任务栏（靠屏幕左边）走横向交集的判据。
    #[test]
    fn settle_handles_vertical_taskbar() {
        let mon = RectI::new(0, 0, 1920, 1080);
        let cur = RectI::new(0, 0, 48, 1080);
        let r = settle_step(cur, mon, None);
        assert!(!r.settled);
        assert_eq!(r.next_last, Some(cur));
    }

    /// 150% 缩放 + 图标居中：托盘左边界 624、任务栏 3840 宽时，条应落在左侧空区，
    /// 且被可用宽度 588 压窄（原来按 480x44 小条算就是没取到 DPI 的症状）。
    #[test]
    fn placement_centered_at_150_percent() {
        let tray = RectI::new(0, 1032, 3840, 1080);
        let p = bar_placement(tray, 3840, Some((624, 950)), 1.5).unwrap();
        assert_eq!(
            p,
            Placement {
                x: 18,
                y: 3,
                w: 588,
                h: 42
            }
        );
    }

    /// 100% 缩放 + 图标靠左：条贴托盘左侧，宽 480、垂直居中。
    #[test]
    fn placement_left_aligned_at_100_percent() {
        let tray = RectI::new(0, 1032, 1920, 1080);
        let p = bar_placement(tray, 1800, Some((55, 382)), 1.0).unwrap();
        assert_eq!(
            p,
            Placement {
                x: 1308,
                y: 2,
                w: 480,
                h: 44
            }
        );
    }

    /// 图标区一直铺到 1700、托盘左边界 1824：中间只剩 100px，压到最小宽度
    /// （200）以下就干脆不落位，不让条挤成一条缝。
    #[test]
    fn placement_gives_up_when_too_narrow() {
        let tray = RectI::new(0, 1032, 1920, 1080);
        assert_eq!(bar_placement(tray, 1824, Some((55, 1700)), 1.0), None);
    }

    /// 任务栏高度被调小（36px）时条跟着变矮，不能还是 44 顶出任务栏。
    #[test]
    fn placement_follows_shorter_taskbar() {
        let tray = RectI::new(0, 1044, 1920, 1080);
        let p = bar_placement(tray, 1800, Some((55, 382)), 1.0).unwrap();
        assert_eq!(p.h, 32);
        assert_eq!(p.y, 2);
    }
}
