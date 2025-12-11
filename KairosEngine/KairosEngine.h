#ifndef __KAIROS_ENGINE__
#define __KAIROS_ENGINE__

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN // 从 Windows 头文件中排除极少使用的内容
#endif

#include "KairosEngineDefines.h"
#include "d3dx12.h"
#include <Windows.h>
#include <d3d12.h>
#include <dxgi1_4.h>
#include <d3dcompiler.h>
#include <DirectXMath.h>

/// <summary>
/// 窗口句柄
/// </summary>
HWND hwnd = NULL;

/// <summary>
/// 窗口名
/// </summary>
LPCTSTR WindowName = L"KairosEngine";

/// <summary>
/// 窗口标题
/// </summary>
LPCTSTR WindowTitle = L"Kairos Window";

/// <summary>
/// 窗口宽度
/// </summary>
int Width = 800;
/// <summary>
/// 窗口高度
/// </summary>
int Height = 600;
/// <summary>
/// 窗口是否是全屏状态
/// </summary>
bool FullScreen = false;

KAIROS_EXPORT_BEGIN

/// <summary>
/// 创建和初始化窗口
/// </summary>
/// <param name="hInstance"></param>
/// <param name="ShowWnd"></param>
/// <param name="width"></param>
/// <param name="height"></param>
/// <param name="fullScreen"></param>
/// <returns></returns>
KAIROS_API bool InitializeWindow(HINSTANCE hInstance, int ShowWnd, int width, int height, bool fullScreen);

/// <summary>
/// 主循环
/// </summary>
KAIROS_API void MainLoop();

/// <summary>
/// Windows消息回调函数
/// </summary>
/// <param name="hWnd"></param>
/// <param name="msg"></param>
/// <param name="wParam"></param>
/// <param name="lParam"></param>
/// <returns></returns>
KAIROS_API LRESULT CALLBACK WndProc(HWND hWnd, UINT msg, WPARAM wParam, LPARAM lParam);

KAIROS_EXPORT_END

#endif
