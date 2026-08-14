# MaterialX 本地构建与 GLSL / WebGL Shader 查看指南

> 本文档记录在 macOS（Apple M4 Pro）上从源码构建 MaterialX、运行 MaterialX Viewer / Graph Editor
> 查看 **Open Chess Set** 场景的完整过程，并深入讲解 **如何查看 MaterialX 生成的 GLSL / WebGL (ESSL) shader**，
> 以及渲染体系、场景结构、颜色管理等相关知识。
>
> - 构建版本：MaterialX **v1.39.6**（`main` 分支，commit `a7b2d60a`）
> - 构建日期：2026-08-14
> - 源码位置：`MaterialX/`（本仓库根目录下，即 `/Users/baiaoxiang/OpenPBR/MaterialX`）

---

## 目录

1. [任务概览与结果](#1-任务概览与结果)
2. [环境与依赖](#2-环境与依赖)
3. [获取源码](#3-获取源码)
4. [本地构建](#4-本地构建)
5. [运行 MaterialX Viewer 查看 Open Chess Set](#5-运行-materialx-viewer-查看-open-chess-set)
6. [运行 MaterialX Graph Editor](#6-运行-materialx-graph-editor)
7. [MaterialX 着色器生成体系](#7-materialx-着色器生成体系)
8. [如何查看生成的 GLSL / WebGL Shader（重点）](#8-如何查看生成的-glsl--webgl-shader重点)
9. [Open Chess Set 场景剖析](#9-open-chess-set-场景剖析)
10. [其他值得了解的内容](#10-其他值得了解的内容)
11. [故障排查](#11-故障排查)

---

## 1. 任务概览与结果

| 任务 | 结果 | 说明 |
| --- | --- | --- |
| 克隆 MaterialX | ✅ | `git clone` + 3 个子模块（NanoGUI、ImGui、ImGuiNodeEditor） |
| 本地编译 | ✅ | Ninja + CMake，Release，产物在 `MaterialX/build/` |
| 运行 MaterialX Viewer | ✅ | 已加载 Open Chess Set 场景（chess_set.glb + 象棋材质） |
| 运行 MaterialX Graph Editor | ✅ | 已加载象棋材质节点图 |
| 生成 GLSL shader | ✅ | `build/captures/shader_gen_glsl/*.frag / *.vert`（15 个材质） |
| 生成 WebGL (ESSL) shader | ✅ | `build/captures/shader_gen_essl/*.essl.frag / *.essl.vert` |
| 编写本文档 | ✅ | 即本文 |

**当前运行中的进程**（交互窗口已打开）：

```sh
MaterialXView        PID 18489   # 查看 Open Chess Set 场景
MaterialXGraphEditor PID 18490   # 编辑象棋材质节点图
```

**关键产物位置：**

| 产物 | 路径 |
| --- | --- |
| Viewer 可执行文件 | `MaterialX/build/bin/MaterialXView` |
| Graph Editor 可执行文件 | `MaterialX/build/bin/MaterialXGraphEditor` |
| C++ 静态库 | `MaterialX/build/lib/libMaterialX*.a` |
| Python 绑定（.so） | `MaterialX/build/lib/PyMaterialX*.so` |
| 安装目录（含 Python 包） | `MaterialX/build/install/` |
| Open Chess Set 截图 | `MaterialX/build/captures/chess_set_002.png` |
| 生成的 GLSL shader | `MaterialX/build/captures/shader_gen_glsl/` |
| 生成的 ESSL shader | `MaterialX/build/captures/shader_gen_essl/` |

![Open Chess Set 渲染截图](MaterialX/build/captures/chess_set_002.png)

---

## 2. 环境与依赖

| 项 | 值 |
| --- | --- |
| 操作系统 | macOS 15.3.1（Apple M4 Pro，arm64） |
| Xcode | `/Applications/Xcode.app`（Apple clang 17） |
| CMake | 3.28.3（Homebrew，x86_64） |
| Ninja | Homebrew |
| Qt | 6.9.3（Homebrew `qt` / `qt@6`） |
| Homebrew 位置 | `/usr/local`（**Intel x86_64 版本，运行在 Rosetta 下**） |
| Python | `/usr/local/bin/python3.14`（x86_64，用于加载 Python 绑定）；系统默认 `python3` 是 arm64 |

> ⚠️ **本机最关键的一个环境事实**：这台 M4 Mac 上配置的 Homebrew 位于 `/usr/local`，
> 是 **x86_64（Intel）** 版本（`cmake`、`ninja`、`qt` 都是 x86_64 二进制，经 Rosetta 2 运行）。
> 因此：
>
> - 构建时**必须**指定 `-DCMAKE_OSX_ARCHITECTURES=x86_64`，否则 clang 默认产出 arm64，
>   链接 x86_64 的 Qt 会失败；
> - 生成的 Python 绑定 `.so` 是 x86_64，**必须用 x86_64 的 Python**（`/usr/local/bin/python3.14`）
>   加载，系统自带 arm64 的 `python3` 会报 `incompatible architecture`。

---

## 3. 获取源码

```sh
cd /Users/baiaoxiang/OpenPBR
git clone https://github.com/AcademySoftwareFoundation/MaterialX.git
cd MaterialX

# Viewer / Graph Editor 依赖的第三方子模块必须初始化：
git submodule update --init --recursive
```

子模块清单（`.gitmodules`）：

| 子模块 | 用途 |
| --- | --- |
| `source/MaterialXView/NanoGUI` | Viewer 的 GUI 框架（内含 glfw / nanovg / nanobind 嵌套子模块） |
| `source/MaterialXGraphEditor/External/ImGui` | Graph Editor 的 GUI 框架 |
| `source/MaterialXGraphEditor/External/ImGuiNodeEditor` | 节点图编辑器 |

---

## 4. 本地构建

### 4.1 CMake 配置

在仓库根目录执行（本机实测通过的命令）：

```sh
cd /Users/baiaoxiang/OpenPBR/MaterialX

cmake -S . -B build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_OSX_ARCHITECTURES=x86_64 \
  -DCMAKE_PREFIX_PATH=/usr/local/opt/qt@6 \
  -DMATERIALX_BUILD_VIEWER=ON \
  -DMATERIALX_BUILD_GRAPH_EDITOR=ON \
  -DMATERIALX_BUILD_PYTHON=ON \
  -DUSE_OPENGL_BACKEND_ON_APPLE_PLATFORM=ON
```

### 4.2 关键 CMake 选项

| 选项 | 默认 | 说明 |
| --- | --- | --- |
| `MATERIALX_BUILD_VIEWER` | OFF | 构建 MaterialX Viewer |
| `MATERIALX_BUILD_GRAPH_EDITOR` | OFF | 构建 MaterialX Graph Editor |
| `MATERIALX_BUILD_PYTHON` | OFF | 构建 Python 绑定（用于脚本化 shader 生成，见第 8 节） |
| `MATERIALX_BUILD_GEN_GLSL` | ON | GLSL / ESSL（WebGL）shader 生成器后端 |
| `MATERIALX_BUILD_GEN_OSL / MDL / MSL / SLANG` | ON | 其他语言的生成器后端 |
| `USE_OPENGL_BACKEND_ON_APPLE_PLATFORM` | OFF | **macOS 上强制 OpenGL 渲染后端**（见下） |
| `MATERIALX_BUILD_OIIO` | OFF | 集成 OpenImageIO（EXR/TIFF 支持），本机未启用 |
| `MATERIALX_BUILD_OCIO` | OFF | 集成 OpenColorIO 颜色管理，本机未启用 |
| `MATERIALX_BUILD_TESTS` | OFF | 单元测试（含 shader 渲染验证测试） |
| `MATERIALX_BUILD_DOCS` | OFF | Doxygen API 文档 |

> 🎯 **为什么必须开 `USE_OPENGL_BACKEND_ON_APPLE_PLATFORM`？**
>
> 在 macOS 上，MaterialX 默认选用 **Metal** 渲染后端（NanoGUI 的 Metal 后端），此时
> `MaterialXView` 内部生成的是 **MSL**（Metal Shading Language）shader，而不是 GLSL！
> 源码 `Viewer.cpp` 中：
>
> ```cpp
> #ifndef MATERIALXVIEW_METAL_BACKEND
>     _genContext(mx::GlslShaderGenerator::create(_typeSystem)),       // GLSL（桌面 OpenGL）
>     _genContextEssl(mx::EsslShaderGenerator::create(_typeSystem)),   // ESSL（WebGL / OpenGL ES）
> #else
>     _genContext(mx::MslShaderGenerator::create(_typeSystem)),        // MSL（Metal）
> #endif
> ```
>
> 要查看 **GLSL / WebGL** shader（本文第 8 节的核心），必须用 OpenGL 后端。
> 本机（macOS 15 + OpenGL 4.1，Intel Homebrew 工具链）下该后端运行正常。

### 4.3 构建与安装

```sh
cmake --build build          # 全部目标，含 Viewer、Graph Editor、Python 绑定
cmake --install build --prefix build/install
```

说明：

- 全量构建约 246 个目标，本机用时约 5~8 分钟。
- `cmake --install` 会把可执行文件、库、Python 包、`libraries/`、`resources/` 一起拷到
  `build/install/`。安装阶段**唯一失败**的是"把 Python 包 pip 安装进环境"这一步
  （离线环境下 pip 构建 wheel 失败），**完全不影响使用**——直接用
  `PYTHONPATH=build/install/python` 即可加载 `MaterialX` Python 包。

### 4.4 验证产物

```sh
file build/bin/MaterialXView build/bin/MaterialXGraphEditor
# 输出：Mach-O 64-bit executable x86_64

build/bin/MaterialXView --help   # 打印全部命令行选项
```

---

## 5. 运行 MaterialX Viewer 查看 Open Chess Set

### 5.1 命令行方式启动（推荐）

```sh
cd /Users/baiaoxiang/OpenPBR/MaterialX

build/bin/MaterialXView \
  --mesh resources/Geometry/chess_set.glb \
  --material resources/Materials/Examples/StandardSurface/standard_surface_chess_set.mtlx \
  --envRad resources/Lights/san_giuseppe_bridge.hdr
```

启动后会出现一个窗口，显示 **Open Chess Set**：棋盘与全套黑/白棋子，每个棋子使用独立的
`standard_surface` 材质，自动按材质文档中的 `look` 分配到对应几何组。

> 注：Viewer 会从可执行文件位置向上查找 `libraries/targets/` 来定位标准库，因此从仓库根目录
> 启动即可；若从其他目录启动，加 `--path /Users/baiaoxiang/OpenPBR/MaterialX`。

### 5.2 常用命令行选项

| 选项 | 说明 |
| --- | --- |
| `--material [mtlx]` | 加载的材质文档（含 look 时会自动按几何名分配材质） |
| `--mesh [obj/glb]` | 网格文件（OBJ 或 glTF/glb） |
| `--envRad [hdr]` | 环境光（lat-long HDR 格式），如 `resources/Lights/san_giuseppe_bridge.hdr` |
| `--envMethod [0/1]` | 0 = 滤波重要性采样（FIS），1 = 预滤波环境贴图 |
| `--envSampleCount [n]` | 环境采样数（默认 16） |
| `--meshScale [f]` / `--meshRotation [x,y,z]` | 网格缩放 / 旋转（度） |
| `--cameraPosition [x,y,z]` | 相机位置（默认 0,0,5） |
| `--cameraTarget [x,y,z]` / `--cameraViewAngle [f]` | 相机朝向 / 视角（0 = 正交） |
| `--screenWidth/W` / `--screenHeight/H` | 窗口尺寸（默认 1280×960） |
| `--path [dir]` | 追加数据搜索路径（查找 libraries、贴图、XInclude 用） |
| `--library [dir]` | 追加数据库目录（如自定义材质库） |
| `--captureFilename [png]` | 渲染第一帧后保存 PNG **并自动退出**（可用于无人值守验证） |
| `--refresh [ms]` | 刷新周期（默认 50，-1 禁用） |
| `--drawEnvironment` | 把环境图渲染为背景 |
| `--help` | 全部选项 |

### 5.3 无人值守验证（截图）

本仓库已用它生成过验证截图：

```sh
build/bin/MaterialXView \
  --mesh resources/Geometry/chess_set.glb \
  --material resources/Materials/Examples/StandardSurface/standard_surface_chess_set.mtlx \
  --envRad resources/Lights/san_giuseppe_bridge.hdr \
  --captureFilename build/captures/chess_set_002.png
```

`--captureFilename` 模式渲染第一帧即保存并退出（见 `Main.cpp`：`requestFrameCapture` +
`draw_all` + `requestExit`），适合 CI / 文档验证。

### 5.4 常用快捷键

| 键 | 功能 | 键 | 功能 |
| --- | --- | --- | --- |
| `G` | **保存当前材质的 GLSL shader 源码**（`*_vs.glsl` / `*_ps.glsl`） | `E` | **保存 ESSL（WebGL）shader 源码**（`*_essl_vs.glsl` / `*_essl_ps.glsl`） |
| `O` | 保存 OSL shader 源码 | `M` | 保存 MDL shader 源码 |
| `L` | 从文件加载 GLSL 源码（可编辑后调试） | `R` | 重新加载当前材质（`SHIFT+R` 连同标准库重载） |
| `D` | 把节点图导出为 DOT（Graphviz）文件 | `T` | 把材质翻译为其他 shading model |
| `F` | 截图保存 | `W` | Wedge 渲染（参数渐变测试） |
| `B` | 烘焙材质为纹理 | `←/→` | 切换上/下一个材质 |
| `↑/↓` | 切换几何体 | `+/-` | 相机缩放 |

> 保存的 shader 文件位置 = 材质文件路径去掉扩展名 + `_vs.glsl`/`_ps.glsl`；
> 也可通过环境变量 `MATERIALX_VIEW_OUTPUT_PATH` 重定向到指定目录。

---

## 6. 运行 MaterialX Graph Editor

```sh
cd /Users/baiaoxiang/OpenPBR/MaterialX

build/bin/MaterialXGraphEditor \
  --material resources/Materials/Examples/StandardSurface/standard_surface_chess_set.mtlx \
  --mesh resources/Geometry/chess_set.glb
```

功能简介：

- 以**节点图**方式可视化 / 编辑 MTLX 文档（类似 Blueprints 的可拖拽画布，基于 ImGui + ImGuiNodeEditor）；
- 左上角可切换当前编辑的材质（M_Chessboard、M_Bishop_B ……），中间是节点图，右侧/底部有
  属性面板与 3D 预览（`RenderView`）；
- 常用选项：`--uiScale`、`--fontSize`、`--captureFilename`、`--previewWidth`、
  `--pinsOnBorder`、`--pinShape [circle|flow]`；
- 节点图可直接保存为 MTLX。

---

## 7. MaterialX 着色器生成体系

理解"如何查看 shader"之前，先了解 shader 是怎么来的。MaterialX 不是把 shader 写死在代码里，
而是**在运行时根据 MTLX 文档动态生成**：

```
MTLX 文档 (节点图: image → standard_surface → surfacematerial)
        │
        ▼
ShaderGenerator（按 target 选择生成器）
   - GlslShaderGenerator   target="glsl"    → #version 400（桌面 OpenGL）
   - EsslShaderGenerator   target="essl"    → #version 300 es（WebGL 2.0）
   - VkShaderGenerator     target="vulkan"  → Vulkan GLSL
   - WgslShaderGenerator   target="wgsl"    → WebGPU WGSL
   - MslShaderGenerator    target="msl"     → Metal
   - OslShaderGenerator    target="osl"     → OSL（离线渲染器）
   - MdlShaderGenerator    target="mdl"     → MDL（NVIDIA）
   - SlangShaderGenerator  target="slang"   → Slang
        │  GenContext（GenOptions + 颜色管理 + 单位系统 + 用户数据）
        ▼
Shader（含多个 ShaderStage：VERTEX / PIXEL / ...）
        │  shader.getSourceCode(Stage::VERTEX | Stage::PIXEL)
        ▼
GLSL 源码字符串
```

核心 API（C++ 与 Python 绑定一致）：

1. `shadergen.generate(name, element, context)` —— 对一个**可渲染元素**
   （material/surfaceshader/lightshader/volume 等，`findRenderableElements(doc)` 可列出）生成 `Shader`；
2. `shader.getSourceCode(Stage::PIXEL)` / `Stage::VERTEX` —— 取像素（片元）与顶点 shader 源码；
3. 节点实现的查找：文档里每个节点（如 `image`、`standard_surface`）对应一个 `nodedef`，
   生成器通过 `nodedef` 找到它的**实现**——`libraries/` 里按 target 分目录的 GLSL/OSL/MDL 源码文件：

```
libraries/
├── stdlib/          # 核心节点定义 + 节点图
│   ├── stdlib_defs.mtlx
│   └── genglsl/     # GLSL 实现库：mx_image_color3.glsl, mx_noise*.glsl ...
├── pbrlib/          # PBR 材质模型（standard_surface / usdpreviewsurface / lights）
│   ├── pbrlib_ng.mtlx
│   └── genglsl/     # mx_standard_surface.glsl, mx_dielectric_bsdf.glsl, ...
├── bxdf/            # BxDF 模型与 translation（含 standard_surface ↔ open_pbr_surface）
├── lights/ cmlib/ nprlib/   # 光照、颜色管理、非写实节点
└── targets/         # 每种 target 的入口库：genglsl.mtlx / essl.mtlx / genosl.mtlx ...
```

生成器用 `GenContext.registerSourceCodeSearchPath(...)` 找到这些 `mx_*.glsl` 实现文件，
把**节点的实现函数逐个内联**到最终 shader 中。所以生成的 fragment shader 往往上千行——
它是把整棵节点图"展开"成函数调用的结果。

---

## 8. 如何查看生成的 GLSL / WebGL Shader（重点）

有四种途径，从"零代码"到"完全程序化"，任选：

### 方法 A：Viewer 内快捷键（零代码，最直接）

1. 按第 5.1 节启动 Viewer（加载 chess set 材质）；
2. 选中某个材质（如 `M_Bishop_B`，`←/→` 切换）；
3. 按 **`G`** → 生成 GLSL；按 **`E`** → 生成 ESSL（WebGL）；
4. 文件输出位置：
   - 默认：`resources/Materials/Examples/StandardSurface/standard_surface_chess_set_vs.glsl`
     与 `..._ps.glsl`（即"材质文件路径去掉扩展名 + 后缀"）；
   - 推荐：先 `export MATERIALX_VIEW_OUTPUT_PATH=/tmp/mxshaders`，输出到独立目录；
5. 用任意编辑器打开即可查看；`L` 键可加载修改后的 GLSL 回去调试（热改 shader）。

### 方法 B：官方 Python 脚本（批量生成，本仓库已实测）

MaterialX 自带 `python/Scripts/generateshader.py`，可为文档内**所有**可渲染元素生成
`glsl` / `essl` / `vulkan` / `wgsl` / `msl` / `osl` / `mdl` / `slang` 目标：

```sh
cd /Users/baiaoxiang/OpenPBR/MaterialX

# GLSL（桌面 OpenGL）
PYTHONPATH=build/install/python /usr/local/bin/python3.14 \
  python/Scripts/generateshader.py \
  --path /Users/baiaoxiang/OpenPBR/MaterialX \
  --target glsl \
  --outputPath build/captures/shader_gen_glsl \
  resources/Materials/Examples/StandardSurface/standard_surface_chess_set.mtlx

# ESSL（WebGL 2.0）
PYTHONPATH=build/install/python /usr/local/bin/python3.14 \
  python/Scripts/generateshader.py \
  --target essl \
  --outputPath build/captures/shader_gen_essl \
  resources/Materials/Examples/StandardSurface/standard_surface_chess_set.mtlx
```

产物示例（chess set 共 15 个材质，每个 2 个文件）：

```
build/captures/shader_gen_glsl/M_Bishop_B.glsl.frag   # 像素(片元) shader，约 2100 行
build/captures/shader_gen_glsl/M_Bishop_B.glsl.vert   # 顶点 shader，84 行
build/captures/shader_gen_essl/M_Bishop_B.essl.frag   # WebGL 像素 shader，#version 300 es
build/captures/shader_gen_essl/M_Bishop_B.essl.vert
```

> 注意：Python 绑定是 x86_64 的，必须用 `/usr/local/bin/python3.14`（x86_64），
> 不能用系统默认的 arm64 `python3`（会报 incompatible architecture，见第 11 节）。

### 方法 C：最小 Python 示例（学习生成 API）

把下面代码存为 `gen_shader.py`（本仓库根目录已放置 `verify_minimal.py`，与下面等价，已验证可运行）：

```python
import MaterialX as mx
import MaterialX.PyMaterialXGenShader as mx_gen_shader
import MaterialX.PyMaterialXGenGlsl as mx_gen_glsl

# 1. 加载标准数据库（libraries/）
stdlib = mx.createDocument()
searchPath = mx.getDefaultDataSearchPath()
mx.loadLibraries(mx.getDefaultDataLibraryFolders(), searchPath, stdlib)

# 2. 加载材质文档（Open Chess Set 材质）
doc = mx.createDocument()
mx.readFromXmlFile(doc, "resources/Materials/Examples/StandardSurface/standard_surface_chess_set.mtlx",
                   searchPath)
doc.setDataLibrary(stdlib)

# 3. 创建 GLSL 生成器与生成上下文
gen = mx_gen_glsl.GlslShaderGenerator.create()
context = mx_gen_shader.GenContext(gen)
context.registerSourceCodeSearchPath(searchPath)   # 查找 libraries/stdlib/genglsl/*.glsl
gen.registerTypeDefs(doc)

# 4. 对第一个可渲染元素生成 shader（如象棋棋盘 M_Chessboard）
elems = mx_gen_shader.findRenderableElements(doc)
print("renderable elements:", [e.getName() for e in elems])
shader = gen.generate("M_Chessboard", elems[0], context)

# 5. 取出源码
print(shader.getSourceCode(mx_gen_shader.VERTEX_STAGE))
print(shader.getSourceCode(mx_gen_shader.PIXEL_STAGE))
```

运行（已验证输出）：

```sh
PYTHONPATH=build/install/python /usr/local/bin/python3.14 \
  /Users/baiaoxiang/OpenPBR/verify_minimal.py
```

换 ESSL 目标只需把第 3 步改为 `mx_gen_glsl.EsslShaderGenerator.create()`。

### 方法 D：直接阅读生成结果（解剖一个真实 shader）

以 `build/captures/shader_gen_glsl/M_Bishop_B.glsl.frag`（像素 shader）为例：

```glsl
#version 400                                   // 目标版本：桌面 GLSL 4.0

// ---------- 内部数据结构 ----------
struct BSDF { vec3 response; vec3 throughput; }; // 双向散射分布函数
#define EDF vec3                                // 发射（发光）
struct VDF { vec3 response; vec3 throughput; }; // 体散射
struct surfaceshader { vec3 color; vec3 transparency; };
#define material surfaceshader                   // 表面着色结果统一为 material

// ---------- Uniform 块 ----------
// PrivateUniforms：由渲染器/引擎填充的"系统"参数
uniform mat4 u_envMatrix = ...;
uniform sampler2D u_envRadiance;                // 环境贴图
uniform float u_envLightIntensity = 1.0;
uniform int u_envRadianceMips = 1;
uniform int u_envRadianceSamples = 16;
uniform sampler2D u_envIrradiance;              // 漫反射卷积
uniform vec3 u_viewPosition;
uniform int u_numActiveLightSources = 0;

// PublicUniforms：材质的公开参数（base color、roughness、specular ...）
// ...（约 2100 行：数学工具函数、mx_* 节点实现、光照循环、
//     NG_standard_surface_surfaceshader_100(...) 整棵标准表面求值函数）

void main()
{
    // 1) 取几何属性（来自顶点着色器）
    vec3 geomprop_Nworld_out1 = normalize(vd.normalWorld);
    vec3 geomprop_Tworld_out1 = normalize(vd.tangentWorld);
    vec2 geomprop_UV0_out1    = vd.texcoord_0.xy;

    // 2) 纹理采样（image 节点 → mx_image_* 实现）
    vec3 diffuse2_out;  mx_image_color3 (diffuse2_file, ..., geomprop_UV0_out1, ..., diffuse2_out);
    float metallic2_out; mx_image_float (metallic2_file, ..., geomprop_UV0_out1, ..., metallic2_out);
    float roughness2_out; mx_image_float(roughness2_file, ..., geomprop_UV0_out1, ..., roughness2_out);
    vec3 normal2_out;   mx_image_vector3(normal2_file, ..., geomprop_UV0_out1, ..., normal2_out);

    // 3) 颜色空间转换（sRGB 贴图 → 线性 rec709）
    vec3 diffuse2_out_cm_out;
    NG_srgb_texture_to_lin_rec709_color3(diffuse2_out, diffuse2_out_cm_out);

    // 4) 法线贴图（tangent space → world space）
    vec3 mtlxnormalmap4_out;
    mx_normalmap_float(normal2_out, ..., geomprop_Nworld_out1, geomprop_Tworld_out1, ..., mtlxnormalmap4_out);

    // 5) 整棵 standard_surface 求值（各 BxDF 分层、混合）
    surfaceshader Bishop_B_out;
    NG_standard_surface_surfaceshader_100(base, diffuse2_out_cm_out, ..., mtlxnormalmap4_out, Bishop_B_out);

    // 6) 输出
    material M_Bishop_B_out = Bishop_B_out;
    out1 = vec4(M_Bishop_B_out.color, 1.0);      // 到 PIXEL 阶段的输出
}
```

顶点 shader（`M_Bishop_B.glsl.vert`，84 行）——**引擎必须提供的顶点输入与 uniform 契约**：

```glsl
#version 400

// PrivateUniforms：模型/相机矩阵
uniform mat4 u_worldMatrix = mat4(1.0);
uniform mat4 u_viewProjectionMatrix = mat4(1.0);
uniform mat4 u_worldInverseTransposeMatrix = mat4(1.0);

// 顶点属性（attribute）——网格必须提供
in vec3 i_position;
in vec3 i_normal;
in vec3 i_tangent;        // 切空间（法线贴图用）
in vec2 i_texcoord_0;     // 第一套 UV

out VertexData {          // 传给像素着色器
    vec3 normalWorld;
    vec3 tangentWorld;
    vec2 texcoord_0;
    vec3 bitangentWorld;
    vec3 positionWorld;
} vd;

void main()
{
    vec4 hPositionWorld = u_worldMatrix * vec4(i_position, 1.0);
    gl_Position = u_viewProjectionMatrix * hPositionWorld;
    vd.normalWorld    = normalize(mx_matrix_mul(u_worldInverseTransposeMatrix, vec4(i_normal, 0.0)).xyz);
    vd.tangentWorld   = normalize(mx_matrix_mul(u_worldMatrix, vec4(i_tangent, 0.0)).xyz);
    vd.texcoord_0     = i_texcoord_0;
    vd.bitangentWorld = cross(vd.normalWorld, vd.tangentWorld);
    vd.positionWorld  = hPositionWorld.xyz;
}
```

### GLSL（桌面）与 ESSL（WebGL）对比

| 项 | `glsl` 目标 | `essl` 目标 |
| --- | --- | --- |
| 生成器 | `GlslShaderGenerator` | `EsslShaderGenerator` |
| 版本指令 | `#version 400` | `#version 300 es` |
| 精度声明 | 无（驱动默认） | 需要 `precision highp float;` |
| 面向平台 | OpenGL 4.0+ 桌面 | **WebGL 2.0**（OpenGL ES 3.0） |
| 代码量 | 与 ESSL 基本一致 | 与 GLSL 基本一致（`u_envRadianceSamples` 等均相同） |
| 取用方式 | Viewer 按 `G`；`--target glsl` | Viewer 按 `E`；`--target essl` |

### 在 WebGL 应用中落地（把 ESSL 用起来）

生成的 `.essl.vert/.essl.frag` 可以直接喂给 WebGL2（`getShaderParameter(COMPILE_STATUS)` 应通过）。
接入一个 WebGL 渲染器需要做到：

1. **顶点属性绑定**：`i_position`(vec3)、`i_normal`(vec3)、`i_tangent`(vec3)、`i_texcoord_0`(vec2)
   —— 按上面顶点 shader 的 `VertexInputs` 绑定；
2. **uniform 填充**：
   - 顶点阶段：`u_worldMatrix`、`u_viewProjectionMatrix`、`u_worldInverseTransposeMatrix`；
   - 像素阶段：`u_viewPosition`、`u_envRadiance`（HDR 环境贴图，`u_envRadianceMips` 级 mipmap）、
     `u_envIrradiance`、`u_numActiveLightSources`，以及材质参数块 `PublicUniforms` 里的全部输入
     （base color、roughness、specular 等——可从材质文档 `property` 读出）；
3. **纹理**：`mx_image_*` 采样的是 `PublicUniforms` 中 `u_<name>_file` 之类的 sampler2D，
   需按材质的 `filename` 属性加载对应贴图并绑定；
4. 环境光方法为 FIS（滤波重要性采样）时，fragment shader 会在 GPU 上做环境采样积分，
   需提供 mipmap 化的 radiance 贴图。

> 更省事的 WebGL 方案：**MaterialX 官方 Web 端（`javascript/` 目录）** 实现了 WebGL 渲染器
> 与 ESSL 生成绑定，可在浏览器直接运行 MTLX（`npm` 构建，本文未展开）。

---

## 9. Open Chess Set 场景剖析

Open Chess Set 是 MaterialX 仓库自带的演示场景（材质由 SideFX 贡献，美术：Moeen & Mujtaba Sayed）。

### 9.1 文件清单

| 文件 | 说明 |
| --- | --- |
| `resources/Geometry/chess_set.glb` | 网格（glTF 二进制，约 35.7 MB），含 15 个几何组 |
| `resources/Materials/Examples/StandardSurface/standard_surface_chess_set.mtlx` | 材质文档 |
| `resources/Materials/Examples/StandardSurface/chess_set/*.jpg` | 贴图（每棋子的 base_color / metallic / roughness / normal + 棋盘贴图） |
| `resources/Lights/san_giuseppe_bridge.hdr` | 环境光（Viewer 文档中的默认环境） |

### 9.2 材质文档结构

`standard_surface_chess_set.mtlx`（约 550 行）包含：

- **15 个 `standard_surface` 材质**：`M_Chessboard`、`M_Bishop_B`、`M_Bishop_W`、`M_Castle_B/W`、
  `M_Knight_B/W`、`M_King_B/W`、`M_Pawn_Body_B/W`、`M_Pawn_Top_B/W`、`M_Queen_B/W`；
- 每个材质用 `image` 节点采样贴图（颜色贴图带 `colorspace="srgb_texture"`，金属/粗糙度/法线为
  线性贴图），再接入 `standard_surface` 的 `base_color`、`metalness`、`roughness`、`normal` 输入；
- **`look` 元素 `L_ChessSet`**：用 `materialassign` 把材质映射到 glb 网格组：

```xml
<look name="L_ChessSet">
  <materialassign name="Chessboard" geom="Chessboard" material="M_Chessboard" />
  <materialassign name="Bishop_B"   geom="Bishop_B"   material="M_Bishop_B" />
  ... <!-- 15 条，geom 与 glb 的网格组一一对应 -->
</look>
```

Viewer 加载含 `look` 的文档时，会把 `materialassign.geom` 与网格组名匹配并自动分配材质。

### 9.3 为什么在渲染测试里被排除

`resources/Materials/TestSuite/_options.mtlx` 的 `renderTestExcludeFiles` 中列出了
`standard_surface_chess_set.mtlx`——因为该场景需要配套的 35MB glb 网格，渲染测试默认只跑
TestSuite 中的流程化材质，大场景单独在 Viewer 里体验。

---

## 10. 其他值得了解的内容

### 10.1 `libraries/` 标准数据库

- `stdlib/`：核心节点定义（图像、纹理坐标、数学、噪声、流程控制……）；
- `pbrlib/`：PBR 材质与光照（`standard_surface`、`usdpreviewsurface`、`light`……）及 GLSL/OSL/MDL/MSL/Slang 实现；
- `bxdf/`：底层 BxDF 模型 + 模型互转（`translation/`，含 `standard_surface` ↔ `open_pbr_surface`）；
- `lights/`、`cmlib/`、`nprlib/`：光照、颜色管理、非写实渲染；
- `targets/`：每个生成目标的后端入口库（`genglsl.mtlx`、`essl.mtlx`、`genosl.mtlx`……）。

### 10.2 颜色管理

- 贴图/材质输入带 `colorspace` 属性（如 `srgb_texture`）；
- 生成时由 `DefaultColorManagementSystem`（按 target 实例化）在 shader 里插入转换函数——
  前面例子中的 `NG_srgb_texture_to_lin_rec709_color3(...)` 就是：**sRGB 纹理 → 线性 rec709** 工作空间；
- 集成了 OpenColorIO（`MATERIALX_BUILD_OCIO=ON`）后可支持自定义色彩空间。

### 10.3 单位系统

- 材质文档里的物理单位（如 `distance`、`angle`）在生成时按 `targetDistanceUnit` 换算，
  例如 `subsurface_scale`、`transmission_depth` 会从 `cm`/`mm` 换到 `meter`。

### 10.4 生成器目标横向对比

| 目标 | 语言 / 平台 | 典型用途 |
| --- | --- | --- |
| `glsl` | GLSL 4.0 桌面 | OpenGL 应用、DCC 工具内嵌预览 |
| `essl` | GLSL ES 3.0 = WebGL 2.0 | 浏览器、移动端 |
| `vulkan` | Vulkan GLSL | Vulkan 应用 |
| `wgsl` | WebGPU | 下一代 Web 渲染 |
| `msl` | Metal | macOS / iOS |
| `osl` | Open Shading Language | 电影级离线渲染器（Arnold、RenderMan、Cycles……） |
| `mdl` | NVIDIA Material Definition Language | Omniverse / Iray 生态 |
| `slang` | Slang | 跨图形 API 的现代着色语言 |

### 10.5 材质翻译（Viewer 的 `T` 键）

Viewer 的 Advanced Settings 里 `Translation Options (T)` 可以把当前材质翻译为其他 shading model
（默认目标 `standard_surface`，可改为 `usdpreviewsurface` 等），用于跨模型比较。
`libraries/bxdf/translation/` 中就有 `standard_surface_to_open_pbr_surface.mtlx` 这样的互转节点图。

### 10.6 测试套件

`resources/Materials/TestSuite/` 是材质渲染回归测试的输入，`source/MaterialXTest/` 是 C++ 单元测试
（`MATERIALX_BUILD_TESTS=ON` 启用），其中 `ShaderValid` 系列会对 TestSuite 逐个生成并校验
GLSL/OSL/MDL shader。`python/Scripts/` 下还有 `translateshader.py`、`baketextures.py`、
`creatematerial.py`、`mxvalidate.py` 等实用脚本。

### 10.7 与 OpenPBR 的关系

- **OpenPBR** 是行业推动的开放 PBR 材质规范（Autodesk / Adobe / Meta 等发起，ASWF 托管），
  本仓库名即来源于此；
- **MaterialX** 是 OpenPBR 规范的主要参考实现载体：当前 MaterialX 主分支的 Oren-Nayar 漫反射
  已按 OpenPBR Surface 的能量补偿版本实现（`mx_microfacet_diffuse.glsl` 源码注释明确引用
  OpenPBR 规范），并提供 `standard_surface` ↔ `open_pbr_surface` 翻译节点；
- 官方规范：<https://academysoftwarefoundation.github.io/OpenPBR/>

### 10.8 参考链接

- MaterialX 仓库：<https://github.com/AcademySoftwareFoundation/MaterialX>
- Viewer 文档：`MaterialX/documents/DeveloperGuide/Viewer.md`
- Graph Editor 文档：`MaterialX/documents/DeveloperGuide/GraphEditor.md`
- Shader 生成文档：`MaterialX/documents/DeveloperGuide/ShaderGeneration.md`
- Python 脚本：`MaterialX/python/Scripts/`（README.md 有说明）

---

## 11. 故障排查

| 现象 | 原因与解决 |
| --- | --- |
| Viewer/Graph Editor 报 `hiservices-xpcservice Connection invalid` 后无法显示窗口 | **终端沙箱阻止 GUI 连接 WindowServer**。必须在**非沙箱**终端中运行（审批提示允许即可），本仓库文档中的启动命令均以非沙箱方式执行 |
| 终端里 `ps` / `pgrep` 报 `Operation not permitted` | 沙箱禁止进程枚举；用 `kill -0 <PID>` 判断进程是否存活 |
| Python `import MaterialX` 报 `incompatible architecture (have 'x86_64', need 'arm64')` | 构建产出的 `.so` 是 x86_64（因链接 x86_64 Qt）。改用 x86_64 Python：`/usr/local/bin/python3.14`（勿用系统 arm64 `python3`） |
| `cmake --install` 时 pip 安装 Python 包失败 | 离线环境 pip 构建 wheel 失败，**不影响使用**；用 `PYTHONPATH=build/install/python` 加载即可 |
| Viewer 日志出现 `GLD_TEXTURE_INDEX_2D is unloadable ... using zero texture` | 无害的驱动级提示（Rosetta 下 OpenGL 兼容层），默认场景也会出现，不影响渲染 |
| 找不到 `libraries/`（`Could not find standard data libraries`） | Viewer 从**可执行文件位置**向上查找 `libraries/targets/`；从仓库根目录启动，或加 `--path /Users/baiaoxiang/OpenPBR/MaterialX` |
| `--captureFilename` 启动后立即退出 | 这是设计行为：渲染第一帧、保存 PNG 后自动退出（`Main.cpp` 中 capture 模式显式 `requestExit`），适合无人值守验证 |
| Metal 后端下按 `G`/`E` 无效 | 当前构建用了 Metal 后端，只生成 MSL。需以 `-DUSE_OPENGL_BACKEND_ON_APPLE_PLATFORM=ON` 重新配置构建 |
| GLSL 与 ESSL 内容几乎一样 | 正常：两者共用同一套实现库，仅版本指令与精度声明不同（`400` vs `300 es`） |
| 构建报 Qt 找不到 | 确认 `-DCMAKE_PREFIX_PATH=/usr/local/opt/qt@6`（Homebrew Qt6 位置） |
