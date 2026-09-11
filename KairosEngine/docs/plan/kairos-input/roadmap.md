# kairos_input 路线图 — 照 `bevy_input` 0.19.1 构造输入系统

**日期**: 2026-09-11
**蓝本**: `bevy_input` / `bevy_winit` 0.19.1（本地完整 checkout：`/Users/baiaoxiang/bevy`，tag 提交 `b56fc29d3`）
**状态**: 路线图定稿，实现未开始
**前置阅读**: `docs/adr/0003-asset-registration-and-stage-ownership.md`（stage label 归属惯例）、`kairos_engine/src/kairos_editor/schedule.rs`（调度骨架）

---

## 0. 先回答出发时那个困惑：消息从哪来、谁消费

`bevy_input` 的 `InputPlugin::build` **只做三件事，它一条消息都不发**：

1. **声明通道** — `add_message::<KeyboardInput>()` / `<MouseButtonInput>` / `<MouseMotion>` / `<MouseWheel>` / `<TouchInput>` / `<GamepadEvent>` …
2. **初始化资源** — `init_resource::<ButtonInput<KeyCode>>()` / `<ButtonInput<Key>>` / `<ButtonInput<MouseButton>>` / `<AccumulatedMouseMotion>` / `<AccumulatedMouseScroll>` / `<Touches>`
3. **注册消费者系统** — 全挂在 `PreUpdate` 的 `InputSystems` set 里（`bevy_input/src/lib.rs:108-166`）

真正的**发送方是平台后端 crate，不在 `bevy_input` 里**：

| 消息 | 发送方 | 发送点 |
|---|---|---|
| `KeyboardInput` / `KeyboardFocusLost` | `bevy_winit` | `state.rs` `forward_bevy_events()`（`world.write_message`）、`system.rs` `check_keyboard_focus_lost()`（`MessageWriter<KeyboardInput>`） |
| `MouseButtonInput` / `MouseMotion` / `MouseWheel` | `bevy_winit` | `state.rs` `forward_bevy_events()` |
| `TouchInput` | `bevy_winit` | `state.rs`（经 `BevyWindowEvent::TouchInput`）、`converters.rs` 转换 |
| `PinchGesture` / `RotationGesture` / `DoubleTapGesture` / `PanGesture` | `bevy_winit` | `state.rs`（由 `WindowEvent::*Gesture` 转换） |
| `GamepadEvent` 族 | `bevy_gilrs` | gilrs 事件泵 |
| `GamepadRumbleRequest` | **用户代码 / bevy_input 侧发起** | 由 `bevy_gilrs` 消费（唯一反向的一条） |

**核心结论**：消息是「**平台层 ↔ 输入层**」的内部边界协议；消费方读的是**资源**，不是消息。

```
winit EventLoop（OS 原始事件）
  └─ bevy_winit: forward_bevy_events() → world.write_message(..)      ← 发送方（不在 bevy_input）
       └─ Messages<KeyboardInput> / <MouseMotion> / …                 ← bevy_input 只声明通道
            └─ bevy_input 的 PreUpdate 系统（InputSystems）            ← 消费第一层：折叠成资源
                 └─ ButtonInput<KeyCode> / AccumulatedMouseMotion / …
                      └─ 玩家代码、bevy_ui、bevy_camera、common_conditions  ← 消费第二层：读资源
```

**消费第一层的完整清单**（`PreUpdate` / `InputSystems`）：

| 系统 | 读 | 写 |
|---|---|---|
| `keyboard_input_system` | `MessageReader<KeyboardInput>`、`MessageReader<KeyboardFocusLost>` | `ButtonInput<KeyCode>`、`ButtonInput<Key>` |
| `mouse_button_input_system` | `MessageReader<MouseButtonInput>` | `ButtonInput<MouseButton>` |
| `accumulate_mouse_motion_system` | `MessageReader<MouseMotion>` | `AccumulatedMouseMotion { delta: float2 }` |
| `accumulate_mouse_scroll_system` | `MessageReader<MouseWheel>` | `AccumulatedMouseScroll { unit, delta }` |
| `touch_screen_input_system` | `MessageReader<TouchInput>` | `Touches` |
| `gamepad_connection_system` / `gamepad_event_processing_system` | `MessageReader<Gamepad*>` | `Gamepad` 组件族 |

两个值得记住的实现细节：

- bevy 自己在消费系统开头用 `bypass_change_detection().clear()`，并在源码留了注释：*"Avoid clearing if not empty to ensure change detection is not triggered."*（`keyboard.rs:176-178`、`mouse.rs:193`）—— 输入资源**每帧都会变**，变更检测必须显式绕开，否则 `resource_changed::<ButtonInput<_>>` 永远为真。
- `AccumulatedMouseMotion` / `AccumulatedMouseScroll` 用**赋值**而非累加写入，所以天然每帧归零（`mouse.rs:267`）。

