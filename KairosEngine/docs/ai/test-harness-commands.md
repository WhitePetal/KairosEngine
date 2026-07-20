# 测试指令参考 (Test Harness Commands)

本文档由代码自动生成，列出 AI agent 可通过 TOML 测试文件调用的所有引擎命令。
生成方式: `cargo run --features test-harness -- --gen-docs`

## Call 命令 (`action = "call"`)

用于触发引擎操作。在 TOML 中：

```toml
[[step]]
action = "call"
target = "<命令名>"
args = { ... }
```

| 命令 | 描述 | 参数 | 模式 |
|------|------|------|------|
| `system.ping` | 连通性测试命令，始终返回成功 | 无 | both |
| `system.query_widget` | 查询指定 ID 的 widget 屏幕坐标（需要 windowed 模式） | id (string) | windowed |

### 可用 Call 命令

#### `system.ping`

连通性测试命令，始终返回成功。

- **模式**: both

```toml
[[step]]
action = "call"
target = "system.ping"
```

#### `system.query_widget`

查询指定 ID 的 widget 屏幕坐标（需要 windowed 模式）。

- **模式**: windowed
- **参数**: id (string)

```toml
[[step]]
action = "call"
target = "system.query_widget"
args = { id (string) }
```

## Assert 命令 (`action = "assert"`)

用于验证引擎状态。在 TOML 中：

```toml
[[step]]
action = "assert"
target = "<断言名>"
args = { ... }
```

| 断言 | 描述 | 参数 | 模式 |
|------|------|------|------|
| `no_crash` | 断言自上次 `no_crash` 检查以来引擎未崩溃或出错 | 无 | both |
| `ecs_query` | 查询 ECS World。v1 支持 `query = "all"` 配合 `expect = "count >= N"` 等条件表达式 | query (string), expect (string) | both |
| `resource_exists` | 断言指定路径的文件或资源存在 | path (string) | both |
| `log_contains` | 断言引擎日志缓冲区包含指定模式字符串 | pattern (string) | both |
| `wgpu_valid` | 断言 GPU 资源有效。v1 为 stub，始终通过 | resource_type (string, 可选) | windowed |

### 可用 Assert 命令

#### `no_crash`

断言自上次 `no_crash` 检查以来引擎未崩溃或出错。

- **模式**: both
- **参数**: 无

```toml
[[step]]
action = "assert"
target = "no_crash"
```

#### `ecs_query`

查询 ECS World。v1 支持 `query = "all"` 配合 `expect = "count >= N"` 等条件表达式。

- **模式**: both
- **参数**: query (string), expect (string)

```toml
[[step]]
action = "assert"
target = "ecs_query"
args = { query = "all", expect = "count >= 1" }
```

#### `resource_exists`

断言指定路径的文件或资源存在。

- **模式**: both
- **参数**: path (string)

```toml
[[step]]
action = "assert"
target = "resource_exists"
args = { path = "path/to/file.asset" }
```

#### `log_contains`

断言引擎日志缓冲区包含指定模式字符串。

- **模式**: both
- **参数**: pattern (string)

```toml
[[step]]
action = "assert"
target = "log_contains"
args = { pattern = "error message" }
```

#### `wgpu_valid`

断言 GPU 资源有效。v1 为 stub，始终通过。

- **模式**: windowed
- **参数**: resource_type (string, 可选)

```toml
[[step]]
action = "assert"
target = "wgpu_valid"
args = { resource_type = "Texture" }
```

## Input 命令 (`action = "input"`)

用于向引擎注入键盘/鼠标事件。在 TOML 中：

```toml
[[step]]
action = "input"
args = { device = "...", event = "...", ... }
```

### 键盘输入

| 参数 | 值 | 说明 |
|------|-----|------|
| `device` | `"keyboard"` | 设备类型 |
| `event` | `"press"` / `"release"` | 按下或释放 |
| `key` | `"W"`, `"A"`, `"S"`, `"D"` | 按键名 |

```toml
[[step]]
action = "input"
args = { device = "keyboard", event = "press", key = "W" }
```

### 鼠标输入

**点击：**

| 参数 | 值 | 说明 |
|------|-----|------|
| `device` | `"mouse"` | 设备类型 |
| `event` | `"click"` | 点击事件 |
| `button` | `"Left"` / `"Right"` | 鼠标按键 |

```toml
[[step]]
action = "input"
args = { device = "mouse", event = "click", button = "Left" }
```

**移动：**

| 参数 | 值 | 说明 |
|------|-----|------|
| `device` | `"mouse"` | 设备类型 |
| `event` | `"move"` | 移动事件 |
| `x` | 数值 | 屏幕 X 坐标 |
| `y` | 数值 | 屏幕 Y 坐标 |

```toml
[[step]]
action = "input"
args = { device = "mouse", event = "move", x = 320.0, y = 240.0 }
```

