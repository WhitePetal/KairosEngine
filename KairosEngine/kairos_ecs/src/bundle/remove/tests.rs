use std::vec;

#[test]
fn sorted_remove() {
    let mut a = vec![1, 2, 3, 4, 5, 6, 7];
    let b = vec![1, 2, 3, 5, 7];
    super::sorted_remove(&mut a, &b);

    assert_eq!(a, vec![4, 6]);

    let mut a = vec![1];
    let b = vec![1];
    super::sorted_remove(&mut a, &b);

    assert!(a.is_empty());

    let mut a = vec![1];
    let b = vec![2];
    super::sorted_remove(&mut a, &b);

    assert_eq!(a, vec![1]);
}