### KairosEngine 现状对照

| bevy 环节 | kairos 现状 |
|---|---|
| `bevy_winit` 事件泵 → 消息 | **不存在**。winit 的 `ApplicationHandler` 是 `KairosEditorRuntime`（`kairos_editor/runtime.rs:475`），事件直接进 `Engine.input_engine` 字段 |
| `Messages` 接线 | **完全没有**。`kairos_ecs::message` 全套机制都在（`MessageRegistry` / `message_update_system` / `MessageUpdateSystems`），但全仓库无 `add_message`、无 `register_message`、无处挂载 |
| `ButtonInput<KeyCode>` 资源 | **不存在**。`Engine.input_engine: InputEngine` 是普通结构体字段，不是 World resource |
| `InputSystems` 消费层 | **不存在** |

而现有的 `InputEngine`（`kairos_engine/src/inputs.rs`，154 行）已核实为**死代码**：

- `InputEngine::update(delta_time)` —— `Holding(f32)` 时长唯一推进的地方 —— **全仓库无人调用**（连 `KairosEngine::update` 都没有调）
- `get_input_state` —— **无人读取**
- 活的只有两条边：`registe_input`（`kairos_game.rs:166-180` 注册 WASD）与 `update_keyboard_input`（runtime 转发 winit）

**所以本路线图不是「扩展一个能用的系统」，而是「在一个 stub 上建真的」。要的输入时长/帧数当前完全不通电。**

另：编辑器相机输入走的是**另一条无关的路** —— `SceneViewInput` 资源由 egui UI 层写入（`ui.rs:692-700`），与 `InputEngine` 没有任何关系。当前输入面是碎的（本路线图把这条划出，见 §7）。

---

## 1. 本次已定决策

| # | 决策 | 结论 | 理由 |
|---|---|---|---|
| 1 | 终点形态 | **只产出路线图文档**（本文件），不建 GitHub wayfinder map | 用户选择 |
| 2 | 左边界 | `kairos_input` **保持纯**：零 winit、零 egui 依赖（与 bevy 一致 —— `bevy_input` 确实不依赖 winit）；`winit → write_message` adapter 落在 **engine runtime**，且**在本路线图范围内** | 分层忠于 bevy，且模块能真跑 |
| 3 | 支柱范围 | **含**：核心 `ButtonInput<T>`、keyboard、mouse、`common_conditions`。**不含**：touch、gestures、gamepad | 桌面编辑器无前两者消费方；gamepad 是大块且需 gilrs 新依赖 |
| 4 | 时长/帧数归属 | **作为 `ButtonInput<T>` 的一等公民**（内嵌，认真设计）。这是对 bevy 的**一处有意分叉** | 用户要求「一处查询」；一致性（失焦/同帧）由类型内部保证，不可能漂移 |
| 5 | 按下帧结算 | **甲**：按下帧 `held_for = 0`、`held_frames = 1`；tick 系统住 **`First`**（`time_system` 之后、`PreUpdate` 之前） | 时长语义物理真实（刚按下＝持续 0）；帧数语义直觉（含按下帧） |
| 6 | 消息轨道 | **本路线图自己拥有 Phase 0**，方案与 asset effort 子票 #190 的**已定决定对齐**，并在 #184 留言知会避免重复实现 | input 100% 消息驱动，不能被更大的 asset effort 卡住；input 反而是消息驱动最纯的案例 |
| 7 | 仲裁信号 | **状态查询**：键盘用 `ctx.wants_keyboard_input()`，指针用 GameWindow 的 `Response`（`hovered()` / `has_focus()`）。**不用**逐事件 `consumed` | `consumed` 恰好会吃掉「聚焦 GameWindow 的那一次点击」，与需求相反；状态查询让「GameWindow 聚焦 → 游戏输入」自动成立 |
| 8 | 闸口 | editor 端设**一道闸口**；闸口是**引擎层通用设施、默认常开**；打包游戏无人关它 ⇒ 等价于「没有这道闸口」 | 用户提出。保留「游戏将来也能关」（暂停菜单、过场、游戏内文本框）的能力 |
| 9 | 打包形态 | **不包含**独立游戏运行时；adapter 写成 egui-free 可复用 + 闸口默认值机制保证将来接入零成本 | 独立 game runtime 是另一个 effort 的体量（需自己的 App 引导、窗口管理、无 egui 渲染路径） |
| 10 | 落地形态 | **包甲**：新 `kairos_input` crate + 保守增量；`SceneViewInput` **不动、划出** | 照 `kairos_math` / `kairos_time` / `kairos_physics` 惯例；第一个真消费方用 `KairosGame` 的 WASD |

