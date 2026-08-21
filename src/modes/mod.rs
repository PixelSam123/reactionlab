pub mod click_timing_reaction_test;
pub mod simple_reaction_time_test;
pub mod video_click_timing_test;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ModeId {
    SimpleReactionTimeTest,
    ClickTimingReactionTest,
    VideoClickTimingTest,
}

impl ModeId {
    pub const fn label(self) -> &'static str {
        match self {
            Self::SimpleReactionTimeTest => "Simple reaction time test",
            Self::ClickTimingReactionTest => "Click timing reaction test",
            Self::VideoClickTimingTest => "Video click timing test",
        }
    }
}
