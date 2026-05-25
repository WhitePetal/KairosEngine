# Texture 资产：TOML + rkyv 混合格式设计讨论

> 来源：AI 辅助设计讨论，2026-05-26  
> 状态：设计草案，尚未在代码库中完整实现  
> 关联代码：`src/serialize_asset.rs`、`src/graphics/texture.rs`、`src/asset_loader/texture.rs`、`res/textures/*.texture`

## 1. 背景

当前 `.texture` 文件采用纯 TOML 格式，将贴图像素数据（`Vec<u8>`）直接序列化进 TOML 字段。例如 `kairos_texture.texture` 仅 8 行 TOML，但文件体积约 64MB，瓶颈在于 TOML 解析器需要处理巨大的文本字段。

本次讨论围绕以下问题展开：

1. Rust 的 `rkyv`（常被写作 rykv）能否将结构体序列化为 TOML？
2. 能否将 TOML 元数据与 rkyv 二进制段组合在同一 `.texture` 文件中？
3. 如何高效地分割 TOML 段与二进制段（`split_at_delimiter`）？
4. 保存时能否预先计算 binary 段的起始偏移并写入头部，以实现 O(1) 加载？

---

## 2. rkyv 与 TOML 的关系

### 2.1 结论：rkyv 不能直接输出 TOML

| | **rkyv** | **Serde + toml** |
|---|---|---|
| 输出格式 | 二进制 | TOML 文本 |
| 可读性 | 差 | 好 |
| 零拷贝 | 是 | 否 |
| trait 体系 | `Archive` / `rkyv::Serialize` / `rkyv::Deserialize` | `serde::Serialize` / `serde::Deserialize` |
| 典型用途 | 运行时缓存、热路径数据 | 配置文件、编辑器资源 |

rkyv 是零拷贝二进制序列化框架，没有 TOML 后端，也不会输出人类可读的文本。需要 TOML 时使用 Serde + `toml` crate（项目已在用）。

### 2.2 能否同时使用两者？

可以。结构体可同时 derive Serde 和 rkyv trait，但仍是两条独立路径：

- TOML：`toml::to_string(&meta)`
- 二进制：`rkyv::to_bytes(&payload)`

不能互相替代，也不能由一个 parser 自动完成。

---

## 3. TOML + rkyv 混合格式

### 3.1 核心思路

将资产拆成两层，加载时再组合：

```rust
// TOML 部分 —— 人类可读
struct Meta {
    source_path: String,
    width: u32,
    height: u32,
}

// rkyv 部分 —— 高性能二进制
#[derive(Archive, rkyv::Serialize, rkyv::Deserialize)]
struct TexturePayload {
    data: Vec<u8>,
}

// 运行时组合
struct TextureAsset {
    meta: Meta,
    texture: Texture,
}
```

加载流程：

1. 读取文件
2. 解析 TOML 段 → `Meta`
3. 解析二进制段 → rkyv 反序列化 → `TexturePayload`
4. 组合为 `TextureAsset`

### 3.2 TOML 不能直接承载原始二进制

TOML 规范没有「原始字节数组」类型，必须通过以下方式之一承载 rkyv 输出：

| 方案 | 说明 | 适用场景 |
|------|------|----------|
| A. TOML 头 + 分隔符 + 原始二进制尾 | 单文件，无 base64 膨胀 | **大贴图（推荐）** |
| B. TOML 内 base64 编码 rkyv 字节 | 纯 TOML，Serde 一条链路 | 小数据、调试 |
| C. 双文件 `.toml` + `.bin` | 元数据与二进制分离 | Git diff 友好 |

当前纯 TOML 方案（巨大 `data` 字段）的问题：体积膨胀、解析慢。方案 A 或 C 更适合大贴图。

### 3.3 推荐单文件布局（方案 A）

```text
[meta]
source_path = "res/textures/foo.png"
width = 2048
height = 2048

---BINARY---
<rkyv 原始字节>
```

---

## 4. 快速 split_at_delimiter

### 4.1 基本语义

```rust
fn split_at_delimiter<'a>(data: &'a [u8], delimiter: &[u8]) -> Option<(&'a [u8], &'a [u8])> {
    let pos = data.windows(delimiter.len()).position(|w| w == delimiter)?;
    Some((&data[..pos], &data[pos + delimiter.len()..]))
}
```

- 输入：整个文件的 `&[u8]`
- 输出：两个子切片，**零拷贝**
- TOML 段：`std::str::from_utf8(toml_part)?`
- 二进制段：直接交给 rkyv，**不要**当字符串解析

### 4.2 性能对比