---

## 2. 目标架构（落地后的形态）

```
【engine 侧】winit 事件循环 —— KairosEditorRuntime::window_event (runtime.rs:505)

  1.  egui 先看：let response = egui_state.on_window_event(&window, &event)
      runtime.rs:519-524 今天只读了 response.repaint，丢掉了 response.consumed

  2.  WindowEvent ──► 闸口（editor 端）
                      读 Res<InputRouting>，默认常开 = 全转发
                      keyboard_to_game / pointer_to_game
                          │
                          │ 放行
                          ▼
  3.  adapter：WindowEvent → 输入消息类型（含 convert_physical_key_code）

  4.  world.write_message(msg)

【kairos_input crate】纯 —— 零 winit、零 egui 依赖
  （消息边界就是 crate 边界：engine 侧只负责把消息写进来，crate 内只负责消费）

  Messages<KeyboardInput> / <MouseButtonInput> / <MouseMotion>
           / <MouseWheel> / <KeyboardFocusLost>      ← 只声明通道，不自产
                          │
      First stage:        │
        hold_tick_system ─┴─► 推进 held_for / held_frames（排在 time_system 之后）
                          │
      PreUpdate / InputSystems:
        keyboard_input_system ────────┐
        mouse_button_input_system ────┤
        accumulate_mouse_motion_sys ──┼──► ButtonInput<KeyCode>
        accumulate_mouse_scroll_sys ──┘    ButtonInput<Key>
                                           ButtonInput<MouseButton>
                                             （含 held_for / held_frames）
                                           AccumulatedMouseMotion / AccumulatedMouseScroll
                          │
      common_conditions:  input_pressed / input_just_pressed /
                          input_just_released / input_toggle_active
                          │
                          ▼
  消费方：KairosGame（WASD）、未来的游戏代码
```

### 归属表

| 部件 | 归属 | 与 bevy 的对应 |
|---|---|---|
| `ButtonInput<T>`（含时长/帧数）、消息类型、`KeyCode`/`Key`/`MouseButton`、消费系统、`common_conditions`、`install(world, first, pre_update)` | **`kairos_input`**（新 crate） | `bevy_input` |
| `convert_physical_key_code` / `convert_logical_key` / `convert_mouse_*` 转换 | **`kairos_engine`**（adapter 内部） | `bevy_winit/src/converters.rs` |
| winit 事件 → `write_message` 的 adapter | **`kairos_engine`**（`kairos_editor`，egui-free 可复用单元） | `bevy_winit/src/state.rs::forward_bevy_events` |
| 闸口 `InputRouting` 的**写方** | **`kairos_engine`** 的 UI 层（egui 感知） | 无对位物（bevy 用 `bevy_ui` focus + `bevy_input_focus`） |
| stage label（`First` / `PreUpdate`） | `kairos_engine` 的 `schedule.rs`，**由调用方 `install` 传入** | 照 ADR 0003 惯例 |

**依赖方向**：`kairos_input ← kairos_ecs (+ kairos_math)`；`kairos_engine → kairos_input`。`kairos_input` **不可**依赖 engine（stage label 的家），照 ADR 0003 由 `install(world, first, pre_update)` 注入 label。

---

## 3. 阶段

每个阶段以「**编译 + 测试绿的可提交状态**」结束。不变量：**任何一阶段结束时编辑器可跑**。

### Phase 0 — 消息轨道（前置，独立价值）

**目标**：让 `kairos_ecs` 的 `Messages` 真正通电。这是整个 input 架构的地基。

**为什么必须先做**：`kairos_ecs` 的消息机制全在，但 engine 从没接线。没有这一步，Phase 4 的 adapter 写进去的消息**永远不会被更新、永远读不到**。

**交付物**：
1. `World` 扩展 trait（对位 bevy 的 `App::add_message`）：`add_message::<M>()` → 内部 `MessageRegistry::register_message::<M>(&mut World)`
2. 接线点在 **`kairos_engine/src/kairos_editor/schedule.rs::install`**（该函数现为 `pub(crate)`，`:245`；`time_system` 在 `:258` 被挂进 `first`）：**`First` 挂 `message_update_system`**（须排在 `time_system` 附近并与之定序）；新增 `FixedPostUpdate` schedule + `signal_message_update_system`；`MessageRegistry` 初值 `Waiting`
3. 顺手清掉源文件里的拼写错误：`singnal_message_update_system` → `signal_message_update_system`、`message_update_comdition` → `message_update_condition`（`kairos_ecs/src/message/update.rs:27,56`）

