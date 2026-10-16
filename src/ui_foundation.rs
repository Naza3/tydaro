//! Experimental native foundations. The three variants share semantic colors;
//! structure and interaction, rather than a palette swap, distinguish them.
use gpui_kit::{
    component::button::{Button, ButtonVariants},
    prelude::*,
    *,
};

pub const BG: u32 = 0xf3f5f7;
pub const SURFACE: u32 = 0xffffff;
pub const INK: u32 = 0x263549;
pub const MUTED: u32 = 0x697789;
pub const LINE: u32 = 0xdce3ea;
pub const ACCENT: u32 = 0x008987;
pub const SOFT: u32 = 0xe9f5f3;
pub const UP: u32 = 0xc73655;
pub const DOWN: u32 = 0x007d78;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Precision,
    Calm,
    Context,
}
impl Variant {
    pub const ALL: [Self; 3] = [Self::Precision, Self::Calm, Self::Context];
    pub fn index(self) -> usize {
        match self {
            Self::Precision => 0,
            Self::Calm => 1,
            Self::Context => 2,
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Precision => "a",
            Self::Calm => "b",
            Self::Context => "c",
        }
    }
    pub fn letter(self) -> &'static str {
        match self {
            Self::Precision => "A",
            Self::Calm => "B",
            Self::Context => "C",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Precision => "精密工具",
            Self::Calm => "轻量工作面",
            Self::Context => "情境交互",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Precision => "对齐、紧凑、直接操作",
            Self::Calm => "主次分明、留白、逐项阅读",
            Self::Context => "先看内容，再展开操作",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Actions,
    Input,
    Choice,
    Toggle,
    Number,
    Tabs,
    Tokens,
    List,
    Disclosure,
    Feedback,
}
impl Family {
    pub const ALL: [Self; 10] = [
        Self::Actions,
        Self::Input,
        Self::Choice,
        Self::Toggle,
        Self::Number,
        Self::Tabs,
        Self::Tokens,
        Self::List,
        Self::Disclosure,
        Self::Feedback,
    ];
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|family| *family == self).unwrap()
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Actions => "actions",
            Self::Input => "input",
            Self::Choice => "choice",
            Self::Toggle => "toggle",
            Self::Number => "number",
            Self::Tabs => "tabs",
            Self::Tokens => "tokens",
            Self::List => "list",
            Self::Disclosure => "disclosure",
            Self::Feedback => "feedback",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Actions => "按钮与动作",
            Self::Input => "文本输入",
            Self::Choice => "单项选择",
            Self::Toggle => "开关与勾选",
            Self::Number => "数值输入",
            Self::Tabs => "页签导航",
            Self::Tokens => "状态与筛选标签",
            Self::List => "列表与数据行",
            Self::Disclosure => "折叠与详情",
            Self::Feedback => "操作反馈",
        }
    }
    pub fn purpose(self) -> &'static str {
        match self {
            Self::Actions => "执行同一个动作：看看按钮如何表达主次、可用性和执行结果。",
            Self::Input => "编辑同一个名称：比较输入的位置、校验时机与保存方式。",
            Self::Choice => "选择同一个显示模式：比较选项的可见性、说明和操作步数。",
            Self::Toggle => "改变同一个提醒状态：比较开关、勾选与按压式表达。",
            Self::Number => "设置同一个数值阈值：比较精确输入、步进与预设刻度。",
            Self::Tabs => "切换同一组内容：比较水平、分段与垂直导航的空间取舍。",
            Self::Tokens => "表达状态与筛选条件：区分只读信息、可切换与可移除的标签。",
            Self::List => "浏览同一组模拟标的：比较扫描密度、信息层级与详情位置。",
            Self::Disclosure => "查看同一条规则的参数：比较原位展开、摘要与独立检视区。",
            Self::Feedback => "完成一次演示操作：比较原位反馈、撤销与明确确认。",
        }
    }
    pub fn differences(self) -> [&'static str; 3] {
        match self {
            Self::Actions => [
                "紧凑工具条，动作并置；可用与禁用并看。",
                "一个清楚的主动作，辅助动作降低权重。",
                "命令完成后变为结果回执，并提供撤销。",
            ],
            Self::Input => [
                "标签固定在上方，错误紧邻输入框。",
                "标签与值同行，缩短纵向扫描距离。",
                "默认阅读，进入编辑后才出现保存与取消。",
            ],
            Self::Choice => [
                "全部选项常驻一条轨道，一次点击切换。",
                "选项自带解释，适合有理解成本的设置。",
                "平时只见当前值，展开后再选择其他项。",
            ],
            Self::Toggle => [
                "设置行右侧滑动开关，状态位置固定。",
                "勾选框与解释绑定，能看清开启的含义。",
                "按压片自身表达开关，状态和动作合一。",
            ],
            Self::Number => [
                "输入精确数字，单位与范围始终可见。",
                "递增递减按钮，适合连续的小幅调整。",
                "预设刻度快速选择，同时保留精确输入。",
            ],
            Self::Tabs => [
                "底线标识活动页签，节省容器边框。",
                "封闭分段强调同组互斥，点击区域完整。",
                "纵向导航保留计数，适合较长的标签名。",
            ],
            Self::Tokens => [
                "状态点表达信息，筛选操作独立放置。",
                "轮廓与选中态区分信息和可选标签。",
                "把已用条件变成可移除的词条。",
            ],
            Self::List => [
                "数值按列对齐，用紧密行距提高扫描密度。",
                "名称与次要信息分两行，选择边线更明确。",
                "列表保持简洁，选中项在检视区展开。",
            ],
            Self::Disclosure => [
                "一行标题控制原位展开，适合次要细节。",
                "折叠时保留参数摘要，展开后查看说明。",
                "选中对象打开独立检查区，显式关闭。",
            ],
            Self::Feedback => [
                "反馈紧邻触发按钮，错误可以原位重试。",
                "回执代替操作区域，保留撤销入口。",
                "先确认具体影响，再执行并返回结果。",
            ],
        }
    }
}

pub fn hint(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(12.))
        .text_color(rgb(MUTED))
        .child(text.into())
}
pub fn caption(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text.into())
}
pub fn surface(variant: Variant) -> Div {
    let base = div().min_w_0().bg(rgb(SURFACE)).text_color(rgb(INK));
    match variant {
        Variant::Precision => base.border_1().border_color(rgb(LINE)).rounded_sm(),
        Variant::Calm => base.rounded_lg(),
        Variant::Context => base.border_l_2().border_color(rgb(LINE)),
    }
}
pub fn action(
    id: impl Into<ElementId>,
    variant: Variant,
    label: impl Into<SharedString>,
) -> Button {
    let button = Button::new(id).label(label).text_size(px(13.));
    match variant {
        Variant::Precision => button.h(px(30.)).rounded_sm().px_2(),
        Variant::Calm => button.h(px(38.)).rounded_lg().px_4(),
        Variant::Context => button.ghost().h(px(34.)).rounded_md().px_3(),
    }
}
