use crate::ecs::system::{IntoSystem, System, SystemInput};

#[test]
fn into_system_type_id_consistency() {
    fn test<T, In: SystemInput, Out, Marker>(function: T)
    where
        T: IntoSystem<In, Out, Marker> + Copy,
    {
        fn reference_system() {}

        use core::any::TypeId;

        let system = IntoSystem::into_system(function);

        assert_eq!(
            system.system_type(),
            function.system_type_id(),
            "System::system_type should be consistent with IntoSystem::system_type_id"
        );

        assert_eq!(
            system.system_type(),
            TypeId::of::<T::System>(),
            "System::system_type should be consistent with TypeId::of::<T::System>()"
        );

        assert_ne!(
            system.system_type(),
            IntoSystem::into_system(reference_system).system_type(),
            "Different systems should have different TypeIds"
        );
    }

    fn function_system() {}

    test(function_system);
}