**验收**：
- 新增测试：注册一个 dummy message → 系统 A `write` → 系统 B `MessageReader` 读到；**跨帧不重复读**；`Messages::clear`/双缓冲语义正确
- `cargo test-crate kairos_ecs`
- 既有测试全绿（这一步不碰输入，风险隔离）

**风险/协调**：asset effort 的 #190 已决定同一套方案（`First` + `FixedPostUpdate` + `Waiting` + World 扩展 trait）。**实现前先在 #184 留言知会**，避免两边各写一份。若 asset effort 先落地，本阶段退化为「验证已有接线可用」，Phase 1+ 不受影响。

---

### Phase 1 — `kairos_input` crate 骨架 + 纯类型层

**目标**：零系统、零依赖，只有类型。这是「照抄蓝本」最纯粹的一段。

**交付物**（照 `bevy_input` 逐文件对位）：

| 文件 | 内容 | 蓝本 |
|---|---|---|
| `Cargo.toml` | 依赖仅 `kairos_ecs` + `kairos_math`。**不加 winit、不加 egui** | `bevy_input/Cargo.toml:68-86` |
| `button_input.rs` | `ButtonInput<T>`：`pressed`/`just_pressed`/`just_released` 三个 `HashSet<T>` **＋ 新增 `held` 字段**（见 §4） | `button_input.rs:125` |
| `keyboard.rs` | `KeyCode`（**195 个变体**，`KeyCode` 是 bevy 自有枚举，非 `keyboard_types` 再导出）、`Key`（`Character(SmolStr)` / `Unidentified(NativeKey)` / `Named(..)` / `Dead`）、`KeyboardInput { key_code, logical_key, state, repeat, window, text }`、`KeyboardFocusLost` | `keyboard.rs:275, 816` |
| `mouse.rs` | `MouseButton`、`MouseButtonInput`、`MouseMotion`、`MouseWheel`、`MouseScrollUnit`、`AccumulatedMouseMotion { delta: float2 }`、`AccumulatedMouseScroll { unit, delta }` | `mouse.rs:218, 239` |
| `lib.rs` | `InputPlugin` 等价物 → 但 kairos 无 `App`，落为 **`install(world, first_stage, pre_update_stage)`**；`InputSystems` SystemSet | `lib.rs:108` |

**数量级预期**：`KeyCode` 195 变体 + `Key` 的 `Named` 变体表是大段机械代码。照抄即可，不要「顺手精简」—— 精简等于引入分叉。

**验收**：
- crate 独立编译：`cargo check -p kairos_input`
- `ButtonInput<T>` 单测**整套移植**：`press`/`release`/`release_all`/`clear`/`reset`/迭代器/`clear_just_pressed`/`clear_just_released`（`button_input.rs:293-390` 有现成测试）
- 时长/帧数的单测（§4 的规格表逐行断言）

---

### Phase 2 — 消费系统（`PreUpdate` / `InputSystems`）

**目标**：消息 → 资源。bevy 的消费第一层。

**交付物**：
- `keyboard_input_system` —— 含 `KeyboardFocusLost` → `release_all()`
- `mouse_button_input_system`
- `accumulate_mouse_motion_system` / `accumulate_mouse_scroll_system`
- `install` 里把四个系统挂进 `PreUpdate` 的 `InputSystems` set，并 `init_resource` 四个输入资源

**必须照搬的两个细节**：
1. 消费系统开头用 `bypass_change_detection().clear()`（`kairos_ecs` 的 `DetectChangesMut` 已有）
2. 鼠标累积资源用**赋值**写入（天然每帧归零）

**验收**：
- **headless 测试**（这是消息架构的最大红利 —— **不需要窗口、不需要 winit**）：构造 `World` → `add_message::<KeyboardInput>()` → 直接 `write_message(KeyboardInput{..Pressed})` → 跑一次 `PreUpdate` → 断言 `ButtonInput<KeyCode>::pressed(KeyCode::KeyW)`
- 逐条覆盖：按下 → `just_pressed`；跨帧 → `just_pressed` 归假而 `pressed` 保持；松开 → `just_released`；同帧 press+release 两者皆真；`KeyboardFocusLost` → 全 `just_released`
- `cargo test-crate kairos_input`

---

### Phase 3 — 时长/帧数支柱（本项目的差异化核心）

**目标**：把 `held_for` / `held_frames` 推进起来。**这一阶段没有蓝本可抄。**

**交付物**：
- `ButtonInput<T>` 内部：`held: HashMap<T, Hold>`，`Hold { duration: Duration, frames: u32 }`
- `press()` 时插入 `Hold { duration: ZERO, frames: 1 }`（**甲案初值**）；`release()` / `release_all()` 时移除（或按 §4 决定保留终值）
- **`hold_tick_system`**：`Res<Time>` + `ResMut<ButtonInput<T>>`，对每个 held 项 `duration += Time::delta_time()`、`frames += 1`；写回时 **`bypass_change_detection()`**
- 挂载：`First` stage，**`.after(schedule::time_system)`**（时钟先推进，时长再结算），并在 `InputSystems` **之前**
- 访问器（§4）

