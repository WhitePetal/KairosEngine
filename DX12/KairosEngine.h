#ifndef __KAIROS_ENGINE__
#define __KAIROS_ENGINE__

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN // 从 Windows 头文件中排除极少使用的内容
#endif

#include "KairosEngineDefines.h"
#include <windows.h>
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"
#include <string>

/// <summary>
/// 窗口句柄
/// </summary>
HWND m_Hwnd = NULL;

/// <summary>
/// 窗口名
/// </summary>
LPCTSTR m_WindowName = L"KairosEngine";

/// <summary>
/// 窗口标题
/// </summary>
LPCTSTR m_WindowTitle = L"Kairos Window";

/// <summary>
/// 窗口宽度
/// </summary>
int m_Width = 800;
/// <summary>
/// 窗口高度
/// </summary>
int m_Height = 600;
/// <summary>
/// 窗口是否是全屏状态
/// </summary>
bool m_FullScreen = false;

/// <summary>
/// 当其为False时，我们将退出主循环，然后退出程序
/// </summary>
bool m_Running = true;

/// <summary>
/// 缓冲区数量，2表示双重缓存，3表示三重缓冲
/// </summary>
const int k_FrameBufferCount = 3;

/// <summary>
/// Direct3D device
/// </summary>
ID3D12Device* m_Device;

/// <summary>
/// 用于在渲染目标之间切换的交换链
/// </summary>
IDXGISwapChain3* m_SwapChain;

/// <summary>
/// 命令列表的容器
/// </summary>
ID3D12CommandQueue* m_CommandQueue;

/// <summary>
/// 渲染目标的描述符堆
/// </summary>
ID3D12DescriptorHeap* m_RTVDescroptorHeap;

/// <summary>
/// 渲染目标资源
/// </summary>
ID3D12Resource* m_RenderTargets[k_FrameBufferCount];

/// <summary>
/// 命令分配器
/// 现在我们只有1个线程，因此只需 FrameBufferCount 个命令分配器
/// </summary>
ID3D12CommandAllocator* m_CommandAllocators[k_FrameBufferCount];

/// <summary>
/// 命令列表
/// </summary>
ID3D12GraphicsCommandList* m_CommandList;

/// <summary>
/// 屏障，为每个缓冲区的渲染(每帧)准备一个屏障
/// </summary>
ID3D12Fence* m_Fences[k_FrameBufferCount];

/// <summary>
/// 当我们的屏障被gpu解锁时的一个事件句柄
/// </summary>
HANDLE m_FenceEvent;

/// <summary>
/// 这个值每帧递增。每个屏障都有自己的值
/// </summary>
UINT64 m_FenceValues[k_FrameBufferCount];

/// <summary>
/// 当前RTV索引
/// </summary>
int m_FrameIndex;

/// <summary>
/// 当前设备RTV描述符的大小，所有后台缓冲的大小必须一致
/// </summary>
int m_RTVDescriptorSize;

/// <summary>
/// 管线状态对象
/// </summary>
ID3D12PipelineState* m_PipelineStateObject;

/// <summary>
/// 根签名
/// </summary>
ID3D12RootSignature* m_RootSignature;

/// <summary>
/// 视口
/// </summary>
D3D12_VIEWPORT m_ViewPort;

/// <summary>
/// 剪刀矩形
/// </summary>
D3D12_RECT m_ScissorRect;

/// <summary>
/// 顶点缓冲
/// </summary>
ID3D12Resource* m_VertexBuffer;

/// <summary>
/// 顶点缓冲视图描述符
/// </summary>
D3D12_VERTEX_BUFFER_VIEW m_VertexBufferView;

struct Vertex
{
	DirectX::XMFLOAT3 pos;
};

/// <summary>
/// 创建和初始化窗口
/// </summary>
/// <param name="hInstance"></param>
/// <param name="ShowWnd"></param>
/// <param name="width"></param>
/// <param name="height"></param>
/// <param name="fullScreen"></param>
/// <returns></returns>
bool InitializeWindow(HINSTANCE hInstance, int ShowWnd, int width, int height, bool fullScreen);

/// <summary>
/// 主循环
/// </summary>
void MainLoop();

/// <summary>
/// Windows消息回调函数
/// </summary>
/// <param name="hWnd"></param>
/// <param name="msg"></param>
/// <param name="wParam"></param>
/// <param name="lParam"></param>
/// <returns></returns>
LRESULT CALLBACK WndProc(HWND hWnd, UINT msg, WPARAM wParam, LPARAM lParam);

/// <summary>
/// 初始化DX12
/// </summary>
/// <returns></returns>
bool InitD3D();

/// <summary>
/// Update game logic
/// </summary>
/// <returns></returns>
void Update();

/// <summary>
/// 更新d3d管线(更新命令列表)
/// </summary>
/// <returns></returns>
void UpdatePipeline();

/// <summary>
/// 执行命令列表
/// </summary>
/// <returns></returns>
void Render();

/// <summary>
/// 释放COM对象，释放内存
/// </summary>
/// <returns></returns>
void Cleanup();

/// <summary>
/// 等待GPU完成命令列表
/// </summary>
/// <returns></returns>
void WaitForPreviousFrame();

#endif
