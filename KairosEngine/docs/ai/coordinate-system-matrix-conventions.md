# 坐标系与矩阵约定讨论笔记

> 来源：AI 辅助设计讨论（2026-05-26）  
> 状态：设计参考；矩阵乘法部分已在 `src/math/matrix.rs` 中开始落地  
> 项目上下文：KairosEngine 使用 Rust + wgpu/WGSL，数学层已有 `float2`、`float3`、`float4` 与 `float4x4`

## 1. 核心结论

KairosEngine 当前建议采用以下 3D 空间约定：

```text
+X = right
+Y = up
+Z = forward
```

这是一套适合 DX12 / wgpu 的左手相机空间约定。DX12 本身并不规定世界坐标轴，只规定裁剪空间与深度范围等渲染 API 约定；世界空间的 `right/up/forward` 应由引擎自己统一定义。

在 wgpu / WGSL 中，建议配套使用：

```text
向量约定：列向量
矩阵语义：列主语义，Mat4 存 4 个列向量
乘法顺序：clip = P * V * M * local_position
NDC 深度：0..1
```

这样 Rust 侧上传的矩阵、WGSL `mat4x4<f32>` 的语义、以及数学公式可以保持一致，避免在 CPU/GPU 边界频繁转置。

## 2. Blender 坐标系

Blender 的整体坐标系可以概括为：

```text
右手系
+Z = up
前后方向在 Y 轴上
```

与游戏引擎常见的 `+Y up, +Z forward` 不同，Blender 更偏向 DCC / CAD / 建模直觉：`XY` 像地面或工作平面，`Z` 表示高度。模型资产的“正面”方向还会受建模习惯与导入导出设置影响，因此从 Blender 导入资产时，推荐在导入层做一次明确的轴转换，而不是让引擎内部跟随 Blender 坐标系。

如果引擎内部采用：

```text
Engine:  +X right, +Y up, +Z forward
Blender: +X right, +Z up, Y as front/back axis
```

那么资产导入时需要显式处理 `Y/Z` 轴语义差异。

## 3. Model 矩阵

列向量约定下，局部到世界：

```text
world = M * local
```

若物体有 `position/right/up/forward/scale`，则：

```text
M =
[ right.x*sx   up.x*sy   forward.x*sz   pos.x ]
[ right.y*sx   up.y*sy   forward.y*sz   pos.y ]
[ right.z*sx   up.z*sy   forward.z*sz   pos.z ]
[ 0            0         0              1     ]
```

等价组合：

```text
M = T * R * S
```

对列向量来说，实际作用顺序是从右到左：

```text
v' = T * R * S * v

先 S 缩放
再 R 旋转
最后 T 平移
```

Rust / WGSL 列数组可以写作：

```rust
pub type Mat4 = [[f32; 4]; 4]; // mat[column][row]

pub fn model_matrix(
    position: float3,
    right: float3,
    up: float3,
    forward: float3,
    scale: float3,
) -> Mat4 {
    [
        [right[0] * scale[0], right[1] * scale[0], right[2] * scale[0], 0.0],
        [up[0] * scale[1], up[1] * scale[1], up[2] * scale[1], 0.0],
        [forward[0] * scale[2], forward[1] * scale[2], forward[2] * scale[2], 0.0],
        [position[0], position[1], position[2], 1.0],
    ]
}
```

## 4. View 矩阵

相机空间采用：

```text
+X = camera right
+Y = camera up
+Z = camera forward
```

因此给定 `position`、`forward`、`world_up`，左手 look-to 可以构建为：

```rust
use crate::math::{cross, dot, float3, normalize};

pub fn view_look_to_lh(position: float3, forward: float3, world_up: float3) -> Mat4 {
    let f: float3 = normalize(&forward);
    let r: float3 = normalize(&cross(&world_up, &f));
    let u: float3 = cross(&f, &r);

    [
        [r[0], u[0], f[0], 0.0],
        [r[1], u[1], f[1], 0.0],
        [r[2], u[2], f[2], 0.0],
        [-dot(&r, &position), -dot(&u, &position), -dot(&f, &position), 1.0],
    ]
}
```

关键关系：

```text
right = normalize(cross(world_up, forward))
up    = cross(forward, right)
```

如果相机已经保存了 `forward/right`，则可以直接从 basis 构建：