**为什么必须是独立系统**：按住的键**不产生消息**。winit 只在操作系统自动重复时发 `repeat: true` 的 `KeyboardInput`（`bevy_winit/src/converters.rs:27`），而那是**操作系统设置的重复率**，不是帧时长 —— 拿它当时钟是错的（首次重复还有延迟）。所以需要一个**每帧无条件跑**的系统。**这一条与「内嵌还是并列」的决定无关，两种方案都逃不掉。**

**验收**（headless，逐行对照 §4 的算术表）：
- 第 1 帧按下 → `held_for == 0`、`held_frames == 1`
- 第 2 帧 → `held_for == delta₂`、`held_frames == 2`
- 第 N 帧 → `held_for == delta₂+…+delta_N`、`held_frames == N`
- 未按住的键 → `held_for == ZERO`、`held_frames == 0`（**这条保证「一处查询」成立**）
- `time_scale = 0.5` → 时长按半速累加；`pause()` → 时长冻结
- **变更检测测试**：仅 tick（无按键变化）的一帧，`ButtonInput<KeyCode>` **不得**被标记为 changed
- `cargo test-crate kairos_input`

---

### Phase 4 — adapter + 闸口（engine 侧）

**目标**：让消息**真的从 winit 流进来**。这是「模块能真跑」的关键一步，也是左边界决定的落点。

**交付物**（`kairos_engine/src/kairos_editor/`）：

1. **转换层**（对位 `bevy_winit/src/converters.rs`）
   - `convert_physical_key_code(winit::keyboard::PhysicalKey) -> KeyCode` —— **约 200 臂的机械 match**（`converters.rs:100+`）。这是本阶段最大的体力活，**照抄，别精简**
   - `convert_logical_key(&winit::keyboard::Key) -> Key`
   - `NativeKeyCode` 转换（macOS/Windows/Xkb/Android 四态）
   - 鼠标/滚轮转换（`MouseWheel` 的 `Line`/`Pixel` 单位要分清）

2. **闸口**
   - `InputRouting { keyboard_to_game: bool, pointer_to_game: bool }`，`Default` = **全转发**（即「默认常开」）
   - 写在 `kairos_input` 还是 engine？→ **engine**（它是编辑器关注点；`kairos_input` 保持纯）。若日后游戏也要用同一机制，再上移

3. **adapter**（egui-free 可复用单元）
   - `WindowEvent → Option<输入消息>` 的纯转换
   - 读 `Res<InputRouting>`；放行则 `world.write_message(..)`
   - **不依赖 egui** —— 打包游戏将来直接复用

4. **闸口的写方**（UI 层，egui 感知）
   - 每帧算 `keyboard_to_game = !ctx.wants_keyboard_input()`
   - `pointer_to_game` 由 GameWindow 的 `Response` 决定

**GameWindow 的接入点（现有代码只差没接）**：
`game_window.rs:134` 已经是 `let (rect, _) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());` —— **Response 被 `_` 丢掉了**。把它接住：
- `response.clicked()` → `response.request_focus()`（让 egui 焦点落到 GameWindow）
- 每帧把 `{ response.hovered(), response.has_focus(), rect }` 发布进 World
- `response.hovered()` **已经正确处理 egui 层叠**（弹窗/菜单在上层时为 false），顺手化解 #27 那类 Area 弹窗层叠问题
- 既有先例可照：`Message::UpdateGameWindowSize` → `GameView.size`（`ui.rs:679`）

**同一处顺带修一个现存 bug**：`runtime.rs:519-524` 读了 `response.repaint` 却**忽略 `response.consumed`**，然后无条件转发 `KeyboardInput` ⇒ **今天在 Inspector 文本框里打字、按 Tab，按键照样灌进游戏输入路径**。改为经闸口判定即修好。

**一帧延迟**：adapter 在帧开始前写消息，而闸口 resource 由上一帧 UI pass 写入 ⇒ 路由有一帧延迟。与 bevy_ui 焦点同形，可接受，但**要写进文档**。

**验收**：
- 编辑器跑起来，在 Inspector 文本框打字**不再**触发游戏输入；点击 GameWindow 后按键**能**进游戏输入
- 免开窗测试：直接对 adapter 喂构造的 `WindowEvent`，断言消息被/未被写入
- `cargo check --workspace --all-targets`（改动了 `kairos_engine`）

---

### Phase 5 — 第一个真消费方（`KairosGame` 的 WASD）

