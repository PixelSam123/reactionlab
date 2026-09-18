pub mod click_timing_test;
pub mod simple_reaction_time_test;
pub mod video_click_timing_test;

mod consts;

#[allow(clippy::enum_variant_names)]
#[derive(PartialEq, Eq)]
pub enum ModeId {
    SimpleReactionTimeTest,
    ClickTimingTest,
    VideoClickTimingTest,
}

impl ModeId {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::SimpleReactionTimeTest => "Simple reaction time test",
            Self::ClickTimingTest => "Click timing test",
            Self::VideoClickTimingTest => "Video click timing test",
        }
    }
}
