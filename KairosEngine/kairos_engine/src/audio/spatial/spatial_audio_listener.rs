use kira::listener::ListenerId;

use crate::ecs::component::Component;

#[derive(Debug, Clone, Copy)]
pub struct SpatialAudioListenerComponent {
    pub listener_id: ListenerId,
    pub priority: u8,
}
impl Component for SpatialAudioListenerComponent {}
