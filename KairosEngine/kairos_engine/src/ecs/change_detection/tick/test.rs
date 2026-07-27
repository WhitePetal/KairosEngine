use super::*;

#[test]
fn tick_wrapping_add() {
    let t = Tick(u32::MAX);
    assert_eq!((t + 1).0, 0);
}

#[test]
fn tick_relative_to() {
    let t1 = Tick(10);
    let t2 = Tick(20);
    assert_eq!(t1.relative_to(t2), Tick(10));

    // wrapping
    let t1 = Tick(u32::MAX);
    let t2 = Tick(5);
    assert_eq!(t1.relative_to(t2), Tick(6));
}

#[test]
fn is_newer_than_detects_change() {
    let last_run = Tick(10);
    let this_run = Tick(20);
    // 组件在 last_run 之后修改
    let component_tick = Tick(15);
    assert!(component_tick.is_newer_than(last_run, this_run));

    // 组件在 last_run 之前修改
    let component_tick = Tick(5);
    assert!(!component_tick.is_newer_than(last_run, this_run));
}

#[test]
fn is_newer_than_wrapping() {
    // 模拟 wrapping 场景
    let last_run = Tick(u32::MAX - 5);
    let this_run = Tick(5); // wrapped around
    let component_tick = Tick(u32::MAX - 2);
    assert!(component_tick.is_newer_than(last_run, this_run));
}

#[test]
fn component_ticks_new() {
    let tick = Tick(42);
    let ct = ComponentTicks::new(tick);
    assert_eq!(ct.added, tick);
    assert_eq!(ct.changed, tick);
}

#[test]
fn component_ticks_set_changed() {
    let mut ct = ComponentTicks::new(Tick(10));
    assert_eq!(ct.added, Tick(10));
    assert_eq!(ct.changed, Tick(10));

    ct.set_changed(Tick(20));
    assert_eq!(ct.added, Tick(10)); // unchanged
    assert_eq!(ct.changed, Tick(20)); // updated
}

#[test]
fn is_added_detects_new_insert() {
    let ct = ComponentTicks::new(Tick(15));
    let last_run = Tick(10);
    let this_run = Tick(20);
    assert!(ct.is_added(last_run, this_run));
}

#[test]
fn is_changed_detects_modification() {
    let mut ct = ComponentTicks::new(Tick(10));
    ct.set_changed(Tick(18));

    let last_run = Tick(10);
    let this_run = Tick(20);
    assert!(ct.is_changed(last_run, this_run));
}
