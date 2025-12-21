use crate::ui::UiLayout;

#[derive(Clone, Copy, Debug)]
pub struct RectI {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl RectI {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
struct HotkeysGroupLayout {
    top_y: i32,
    right_x: i32,
    group_w: i32,
    group_h: i32,
    hx: i32,
    hy0: i32,
    w_label: i32,
    w_edit: i32,
}

#[allow(dead_code)]
impl HotkeysGroupLayout {
    fn new(l: &UiLayout) -> Self {
        let right_x = l.right_x;
        let top_y = l.top_y;
        let group_w = l.group_w_right;
        let group_h = l.group_h;

        let hx = right_x + 12;
        let hy0 = top_y + 28;

        let w_label = 130;
        let w_edit = group_w - 12 - 12 - w_label - 8;

        Self {
            top_y,
            right_x,
            group_w,
            group_h,
            hx,
            hy0,
            w_label,
            w_edit,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    margin: i32,
    group_h: i32,
    group_w_left: i32,
    gap: i32,
    group_w_right: i32,
}

impl Layout {
    pub fn new(client_w: i32) -> Self {
        let margin = 12;
        let group_h = 170;
        let group_w_left = 240;
        let gap = 12;

        let fixed = margin * 2 + group_w_left + gap;
        let group_w_right = (client_w - fixed).max(420);

        Self {
            margin,
            group_h,
            group_w_left,
            gap,
            group_w_right,
        }
    }

    pub const fn left_x(self) -> i32 {
        self.margin
    }

    pub const fn top_y(self) -> i32 {
        self.margin
    }

    pub const fn right_x(self) -> i32 {
        self.margin + self.group_w_left + self.gap
    }

    pub const fn group_h(self) -> i32 {
        self.group_h
    }

    pub const fn group_w_left(self) -> i32 {
        self.group_w_left
    }

    pub const fn group_w_right(self) -> i32 {
        self.group_w_right
    }
}
