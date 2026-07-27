use std::any::TypeId;

use crate::define_label;

define_label!(
    /// A strongly-typed class of labels used to identify a [`Schedule`].
    ///
    /// Each schedule in a [`World`] has a unique schedule label value, and
    /// schedules can be automatically created from labels via [`Schedules::add_systems()`].
    ///
    /// # Defining new schedule labels
    ///
    /// By default, you should use Bevy's premade schedule labels which implement this trait.
    /// If you are using [`bevy_ecs`] directly or if you need to run a group of systems outside
    /// the existing schedules, you may define your own schedule labels by using
    /// `#[derive(ScheduleLabel)]`.
    ///
    /// ```
    /// use bevy_ecs::prelude::*;
    /// use bevy_ecs::schedule::ScheduleLabel;
    ///
    /// // Declare a new schedule label.
    /// #[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash, Default)]
    /// struct Update;
    ///
    /// let mut world = World::new();
    ///
    /// // Add a system to the schedule with that label (creating it automatically).
    /// fn a_system_function() {}
    /// world.get_resource_or_init::<Schedules>().add_systems(Update, a_system_function);
    ///
    /// // Run the schedule, and therefore run the system.
    /// world.run_schedule(Update);
    /// ```
    ///
    /// [`Schedule`]: crate::schedule::Schedule
    /// [`Schedules::add_systems()`]: crate::schedule::Schedules::add_systems
    /// [`World`]: crate::world::World
    #[diagnostic::on_unimplemented(
        note = "consider annotating `{Self}` with `#[derive(ScheduleLabel)]`"
    )]
    ScheduleLabel,
    SCHEDULE_LABEL_INTERNER
);

define_label!(
    /// System sets are tag-like labels that can be used to group systems together.
    ///
    /// This allows you to share configuration (like run conditions) across multiple systems,
    /// and order systems or system sets relative to conceptual groups of systems.
    /// To control the behavior of a system set as a whole, use [`Schedule::configure_sets`](crate::prelude::Schedule::configure_sets),
    /// or the method of the same name on `App`.
    ///
    /// Systems can belong to any number of system sets, reflecting multiple roles or facets that they might have.
    /// For example, you may want to annotate a system as "consumes input" and "applies forces",
    /// and ensure that your systems are ordered correctly for both of those sets.
    ///
    /// System sets can belong to any number of other system sets,
    /// allowing you to create nested hierarchies of system sets to group systems together.
    /// Configuration applied to system sets will flow down to their members (including other system sets),
    /// allowing you to set and modify the configuration in a single place.
    ///
    /// Systems sets are also useful for exposing a consistent public API for dependencies
    /// to hook into across versions of your crate,
    /// allowing them to add systems to a specific set, or order relative to that set,
    /// without leaking implementation details of the exact systems involved.
    ///
    /// ## Defining new system sets
    ///
    /// To create a new system set, use the `#[derive(SystemSet)]` macro.
    /// Unit structs are a good choice for one-off sets.
    ///
    /// ```rust
    /// # use bevy_ecs::prelude::*;
    ///
    /// #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
    /// struct PhysicsSystems;
    /// ```
    ///
    /// When you want to define several related system sets,
    /// consider creating an enum system set.
    /// Each variant will be treated as a separate system set.
    ///
    /// ```rust
    /// # use bevy_ecs::prelude::*;
    ///
    /// #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
    /// enum CombatSystems {
    ///    TargetSelection,
    ///    DamageCalculation,
    ///    Cleanup,
    /// }
    /// ```
    ///
    /// By convention, the listed order of the system set in the enum
    /// corresponds to the order in which the systems are run.
    /// Ordering must be explicitly added to ensure that this is the case,
    /// but following this convention will help avoid confusion.
    ///
    /// ### Adding systems to system sets
    ///
    /// To add systems to a system set, call [`in_set`](crate::prelude::IntoScheduleConfigs::in_set) on the system function
    /// while adding it to your app or schedule.
    ///
    /// Like usual, these methods can be chained with other configuration methods like [`before`](crate::prelude::IntoScheduleConfigs::before),
    /// or repeated to add systems to multiple sets.
    ///
    /// ```rust
    /// use bevy_ecs::prelude::*;
    ///
    /// #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
    /// enum CombatSystems {
    ///    TargetSelection,
    ///    DamageCalculation,
    ///    Cleanup,
    /// }
    ///
    /// fn target_selection() {}
    ///
    /// fn enemy_damage_calculation() {}
    ///
    /// fn player_damage_calculation() {}
    ///
    /// let mut schedule = Schedule::default();
    /// // Configuring the sets to run in order.
    /// schedule.configure_sets((CombatSystems::TargetSelection, CombatSystems::DamageCalculation, CombatSystems::Cleanup).chain());
    ///
    /// // Adding a single system to a set.
    /// schedule.add_systems(target_selection.in_set(CombatSystems::TargetSelection));
    ///
    /// // Adding multiple systems to a set.
    /// schedule.add_systems((player_damage_calculation, enemy_damage_calculation).in_set(CombatSystems::DamageCalculation));
    /// ```
    #[diagnostic::on_unimplemented(
        note = "consider annotating '{Self}' with '#[derive(SystemSet)]'"
    )]
    SystemSet,
    SYSTEM_SET_INTERNER,
    extra_methods: {
        /// Returns `Some` if this system set is a [`SystemTypeSet`].
        fn system_type(&self) -> Option<TypeId> {
            None
        }

        /// Returns `true` if this system set is an [`AnonymousSet`].
        fn is_anonymous(&self) -> bool {
            false
        }
    },
    extra_methods_impl: {
        fn system_type(&self) -> Option<TypeId> {
            (**self).system_type()
        }

        fn is_anonymous(&self) -> bool {
            (**self).is_anonymous()
        }
    }
);
