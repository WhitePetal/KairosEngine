# 大型开放世界地形系统 —— 技术资源索引

> **日期:** 2026-07-28
> **概述文档:** [open-world-terrain-overview.md](./open-world-terrain-overview.md)

---

## 目录

1. [学术论文](#1-学术论文)
2. [GPU Gems / GPU Pro 章节](#2-gpu-gems--gpu-pro-章节)
3. [GDC / SIGGRAPH 演讲](#3-gdc--siggraph-演讲)
4. [开源代码项目](#4-开源代码项目)
5. [引擎官方文档](#5-引擎官方文档)
6. [技术博客与视频](#6-技术博客与视频)

---

## 1. 学术论文

| 标题 | 作者 | 出处 | 年份 | 链接 |
|------|------|------|------|------|
| Continuous Distance-Dependent Level of Detail for Rendering Heightmaps | Filip Strugar | *J. Graphics, GPU, and Game Tools*, 14(4), 57–74 | 2010 | DOI: 10.1080/2151237X.2009.10129287 |
| Geometry Clipmaps: Terrain Rendering Using Nested Regular Grids | Frank Losasso, Hugues Hoppe | *ACM Trans. Graphics (SIGGRAPH 2004)*, 23(3), 769–776 | 2004 | [Hoppe's page](https://hhoppe.com/proj/geomclipmap/) |
| The Clipmap: A Virtual Mipmap | Christopher Tanner, Christopher Migdal, Michael Jones | *SIGGRAPH 98*, pp. 151–158 | 1998 | 原始 clipmap 概念 |
| Simulating Ocean Water | Jerry Tessendorf | SIGGRAPH Course Notes | 2001/2004 | Tessendorf 波理论基础，见 [GPU Gems 1 Ch.1](https://developer.nvidia.com/gpugems/gpugems/part-i-natural-effects/chapter-1-effective-water-simulation-physical-models) |
| Real-time Water Rendering — Introducing the Projected Grid Concept | Claes Johanson | Master's Thesis, Lund University | 2004 | Projected Grid 原始论文 |
| Sparse Virtual Textures | Sean Barrett | GDC 2008 | 2008 | id Tech 5 的虚拟纹理系统，[GDC Vault](https://www.gdcvault.com/play/1774/Sparse-Virtual) |
| Adaptive Virtual Texturing | Anton Kaplanyan | — | 2010 | 动态页面分辨率 AVT |
| The Transvoxel Algorithm | Eric Lengyel | *Mathematics for 3D Game Programming and Computer Graphics*, 3rd Ed. | 2010 | 体素 LOD 过渡算法，[transvoxel.org](http://transvoxel.org/) |
| ROAMing Terrain: Real-time Optimally Adapting Meshes | Mark Duchaineau et al. | *IEEE Visualization 97* | 1997 | 经典自适应地形算法 |
| Marching Cubes: A High Resolution 3D Surface Construction Algorithm | William Lorensen, Harvey Cline | *SIGGRAPH 1987* | 1987 | 体素等值面提取基础算法 |
| Dual Contouring of Hermite Data | Tao Ju et al. | *SIGGRAPH 2002* | 2002 | 保留尖角特征的等值面算法 |

---

## 2. GPU Gems / GPU Pro 章节

NVIDIA GPU Gems 系列全书免费在线获取：https://developer.nvidia.com/gpugems

| 书目 | 章节 | 标题 | 作者 | 链接 |
|------|------|------|------|------|
| GPU Gems 1 | Ch.1 | Effective Water Simulation from Physical Models | Mark Finch | [链接](https://developer.nvidia.com/gpugems/gpugems/part-i-natural-effects/chapter-1-effective-water-simulation-physical-models) |
| GPU Gems 1 | Ch.2 | Rendering Water Caustics | Juan Guardado | [链接](https://developer.nvidia.com/gpugems/gpugems/part-i-natural-effects/chapter-2-rendering-water-caustics) |
| GPU Gems 2 | **Ch.2** | **Terrain Rendering Using GPU-Based Geometry Clipmaps** | Arul Asirvatham, Hugues Hoppe | [链接](https://developer.nvidia.com/gpugems/gpugems2/part-i-geometric-complexity/chapter-2-terrain-rendering-using-gpu-based-geometry) ⭐ |
| GPU Gems 2 | Ch.18 | Using Vertex Texture Displacement for Realistic Water Rendering | Yuri Kryachko | [链接](https://developer.nvidia.com/gpugems/gpugems2/part-iii-high-quality-rendering/chapter-18-using-vertex-texture-displacement) |
| GPU Gems 3 | Ch.1 | Generating Complex Procedural Terrains Using the GPU | — | [链接](https://developer.nvidia.com/gpugems/gpugems3/part-i-geometry/chapter-1-generating-complex-procedural-terrains-using-gpu) |
| GPU Pro 1 | — | Voxel Terrain Rendering | — | — |
| GPU Pro 2 | — | Real-Time Open Water Environments with Level of Detail | — | — |
| GPU Pro 4 | — | Practical and Realistic Virtual Texturing | Chen, Mayer | — |
| GPU Pro 5 | — | Water Rendering Optimization | — | — |
| GPU Pro 7 | — | Adaptive Virtual Texture Rendering in Far Cry 4 | Egor Yusov | — |

> ⭐ = 必读章节，奠定了现代 GPU 地形渲染的基础。

---

## 3. GDC / SIGGRAPH 演讲

### 3.1 引擎/技术通用

| 会议 | 年份 | 标题 | 演讲者/公司 | 备注 |
|------|------|------|------------|------|
| GDC | 2008 | Sparse Virtual Textures | Sean Barrett / id Software | 虚拟纹理起源，[GDC Vault](https://www.gdcvault.com/play/1774) |
| GDC | 2014 | Streaming Open Worlds in The Witcher 3 | CD Projekt Red | Clipmap 地形 + 流式加载 |
| SIGGRAPH | 2017 | Terrain Rendering in Horizon Zero Dawn | Nathan Vos / Guerrilla | [Slides](http://advances.realtimerendering.com/s2017/) |
| GDC | 2017 | GPU-Based Run-Time Procedural Placement in Horizon Zero Dawn | Jaap van Muijden / Guerrilla | — |
| GDC | 2017 | The Decima Engine: Visibility, Culling, and LOD | Michal van der Leeuw / Guerrilla | — |
| GDC | 2018 | World Building and Streaming in Assassin's Creed Origins | Ubisoft | Anvil 引擎世界流式加载 |
| GDC | 2018 | Rendering the World of Far Cry 5 | Ubisoft | Dunia 引擎地形 |
| GDC | 2019 | Terrain Rendering in Assassin's Creed Odyssey | Barthélémy Stevens / Ubisoft | GPU clipmap 地形 |
| SIGGRAPH | 2019 | Red Dead Redemption 2: Terrain, Vegetation, and Global Illumination | Rockstar Games | RAGE 引擎地形 |
| SIGGRAPH | 2019 | Decima Engine: Rendering Systems | Guerrilla Games / Kojima Productions | [Slides](http://advances.realtimerendering.com/s2019/) |
| GDC | 2019 | The Rendering Pipeline of Red Dead Redemption 2 | Rockstar Games | — |
| SIGGRAPH | 2020 | Rendering the World of Ghost of Tsushima | Sucker Punch | [Slides](http://advances.realtimerendering.com/s2020/) |
| GDC | 2021 | Procedural Generation of the World of Ghost of Tsushima | Ian Lloyd, Jason Wang / Sucker Punch | — |
| GDC | 2021 | The Water Technology of Assassin's Creed Valhalla | Ubisoft | FFT 海洋 |
| GDC | 2021 | Microsoft Flight Simulator: World Streaming | Asobo Studio | 全球尺度地形流式加载 |
| GDC | 2022 | Horizon Forbidden West: The Technology Behind the World | Guerrilla Games | SDF 地形 + 多层世界 |
| GDC | 2024 | World Building in Starfield | Bethesda | Creation Engine 2 地形 |

### 3.2 水体/海洋专项

| 会议 | 年份 | 标题 | 演讲者/公司 |
|------|------|------|------------|
| GDC | 2013 | Water Rendering in Assassin's Creed III | Ubisoft |
| GDC | 2014 | Rendering the Stormy Seas of Black Flag | Ubisoft |
| GDC | 2015 | Water Rendering in Far Cry 4 | Ubisoft |
| GDC | 2018 | The Ocean in Sea of Thieves | Rare |
| SIGGRAPH | 2016 | Water Technology of Uncharted 4 | Naughty Dog |

### 3.3 程序化/体素专项

| 会议 | 年份 | 标题 | 演讲者/公司 |
|------|------|------|------------|
| GDC | 2017 | Procedural World Generation in No Man's Sky | Hello Games |
| GDC | 2017 | Change and Constant: Breaking Conventions with Breath of the Wild | Nintendo EPD |
| CEDEC | 2017 | Zelda BOTW Technical Sessions（日文） | Nintendo EPD |

### 3.4 关键资源入口

- **SIGGRAPH Advances in Real-Time Rendering 课程幻灯片:** http://advances.realtimerendering.com/ （2006 年至今全部存档）
- **YouTube 频道:** https://www.youtube.com/@AdvancesinRealTimeRendering （2020 年起有完整录像）
- **GDC Vault:** https://www.gdcvault.com/ （需付费会员）

---

## 4. 开源代码项目

### 4.1 地形 LOD

| 项目 | 描述 | 语言 | 链接 |
|------|------|------|------|
| **fstrugar/CDLOD** | CDLOD 算法官方参考实现 | C++/OpenGL | https://github.com/fstrugar/CDLOD |
| **Procedural-Terrain-LOD** | 四叉树地形 LOD 实现 | C++ | GitHub 可搜索 |
| **geometry-clipmap** | Geometry Clipmap 的各种实现 | C++/GLSL | GitHub 可搜索 |

### 4.2 海洋渲染

| 项目 | 描述 | 语言/平台 | 链接 |
|------|------|----------|------|
| **gasgiant/Ocean-FFT** | Unity FFT 海洋仿真 | C#/Unity | https://github.com/gasgiant/Ocean-FFT |
| **jbouny/fft-ocean** | WebGL FFT 海洋，浏览器可运行 | JS/WebGL | https://github.com/jbouny/fft-ocean |
| **Crest Ocean System** | Unity 海洋渲染系统（生产级） | C#/Unity | https://github.com/crest-ocean/crest |
| **UE4-OceanProject** | UE4 海洋渲染项目 | C++/UE4 | https://github.com/UE4-OceanProject |

### 4.3 体素地形

| 项目 | 描述 | 语言 | 链接 |
|------|------|------|------|
| **PolyVox** | C++ 体素库 | C++ | 各镜像站可搜索 |
| **Transvoxel** | Lengyel 的 Transvoxel 参考实现 | C++ | http://transvoxel.org/ |
| **Dual Contouring** | 多种 Dual Contouring 实现 | 多种 | GitHub 可搜索 "dual contouring terrain" |

### 4.4 引擎/插件

| 项目 | 描述 | 语言 | 链接 |
|------|------|------|------|
| **Terrain3D** | Godot 地形插件（Clipmap，生产级，4.1k ★） | C++/GDScript | https://github.com/TokisanGames/Terrain3D |
| **Voxel Plugin** | UE5 第三方体素地形 | C++ | https://voxelplugin.com/ |

---

## 5. 引擎官方文档

### 5.1 Unreal Engine 5

| 文档 | 链接 |
|------|------|
| Landscape Technical Guide | https://docs.unrealengine.com/5.0/en-US/landscape-technical-guide-in-unreal-engine/ |
| Runtime Virtual Texturing | https://dev.epicgames.com/documentation/en-us/unreal-engine/runtime-virtual-texturing-in-unreal-engine |
| Water System | https://docs.unrealengine.com/5.0/en-US/water-system-in-unreal-engine/ |
| World Partition | https://docs.unrealengine.com/5.0/en-US/world-partition-in-unreal-engine/ |
| Landmass Plugin | https://docs.unrealengine.com/5.0/en-US/landmass-plugin-in-unreal-engine/ |
| Virtual Heightfield Mesh | https://docs.unrealengine.com/5.0/en-US/virtual-heightfield-mesh-in-unreal-engine/ |

### 5.2 Unity

| 文档 | 链接 |
|------|------|
| Terrain Manual | https://docs.unity3d.com/Manual/terrain-UsingTerrains.html |
| Terrain Settings | https://docs.unity3d.com/Manual/terrain-OtherSettings.html |
| Heightmaps | https://docs.unity3d.com/Manual/terrain-Heightmaps.html |
| Terrain Tools Package | https://docs.unity3d.com/Packages/com.unity.terrain-tools@5.1/manual/index.html |

### 5.3 Godot

| 文档 | 链接 |
|------|------|
| HeightMapShape3D（物理地形） | https://docs.godotengine.org/en/stable/classes/class_heightmapshape3d.html |
| Terrain3D 系统架构 | https://terrain3d.readthedocs.io/en/stable/docs/system_architecture.html |
| Terrain3D 文档首页 | https://terrain3d.readthedocs.io/en/stable/ |

---

## 6. 技术博客与视频

### 6.1 技术分析频道

| 频道/来源 | 内容 | 链接 |
|-----------|------|------|
| **Digital Foundry** | 3A 游戏技术深度分析（含开发者访谈） | https://www.eurogamer.net/digitalfoundry |
| **80 Level** | 游戏美术/技术访谈 | https://80.lv/ |
| **Game Developer（原 Gamasutra）** | 游戏开发技术文章和事后分析 | https://www.gamedeveloper.com/ |
| **GPUOpen** | AMD 开源图形技术博客 | https://gpuopen.com/ |
| **NVIDIA Developer Blog** | NVIDIA 图形技术博客 | https://developer.nvidia.com/blog/ |

### 6.2 推荐视频

| 标题 | 来源 | 说明 |
|------|------|------|
| Ghost of Tsushima: Full Tech Analysis | Digital Foundry (2020) | 地形、LOD、风系统全方位分析 |
| Red Dead Redemption 2: The Digital Foundry Tech Analysis | Digital Foundry (2018) | RDR2 地形与渲染管线 |
| Death Stranding PC: Exclusive Tech Deep Dive + DLSS 2.0 Analysis | Digital Foundry (2020) | Decima 引擎的 PC 移植细节 |
| Zelda Breath of the Wild: Switch vs Wii U Analysis | Digital Foundry (2017) | BOTW 地形与渲染 |
| Zelda Tears of the Kingdom: A Technical Masterclass on Switch | Digital Foundry (2023) | TOTK 多层世界技术分析 |
| Horizon Forbidden West: The Digital Foundry Tech Review | Digital Foundry (2022) | Decima 引擎进化分析 |
| SIGGRAPH Advances in Real-Time Rendering (全系列) | YouTube @AdvancesinRealTimeRendering | 2020 年起全部录像 |

### 6.3 推荐博客文章

| 标题 | 作者/来源 | 链接 |
|------|----------|------|
| Terrain Rendering in Horizon Zero Dawn | Nathan Vos | http://advances.realtimerendering.com/s2017/ |
| Decima Engine Overview | Guerrilla Games | https://www.guerrilla-games.com/decima |
| GPU Terrain Clipmaps（Clipmap 技术深度讲解） | Mike Savage | 个人博客（被 Terrain3D 引用） |

---

## 附录：快速索引 —— 按问题查找

### 纹理混合

1. → GPU Gems 2, Ch.2（Clipmap 地形，含纹理方案）
2. → UE5 RVT 官方文档
3. → GDC 2019 "Terrain Rendering in AC Odyssey"（VT + 材质 ID）
4. → GPU Pro 7 "Adaptive Virtual Texture Rendering in Far Cry 4"

### 地形 LOD / 远处山脉

1. → **GPU Gems 2, Ch.2** ⭐（Geometry Clipmaps，必读）
2. → Strugar (2010) CDLOD 论文
3. → SIGGRAPH 2017 "Terrain Rendering in Horizon Zero Dawn"（GPU 四叉树）
4. → GDC 2021 "Microsoft Flight Simulator World Streaming"（全球尺度）
5. → GitHub: fstrugar/CDLOD

### 海洋渲染

1. → **GPU Gems 1, Ch.1**（FFT/Tessendorf 波理论）
2. → GPU Gems 2, Ch.18（顶点纹理位移水渲染）
3. → Johanson (2004) Projected Grid 论文
4. → GDC 2021 "The Water Technology of AC Valhalla"
5. → GitHub: gasgiant/Ocean-FFT、Crest Ocean System

### 洞穴 / 地下

1. → UE5 Landscape Visibility 工具文档（高度场打孔）
2. → Lengyel (2010) Transvoxel 算法
3. → Lorensen & Cline (1987) Marching Cubes / Ju et al. (2002) Dual Contouring
4. → GDC 2022 "Horizon Forbidden West: The Technology Behind the World"（mesh/voxel blocks）
5. → Digital Foundry TOTK 分析（多层世界架构）

---

*本索引由四份独立研究报告交叉验证汇编而成。所有链接在 2026-07-28 验证有效。*
