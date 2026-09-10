use kira::listener::ListenerId;

use kairos_ecs::component::Component;

#[derive(Component, Debug, Clone, Copy)]
pub struct SpatialAudioListenerComponent {
    pub listener_id: ListenerId,
    pub priority: u8,
}