| 实现 | 复杂度 | 说明 |
|------|--------|------|
| `windows` + 逐字节比较 | O(n × m) | 小文件足够，m 为分隔符长度 |
| `memchr::memmem` | O(n)，常数更小 | **大文件推荐**，SIMD 加速 |
| 长度前缀 | O(1) 定位 | 无需扫描，见 4.4 |

对「头小尾大」的 `.texture`，分隔符通常在文件前几百字节，`memchr` 找到即停，微秒级。

### 4.3 memchr 实现（推荐）

```toml
# Cargo.toml
memchr = "2"
```

```rust
use memchr::memmem;

const DELIM: &[u8] = b"\n---BINARY---\n";

pub fn split_at_delimiter<'a>(data: &'a [u8], delimiter: &[u8]) -> Option<(&'a [u8], &'a [u8])> {
    if delimiter.is_empty() {
        return None;
    }
    let pos = memmem::find(data, delimiter)?;
    Some((&data[..pos], &data[pos + delimiter.len()..]))
}
```

### 4.4 长度前缀（更快，O(1)）

若希望完全避免扫描：

```text
[4 byte: toml_len (LE u32)]
[toml_len bytes: UTF-8 TOML]
[remaining: raw rkyv bytes]
```

```rust
pub fn split_length_prefixed(data: &[u8]) -> anyhow::Result<(&[u8], &[u8])> {
    let toml_len = u32::from_le_bytes(data[..4].try_into()?) as usize;
    let header_end = 4 + toml_len;
    Ok((&data[4..header_end], &data[header_end..]))
}
```

### 4.5 混合方案（可读 + 安全）

分隔符 + 4 字节 payload 长度，避免二进制段误含分隔符子串：

```text
[toml...]
---BINARY---
[4 byte: payload_len]
[payload_len bytes: rkyv]
```

### 4.6 分隔符选择

- 使用足够长的分隔符，如 `b"\n---BINARY---\n"`（15 字节）
- 二进制段可能含任意字节，理论上可能误匹配分隔符；长度前缀可限定边界
- 可选 magic header：`KAIROS\x01\n` 防止误读其他格式

### 4.7 性能直觉（64MB 贴图）

- `split_at_delimiter`（memchr）：微秒级
- 全文件 base64 TOML 解析：**秒级**（当前方案主要成本）

混合格式的收益主要来自**不解析巨大 TOML 字符串**，而非 split 算法本身。

### 4.8 应避免的做法

- 将整个文件 `String::from_utf8` 再 `split`（二进制段可能 invalid UTF-8，且多一次分配）
- 用 `lines().collect()` 再拼接（多余分配）
- 分隔符过短（如单个 `\0`），误匹配风险高

---

## 5. 保存时预计算 binary 偏移

### 5.1 结论：可以，且推荐

保存时先算好「从文件头到 binary 第一字节」的偏移，写入头部，加载时 O(1) 定位，无需 `find` 分隔符。

```text
binary_start = header_bytes.len() + delimiter_bytes.len()
```

### 5.2 方案 A：偏移放在 TOML 外（最推荐）

TOML 保持纯元数据，文件最前 8 字节存 `u64 LE` 偏移：

```text
[0..8)     u64 LE → binary_start
[8..N)     TOML UTF-8
[N..)      rkyv 二进制
```

保存：

```rust
fn save(path: &Path, meta: &Meta, payload: &[u8]) -> anyhow::Result<()> {
    let toml_bytes = toml::to_vec(meta)?;
    let binary_start = 8u64 + toml_bytes.len() as u64;

    let mut buf = Vec::with_capacity(8 + toml_bytes.len() + payload.len());
    buf.extend_from_slice(&binary_start.to_le_bytes());
    buf.extend_from_slice(&toml_bytes);
    buf.extend_from_slice(payload);
    std::fs::write(path, buf)?;
    Ok(())
}
```

加载：

```rust
fn load(data: &[u8]) -> anyhow::Result<(Meta, &[u8])> {
    let binary_start = u64::from_le_bytes(data[..8].try_into()?) as usize;
    let meta: Meta = toml::from_slice(&data[8..binary_start])?;
    let payload = &data[binary_start..];
    Ok((meta, payload))
}
```

优点：一次算准、无循环依赖、加载 O(1)。前 8 字节非可读 TOML，但 offset 8 之后仍可人工查看 meta。

### 5.3 方案 B：偏移写在 TOML 内 —— 迭代直到稳定

若坚持在 TOML 中写 `binary_offset = 512`，添加该行会改变 TOML 长度，偏移需重新计算。迭代收敛：

