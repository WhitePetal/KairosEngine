use std::fmt::Debug;

/// Object safe version of [`ScheduleBuildPass`].
pub(super) trait ScheduleBuildPassObj: Send + Sync + Debug {}