**目标**：让新系统有**真实消费方**，否则整条链路无法在真机上验证。

**交付物**：
- `KairosGame::new` 去掉 `engine.input_engine.registe_input(..)` 四连（`kairos_game.rs:166-180`）
- `KairosGame::update` 改为读 `Res<ButtonInput<KeyCode>>` 的 `pressed(KeyCode::KeyW)` 等
- 若需要，「按住多久」用新的 `held_for` 验证一次（让时长支柱在真机上过一次电）

**验收**：编辑器里跑 `KairosGame`，WASD 行为**与重构前一致**（这是「行为不变」的硬指标）；且新增一处用 `held_for` 的行为（如长按加速）证明时长支柱真的通。

---

### Phase 6 — 旧面退役 + 收尾

**交付物**：
- **删 `kairos_engine/src/inputs.rs`**（154 行死代码）及其 `lib.rs` 的 `pub mod inputs;`
- 删 `KairosEngine::update_keyboard_input`（`kairos_editor.rs:113-115`）—— 其职责已被 adapter 取代
- `common_conditions`：`input_pressed` / `input_just_pressed` / `input_just_released` / `input_toggle_active`（**注意：是 `input_toggle_active`，不是 `input_toggled`**）
- 测试收尾：把 §9 的测试面补齐
- 文档：本文件的「实现状态」回头更新

**验收**：
- 全仓库无 `InputEngine` / `InputState` 残留
- `cargo test-crate kairos_input` + `cargo check --workspace --all-targets`
- 编辑器与 `KairosGame` 人工过一遍（出图 + WASD 可用）

---

## 4. 时长/帧数的完整语义规格

这一节是**唯一没有蓝本可抄**的部分，因此写得最细。它是对 `bevy_input` 的**一处有意分叉**（其余全部照搬）。

### 4.1 访问器面

| 访问器 | 语义 | 未按住时 |
|---|---|---|
| `pressed(k) -> bool` | 照 bevy | `false` |
| `just_pressed(k) -> bool` | 照 bevy | `false` |
| `just_released(k) -> bool` | 照 bevy | `false` |
| **`held_for(k) -> Duration`** | 从按下到现在的累计时长 | **`Duration::ZERO`** |
| **`held_frames(k) -> u32`** | 含按下帧共经历多少帧 | **`0`** |
| **`longer_than(k, d) -> bool`** | `held_for(k) > d` | `false` |

**关键设计点：`held_for` / `held_frames` 自带按住语义**（未按住返回零值），所以「按住超过 0.5 秒」是**一次调用**：

```rust
if keyboard.longer_than(KeyCode::KeyW, Duration::from_millis(500)) { … }
```

**不需要先查 `pressed`。** 两个查询是正交的，不是串联的：

| 你在问 | 查谁 |
|---|---|
| 这一帧刚按下吗 | `just_pressed(k)` |
| 现在按着吗 | `pressed(k)` |
| 按着多久了 / 多少帧 | `held_for(k)` / `held_frames(k)` |
| 按着超过 0.5 秒吗 | `longer_than(k, 0.5s)` |

### 4.2 按下帧结算（甲案）

`hold_tick_system` 住 **`First`**（`time_system` 之后、`PreUpdate` 之前）；`press()` 初值 `duration = ZERO, frames = 1`。

| 帧 | `held_for` | `held_frames` |
|---|---|---|
| 1（刚按下） | `0` | `1` |
| 2 | `delta₂` | `2` |
| N | `delta₂ + … + delta_N` | `N` |

- **时长语义**：从按下到现在**真实流逝**了多久 ⇒ 刚按下确实是 0
- **帧数语义**：「含按下帧，共有几帧被按着」⇒ 按下帧算 1
- **已知代价**：`held_for` 比墙钟少算「按下帧内已流逝的那一小段」，且与 `held_frames` 不成简单倍数。**这是刻意的**：如果改成「帧末结算」，刚按下就已经有时长，长按阈值会在按下帧多算一帧，且 `just_pressed(k) && held_for(k) > ZERO` 在按下帧为真（语义上别扭）
- **附带好处**：tick 在 `First` 与「时钟先推进、时长再结算」天然同构，且与 asset effort 把 `message_update_system` 放 `First` 的决定同构；`PreUpdate` 整个留给输入消费

### 4.3 边界场景（必须在 Phase 3 逐条测到）

