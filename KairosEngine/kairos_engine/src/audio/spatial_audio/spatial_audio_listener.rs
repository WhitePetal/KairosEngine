use kira::listener::ListenerHandle;

use crate::ecs::component::Component;



pub struct SpatialAudioListenerComponent {
    pub handle: ListenerHandle,
    pub priority: u32,
}
impl Component for SpatialAudioListenerComponent {}