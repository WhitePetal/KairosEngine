# glTF 顶点属性集编号笔记

本文记录 `gltf` crate 中 `read_colors(set)`、`read_tex_coords(set)` 等接口里 `set` 参数的含义，以及如何判断一个 mesh primitive 实际包含哪些属性集。

## `read_colors(set)` 的 `set` 是什么

`read_colors(set)` 里的 `set` 是顶点颜色属性集编号，对应 glTF primitive attributes 里的 `COLOR_n`。

例如：

```rust
reader.read_colors(0);
```

表示读取当前 primitive 的第一套顶点颜色，也就是 glTF 中的 `COLOR_0` 属性。

常见对应关系：

| glTF 属性 | `gltf` reader 调用 |
| --- | --- |
| `COLOR_0` | `reader.read_colors(0)` |
| `COLOR_1` | `reader.read_colors(1)` |
| `COLOR_2` | `reader.read_colors(2)` |

它不是颜色通道编号，也不是材质颜色编号，而是“第几套 vertex color”。如果模型没有对应的 `COLOR_n` 属性，`read_colors(n)` 会返回 `None`。

例如当前代码中的写法：

```rust
let colors = reader
    .read_colors(0)
    .map(|colors| {
        colors
            .into_rgba_f32()
            .map(|col| float4::from(col))
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();
```

含义是：尝试读取 `COLOR_0`，并转换成 `rgba f32`。如果当前 primitive 没有 `COLOR_0`，最终得到空的 `Vec`。

## 如何知道有多少个 Color 属性集

可以枚举 `primitive.attributes()`，查看当前 primitive 的所有 attribute semantic。

简单调试写法：

```rust
for (semantic, _accessor) in primitive.attributes() {
    println!("{semantic:?}");
}
```

如果 primitive 有顶点颜色，可能会输出类似：

```text
Positions
Normals
TexCoords(0)
Colors(0)
Colors(1)
```

其中：

- `Colors(0)` 对应 glTF 的 `COLOR_0`
- `Colors(1)` 对应 glTF 的 `COLOR_1`

如果想只统计颜色属性集，可以这样写：

```rust
let color_sets: Vec<_> = primitive
    .attributes()
    .filter_map(|(semantic, _)| match semantic {
        gltf::Semantic::Colors(set) => Some(set),
        _ => None,
    })
    .collect();

println!("color sets: {color_sets:?}");
```

输出示例：

```text
color sets: [0]
```

表示只有 `COLOR_0`。

```text
color sets: []
```

表示当前 primitive 没有顶点颜色属性，调用 `reader.read_colors(0)` 会返回 `None`。

## `read_tex_coords(set)` 也是同样的概念

`read_tex_coords(set)` 中的 `set` 是纹理坐标属性集编号，对应 glTF primitive attributes 里的 `TEXCOORD_n`。

常见对应关系：

| glTF 属性 | `gltf` reader 调用 |
| --- | --- |
| `TEXCOORD_0` | `reader.read_tex_coords(0)` |
| `TEXCOORD_1` | `reader.read_tex_coords(1)` |

也可以通过 `primitive.attributes()` 统计：

```rust
let texcoord_sets: Vec<_> = primitive
    .attributes()
    .filter_map(|(semantic, _)| match semantic {
        gltf::Semantic::TexCoords(set) => Some(set),
        _ => None,
    })
    .collect();

println!("texcoord sets: {texcoord_sets:?}");
```

## 实用建议

加载 glTF mesh 时，不要假设一定存在 `COLOR_0` 或 `TEXCOORD_0`。更稳妥的做法是先枚举 attributes，确认当前 primitive 有哪些数据，再决定读取哪一套属性。

对于引擎内部的默认顶点结构，如果某个属性不存在，可以按需求补默认值。例如：

- 没有 `COLOR_0` 时，使用 `float4::new(1.0, 1.0, 1.0, 1.0)` 作为默认顶点颜色。
- 没有 `TEXCOORD_0` 时，使用 `float2::new(0.0, 0.0)` 作为默认 UV。
- 没有 normal 或 tangent 时，根据后续渲染需求决定是否生成、跳过，或使用占位值。