```rust
pub fn view_from_camera_basis_lh(position: float3, forward: float3, right: float3) -> Mat4 {
    let f: float3 = normalize(&forward);
    let r: float3 = normalize(&right);
    let u: float3 = cross(&f, &r);

    [
        [r[0], u[0], f[0], 0.0],
        [r[1], u[1], f[1], 0.0],
        [r[2], u[2], f[2], 0.0],
        [-dot(&r, &position), -dot(&u, &position), -dot(&f, &position), 1.0],
    ]
}
```

## 5. Projection 矩阵

DX12 / wgpu 风格透视投影：

```text
左手系
+Z forward
NDC depth = 0..1
```

数学形式：

```text
y = 1 / tan(fov_y / 2)
x = y / aspect
a = far / (far - near)
b = -near * far / (far - near)

P =
[ x  0  0  0 ]
[ 0  y  0  0 ]
[ 0  0  a  b ]
[ 0  0  1  0 ]
```

Rust / WGSL 列数组：

```rust
pub fn perspective_lh_zo(fov_y_radians: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let y = 1.0 / (fov_y_radians * 0.5).tan();
    let x = y / aspect;
    let a = far / (far - near);
    let b = -near * far / (far - near);

    [
        [x, 0.0, 0.0, 0.0],
        [0.0, y, 0.0, 0.0],
        [0.0, 0.0, a, 1.0],
        [0.0, 0.0, b, 0.0],
    ]
}
```

最终 shader 侧使用：

```wgsl
out.position = camera.proj * camera.view * model.matrix * in.position;
```

## 6. 列向量矩阵乘法

列向量约定下：

```text
C = A * B
```

结果矩阵 `C` 的每一列等于左矩阵 `A` 乘以右矩阵 `B` 的对应列向量：

```text
C.col0 = A * B.col0
C.col1 = A * B.col1
C.col2 = A * B.col2
C.col3 = A * B.col3
```

这也是 `float4x4` 作为 4 个列向量存储时最自然的实现方式：

```rust
Self([
    Self::mul_col(&self.0, rhs.0[0]),
    Self::mul_col(&self.0, rhs.0[1]),
    Self::mul_col(&self.0, rhs.0[2]),
    Self::mul_col(&self.0, rhs.0[3]),
])
```

直观理解：右矩阵 `B` 是一组列向量，左矩阵 `A` 分别变换这些列向量，得到的新 4 列拼成结果矩阵。

## 7. SIMD 实现思路

`src/math/matrix.rs` 中的 `float4x4` 当前是列主存储：

```rust
pub struct float4x4([float4; 4]);
```

`float4` 内部包了一层 `f32x4`，所以矩阵乘向量可以写成：

```text
result = lhs_col0 * rhs.x
       + lhs_col1 * rhs.y
       + lhs_col2 * rhs.z
       + lhs_col3 * rhs.w
```

对应 SIMD 形式：

```rust
fn mul_col(lhs: &[float4; 4], rhs: float4) -> float4 {
    float4::from_simd(
        lhs[0].0 * simd_swizzle!(rhs.0, [0, 0, 0, 0])
            + lhs[1].0 * simd_swizzle!(rhs.0, [1, 1, 1, 1])
            + lhs[2].0 * simd_swizzle!(rhs.0, [2, 2, 2, 2])
            + lhs[3].0 * simd_swizzle!(rhs.0, [3, 3, 3, 3]),
    )
}
```

这里 `simd_swizzle!` 的作用是把 `rhs` 的一个分量广播成完整的 `f32x4`：

```text
rhs.x -> [rhs.x, rhs.x, rhs.x, rhs.x]
rhs.y -> [rhs.y, rhs.y, rhs.y, rhs.y]
rhs.z -> [rhs.z, rhs.z, rhs.z, rhs.z]
rhs.w -> [rhs.w, rhs.w, rhs.w, rhs.w]
```

然后让 4 个 lane 同时完成结果列向量的 `x/y/z/w` 计算。这样避免了手写 16 个标量 dot，也不需要先转置矩阵。

## 8. 工程建议

后续数学层建议继续保持以下约定：

```text
1. 引擎世界空间：+X right, +Y up, +Z forward
2. 相机/view 空间：左手系，+Z forward
3. 矩阵与向量：列向量
4. 矩阵存储：4 个列向量
5. WGSL：clip = P * V * M * vertex
```

如果未来接入 HLSL row-vector 风格或某些资产格式的不同轴约定，应在边界层做转换，避免核心数学库内部混用多套语义。
