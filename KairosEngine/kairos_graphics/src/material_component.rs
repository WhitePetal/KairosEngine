use kairos_asset::next::Handle;

use crate::material::Material;
use kairos_ecs::component::Component;

#[derive(Component)]
pub struct MaterialComponent {
    pub material: Handle<Material>,
}

impl MaterialComponent {
    pub fn new(material: Handle<Material>) -> Self {
        Self { material }
    }
}