| 场景 | 规定行为 |
|---|---|
| **失焦**（`KeyboardFocusLost`） | `release_all()` 一并收尾所有 held 项 —— **两条线同一次 `drain()` 收尾，不可能漂移**。这是内嵌方案相对并列方案的真优势 |
| **同帧 press + release** | 照 bevy：`just_pressed` 与 `just_released` **同时为真**（`press` 先插入、`release` 再移除，两个集合各留一条）。时长：按下帧即松开 ⇒ `held_for == 0`、`held_frames == 1`；松开后是否保留终值见下 |
| **松开后终值** | **建议保留最后一帧的 `held_for`/`held_frames` 直到下一次按下**，这样才能做「按了多久才松手」的判定（如蓄力）。若不做，则松手瞬间信息就丢了。**待定，见 §6** |
| **`time_scale = 0` / `pause()`** | 时长冻结（`Time::delta_time()` 为零）—— 这是**特性**：暂停游戏时长按不该继续计。帧数**是否也跟着停**待定，见 §6 |
| **`clear()` / `reset()`** | 照 bevy 的语义扩展：清边沿集合，**held 是否清空**待定，见 §6 |
| **重按（松手后立刻再按）** | 新的一轮从 `0` / `1` 开始，不累加 |

### 4.4 实现约束（内嵌方案的四个代价及其解药）

| 代价 | 解药 |
|---|---|
| tick 每帧写 `ResMut<ButtonInput<T>>` 会**永久点亮变更检测** | 写回时用 `bypass_change_detection()`（`kairos_ecs` 的 `DetectChangesMut` 已有）。**必须在 Phase 3 就加，并写测试锁住** |
| `press`/`release`/`release_all` 是 `pub`，不变量外溢 | **由类型内部维护**：`press` 建 `Hold`、`release`/`release_all` 拆 `Hold`。外部调用者无需知道 `Hold` 存在 |
| 独占写与同资源消费者互斥 | **基本不成立**：tick 在 `First`、消费者在 `Update`，中间隔着 `PreUpdate → Update` 的**调度边界（屏障）**，不存在争用 |
| 与 `bevy_input::ButtonInput<T>` 分叉 | 立 ADR 记录（§8）—— 这是本次重构的**核心价值主张**，不是意外 |

---

## 5. 与 asset effort（#184）的接口

| 交叉点 | 约定 |
|---|---|
| 消息轨道接线 | #190 已决定的方案与本路线图 Phase 0 **一致**（`First` 挂 `message_update_system`、新增 `FixedPostUpdate` + `signal_message_update_system`、registry 初值 `Waiting`、`World` 扩展 trait 对位 `App`）。**实现前先在 #184 留言知会**，避免两边各写一份 |
| `FixedPostUpdate` | Phase 0 会新增它（asset 也需要）。两个 effort 谁先落地，另一个只需验证 |
| `install` 惯例 | 照 ADR 0003：stage label 由调用方传入，`MainScheduleOrder` 不动。input 的 `install(world, first, pre_update)` 同构 |
| 引导顺序 | `schedule::install` → `kairos_input::install` → 其余。**待定**：input 是否要排在 asset/physics/graphics 之前（倾向是，因为 Phase 0 的接线在 `schedule::install` 内，input 只是挂系统） |

---

## 6. 待定项（实现到该阶段时必须先定）

1. **`Tab` 的路由**：egui 官方注释明说 *"this will always be `true` for tabs"* —— 按 `Tab` 时 egui 必定消费它。GameWindow 聚焦时 `Tab` 该不该进游戏输入？（倾向：不进，保持 egui 焦点导航语义）
2. **固定步长时间线**：`FixedUpdate` 里的物理要「按住多久」时，读到的是**帧线**的值（tick 在 `First`，一个 `FixedUpdate` 帧内不推进）。将来是否需要第二条固定步长线（`FixedHold`）？**倾向**：先只做帧线，需要时再出独立资源
3. **松开后是否保留终值**（§4.3）：影响能否做「按了多久才松手」的蓄力类判定
4. **`pause()` 时帧数是否冻结**（§4.3）：时长时间线受 `time_scale` 影响是明确的，帧数是否跟停需定
5. **`clear()` / `reset()` 是否清 held**（§4.3）
6. **闸口 `InputRouting` 的宿主**：本路线图放 engine；若证明游戏也需要，再上移到 `kairos_input`（**不要现在就上移**）
7. **多窗口**：bevy 的 `KeyboardInput` 带 `window: Entity`，`ButtonInput<KeyCode>` 是全局的。编辑器只有一窗，但 GameWindow 是 egui 区域的**伪窗口** —— 若将来要「GameWindow 失焦才清键」，需要按 window 分流。**倾向**：暂不做

---

## 7. Out of scope

