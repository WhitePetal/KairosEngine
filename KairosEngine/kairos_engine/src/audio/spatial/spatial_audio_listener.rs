use kira::listener::ListenerId;

use kairos_ecs::component::Component;

#[derive(Debug, Clone, Copy)]
pub struct SpatialAudioListenerComponent {
    pub listener_id: ListenerId,
    pub priority: u8,
}
// TODO!
// impl Component for SpatialAudioListenerComponent {}
