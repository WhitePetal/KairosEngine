//! The picker on platforms without a screen sampler.

use winit::window::Window;

use crate::eyedropper::Sample;

/// A picker that cannot sample: the editor still compiles and the widget keeps
/// its "not available" tooltip.
pub struct ScreenPicker;

impl ScreenPicker {
    pub const fn new() -> Self {
        Self
    }

    pub const fn available() -> bool {
        false
    }

    pub const fn is_running(&self) -> bool {
        false
    }

    pub const fn any_button_down(&self) -> bool {
        false
    }

    pub fn start(&mut self, _window: &Window) -> bool {
        false
    }

    pub fn poll(&mut self) -> Option<Sample> {
        None
    }

    pub fn stop(&mut self, _window: Option<&Window>) {}
}