- **touch / gestures / gamepad 支柱** —— 桌面编辑器无消费方；gamepad 另需 gilrs 后端。**登记为后续 effort**
- **`SceneViewInput` 的迁移** —— 编辑器相机/视口交互接入新输入系统，触及 egui 交互模型与 gizmos 拖拽。本路线图**不动它**，改动面留在后续
- **独立游戏运行时（打包形态）** —— 需要自己的 App 引导、窗口管理、无 egui 渲染路径。本路线图只保证 adapter 写成 egui-free 可复用 + 闸口默认值机制，使将来接入零成本
- **焦点模型 / `bevy_input_focus` 对位物** —— 需要先有一个焦点模型（面板级路由），是独立 effort
- **`bevy_input::axis::Axis`** —— 2D 方向辅助类型，无当前消费方
- **IME / `text` 字段的实际消费** —— `KeyboardInput` 结构体会带上 `text` 与 IME 相关消息，但消费留白

---

## 8. 待立 ADR

| 主题 | 要点 |
|---|---|
| **时长/帧数内嵌 `ButtonInput<T>`（对 bevy 的有意分叉）** | 三个 `HashSet` + `held: HashMap<T, Hold{duration, frames}>`；`press/release/release_all` 内部维护不变量；tick 在 `First`；`bypass_change_detection` 是硬约束；按下帧 `held_for = 0`、`held_frames = 1` |
| **输入闸口（Gate）** | editor 端一道闸口；引擎层通用设施、默认常开；打包游戏无人关它 ⇒ 等价于无闸口。用**状态查询**（`wants_keyboard_input` / `Response`）而非逐事件 `consumed`，否则会吃掉「聚焦 GameWindow」那一下 |
| **`kairos_input` 的 crate 边界与 stage 注入** | 纯 crate（零 winit/egui）；`install(world, first, pre_update)` 由调用方传 label（照 ADR 0003）；转换层归 engine |

`CONTEXT.md`：`kairos_input` 建 crate 时同步建 `kairos_input/CONTEXT.md`，把 **闸口 / held_for / held_frames / InputSystems** 四个术语定下来，并更新 `CONTEXT-MAP.md`。

---

## 9. 验证策略

**消息架构最大的红利：输入可以 headless 测。** 不需要窗口、不需要 winit、不需要人工点击 —— 直接往 `World` 里写消息，跑一帧，断言资源。

```
构造 World → add_message::<KeyboardInput>() → write_message(Pressed KeyW)
  → run PreUpdate → assert ButtonInput<KeyCode>::pressed(KeyCode::KeyW)
```

测试面清单：

| 层 | 测什么 | 在哪 |
|---|---|---|
| 纯类型 | `ButtonInput<T>` 状态机全套（照搬 bevy 既有测试）+ 时长/帧数算术表 | `kairos_input` 单测 |
| 消费系统 | 消息 → 资源，逐条：按下/跨帧/松开/同帧双真/失焦清空 | `kairos_input` 单测 |
| 时长支柱 | §4.2 算术表逐行 + `time_scale`/`pause` + **变更检测不被点亮** | `kairos_input` 单测 |
| adapter | 构造 `WindowEvent` → 断言消息被/未被写入；闸口开/关两态 | `kairos_engine` 单测 |
| 端到端 | 编辑器里 Inspector 打字不进游戏输入；GameWindow 聚焦后按键进游戏 | **人工**（无自动化路径） |

**门禁**（照 `docs/agents/testing.md`）：`cargo test-crate <crate>`；改了共享 crate 加 `cargo check --workspace --all-targets`；**不要**跑裸 `cargo test` 或 `cargo test-full`。

---

## 10. 一页速查：从零到通的顺序

```
Phase 0  消息轨道通电          World::add_message + First 挂 message_update_system
   │                          ⇒ 验：dummy message 跨系统可读、跨帧不重读
   ▼
Phase 1  kairos_input 骨架     纯类型：ButtonInput/KeyCode/Key/MouseButton/消息类型
   │                          ⇒ 验：crate 独立编译 + 状态机单测
   ▼
Phase 2  消费系统              PreUpdate/InputSystems：消息 → 资源
   │                          ⇒ 验：headless 写消息 → 断言资源
   ▼
Phase 3  时长/帧数支柱          hold_tick_system 住 First + held_for/held_frames
   │                          ⇒ 验：§4.2 算术表逐行 + 变更检测不被点亮
   ▼
Phase 4  adapter + 闸口        convert_physical_key_code(200 臂) + InputRouting
   │                          + GameWindow Response 接住 + 修 consumed bug
   │                          ⇒ 验：真机打字不串、点 GameWindow 后按键进游戏
   ▼
Phase 5  第一个真消费方         KairosGame WASD 改读 ButtonInput<KeyCode>
   │                          ⇒ 验：行为不变 + 一处 held_for 真机通电
   ▼
Phase 6  旧面退役             删 inputs.rs / 删 update_keyboard_input / common_conditions
                              ⇒ 验：无残留 + 编辑器与游戏人工过一遍
```
