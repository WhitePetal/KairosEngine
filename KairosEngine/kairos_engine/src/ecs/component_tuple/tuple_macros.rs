/// 编译期计算 tuple 中元素个数
macro_rules! count {
    () => { 0 };
    ($x: ident $(, $rest: ident)*) => { 1 + count!($($rest),*) };
}

/// 逆序展开 token 序列：reverse_apply!{macro_name [A B C]} → macro_name!{C, B, A}
macro_rules! reverse_apply {
    ($m: ident [] $($reversed:tt)*) => {
        $m!{$($reversed),*}  // base case
    };
    ($m: ident [$first:tt $($rest:tt)*] $($reversed:tt)*) => {
        reverse_apply!{$m [$($rest)*] $first $($reversed)*}
    };
}

/// 对逐渐缩小的 tuple 逐一调用宏：从完整序列一直递减到单个和空。
/// 例如 `smaller_tuples_too!(m, A, B, C)` 会展开为：
///   m!{}
///   m!{A}
///   m!{A, B}
///   m!{C, B, A}  (reverse_apply)
///   m!{A, B, C}? 不，是递减子序列...
///
/// 实际上：`smaller_tuples_too!(m, A, B, C)` 展开过程：
///   smaller_tuples_too!{m, B, C}
///   reverse_apply!{m [A B C]} → m!{C, B, A}
macro_rules! smaller_tuples_too {
    ($m: ident, $next: tt) => {
        $m!{}
        $m!{$next}
    };
    ($m: ident, $next: tt, $($rest: tt),*) => {
        smaller_tuples_too!{$m, $($rest),*}
        reverse_apply!{$m [$next $($rest)*]}
    };
}

// 让父模块能够 use 这些宏
pub(crate) use count;
pub(crate) use reverse_apply;
pub(crate) use smaller_tuples_too;