```rust
fn toml_with_stable_offset(meta: &mut Meta, delim_len: usize) -> anyhow::Result<Vec<u8>> {
    let mut offset = 0u64;
    loop {
        meta.binary_offset = offset;
        let bytes = toml::to_vec(&*meta)?;
        let new_offset = (bytes.len() + delim_len) as u64;
        if new_offset == offset {
            return Ok(bytes);
        }
        offset = new_offset;
    }
}
```

通常 1～2 轮收敛；仅在 `999 → 1000` 等位数变化时可能多一轮。

### 5.4 方案 C：固定宽度字段

```toml
binary_offset = 0000000512
```

固定 10 位数字 + 固定键名，TOML 前缀长度恒定，一次算准，无迭代。

### 5.5 方案 D：存 TOML 长度而非偏移

```text
[0..4)  u32 LE → toml_len
[4..4+toml_len)  TOML
[4+toml_len..)   binary
```

与存偏移等价，`binary_start = 4 + toml_len (+ delim_len)`。

### 5.6 大文件两阶段写入

```rust
let mut f = File::create(path)?;
f.write_all(&[0u8; 8])?;              // placeholder
f.write_all(&toml_bytes)?;
f.write_all(DELIM)?;
let binary_start = 8 + toml_bytes.len() + DELIM.len();
f.seek(SeekFrom::Start(0))?;
f.write_all(&(binary_start as u64).to_le_bytes())?;
f.seek(SeekFrom::End(0))?;
f.write_all(payload)?;                // 或流式拷贝
```

### 5.7 方案选择

| 需求 | 推荐 |
|------|------|
| 加载最快、实现最简单 | 方案 A/D：文件头 u64/u32 |
| 文件打开尽量全是 TOML | 方案 B 迭代，或分隔符但不存 offset |
| 格式严格、无迭代 | 方案 C 固定宽度头 |
| 超大 binary、省内存 | 两阶段写 / seek patch |

---

## 6. 与当前代码的衔接

### 6.1 当前结构

```rust
// src/serialize_asset.rs
pub struct Meta {
    pub source_path: String,
}

pub struct TextureAsset {
    pub meta: Meta,
    pub texture: Texture,  // data: Vec<u8>, width, height
}

// src/asset_loader/texture.rs
let texture = toml::from_slice::<TextureAsset>(&texture_bytes)?;
```

### 6.2 建议迁移后的结构

```rust
#[derive(Serialize, Deserialize)]
pub struct TextureAssetMeta {
    pub source_path: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct TextureBinary {
    pub data: Vec<u8>,
}
```

### 6.3 建议文件布局（最终推荐）

```text
[0..8)   u64 LE binary_start
[8..binary_start)  TOML (TextureAssetMeta)
[binary_start..)   rkyv (TextureBinary)
```

理由：

- 加载 O(1)，无需 memchr 扫描
- TOML 段仍可人工阅读（跳过前 8 字节）
- 无 base64 膨胀
- 保存时一次算准偏移，无迭代

### 6.4 依赖变更（若采用 rkyv）

```toml
# Cargo.toml
rkyv = { version = "0.8", features = ["bytecheck"] }
memchr = "2"  # 若仍保留分隔符方案作为备选
```

---

## 7. rkyv 使用注意

| 点 | 说明 |
|----|------|
| trait 独立 | 与 Serde 是两套 derive，不能混用 |
| 类型固定 | 反序列化时必须知道具体类型 |
| 布局版本 | 结构体变更需考虑兼容性 |
| 校验 | 生产环境建议 `rkyv::check_bytes` 或 `bytecheck` feature |
| 元数据位置 | `width`/`height` 放 TOML 便于人工查看；也可只放在 rkyv 中 |

---

## 8. 总结

| 问题 | 答案 |
|------|------|
| rkyv 能序列化为 TOML 吗？ | **不能**，各走各的格式 |
| 能 TOML + rkyv 组合吗？ | **能**，需手动拆分/组合 |
| 如何快速 split？ | `memchr` 或长度前缀；大贴图优先长度前缀/预写偏移 |
| 保存时能预写 binary 偏移吗？ | **能**；放 TOML 外（前 8 字节）最干净 |
| 大贴图最佳实践？ | u64 偏移 + TOML meta + raw rkyv，避免 base64 大 blob |

---

## 9. 待办（实现前）

- [ ] 确定 `.texture` 格式版本号与 magic bytes
- [ ] 添加 `rkyv` 依赖并实现 `TextureBinary`
- [ ] 实现 `save_texture_asset` / `load_texture_asset`
- [ ] 迁移 `asset_loader/texture.rs` 加载路径
- [ ] 提供旧格式（纯 TOML）到新格式的导入工具
- [ ] 基准测试：对比纯 TOML vs 混合格式的加载耗时与文件体积
