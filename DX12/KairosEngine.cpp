#include "KairosEngine.h"




//int WINAPI WinMain(HINSTANCE hInstance,    //Main windows function
//	HINSTANCE hPrevInstance,
//	LPSTR lpCmdLine,
//	int nShowCmd)
//{
//	// 创建窗口
//	if (!InitializeWindow(hInstance, nShowCmd, m_Width, m_Height, m_FullScreen))
//	{
//		MessageBox(0, L"Window Initialization - Failed", L"Error", MB_OK);
//		return 0;
//	}
//
//	// 初始化 direct3d
//	if (!InitD3D())
//	{
//		MessageBox(0, L"Failed to initialize direct3d 12", L"ERROR", MB_OK);
//		Cleanup();
//		return 1;
//	}
//
//	// 开启主循环
//	MainLoop();
//
//	// 在释放所有东西之前，我们先等待GPU完成所有任务
//	WaitForPreviousFrame();
//
//	// 关闭屏障事件
//	CloseHandle(m_FenceEvent);
//	return 0;
//}

KAIROS_EXPORT_BEGIN

bool KAIROS_API InitializeWindow(HINSTANCE hInstance, int ShowWnd, int width, int height, bool fullScreen)
{
	if (fullScreen)
	{
		HMONITOR hmon = MonitorFromWindow(m_Hwnd, MONITOR_DEFAULTTONEAREST);
		MONITORINFO mi = { sizeof(mi) };
		GetMonitorInfo(hmon, &mi);

		width = mi.rcMonitor.right - mi.rcMonitor.left;
		height = mi.rcMonitor.bottom - mi.rcMonitor.top;
	}

	WNDCLASSEX wc;
	wc.cbSize = sizeof(WNDCLASSEX);
	wc.style = CS_HREDRAW | CS_VREDRAW;
	wc.lpfnWndProc = WndProc;
	wc.cbClsExtra = NULL;
	wc.cbWndExtra = NULL;
	wc.hInstance = hInstance;
	wc.hIcon = LoadIcon(NULL, IDI_APPLICATION);
	wc.hCursor = LoadCursor(NULL, IDC_ARROW);
	wc.hbrBackground = (HBRUSH)(COLOR_WINDOW + 2);
	wc.lpszMenuName = NULL;
	wc.lpszClassName = m_WindowName;
	wc.hIconSm = LoadIcon(NULL, IDI_APPLICATION);

	if (!RegisterClassEx(&wc))
	{
		MessageBox(NULL, L"Error registering class", L"ERROR", MB_OK | MB_ICONERROR);
		return false;
	}

	m_Hwnd = CreateWindowEx(
		NULL, 
		m_WindowName, m_WindowTitle, 
		WS_OVERLAPPEDWINDOW,
		CW_USEDEFAULT, CW_USEDEFAULT,
		width, height, 
		NULL, 
		NULL, 
		hInstance, 
		NULL);

	if (!m_Hwnd)
	{
		MessageBox(NULL, L"Error creating window", L"Error", MB_OK | MB_ICONERROR);
		return false;
	}

	if (fullScreen)
	{
		SetWindowLong(m_Hwnd, GWL_STYLE, 0);
	}

	ShowWindow(m_Hwnd, ShowWnd);
	UpdateWindow(m_Hwnd);
	return true;
}

void KAIROS_API MainLoop()
{
	MSG msg;
	ZeroMemory(&msg, sizeof(MSG));

	while (m_Running)
	{
		if (PeekMessage(&msg, NULL, 0, 0, PM_REMOVE))
		{
			if (msg.message == WM_QUIT)
				break;

			TranslateMessage(&msg);
			DispatchMessage(&msg);
		}
		else
		{
			// 运行游戏代码
			Update(); // 游戏逻辑
			Render(); // 渲染逻辑
		}
	}
}

LRESULT KAIROS_API WndProc(HWND hWnd, UINT msg, WPARAM wParam, LPARAM lParam)
{
	switch (msg)
	{
	case WM_KEYDOWN:
		if (wParam == VK_ESCAPE)
			if (MessageBox(0, L"Are you sure you want to exit?", L"Really?", MB_YESNO | MB_ICONQUESTION) == IDYES)
			{
				m_Running = false;
				DestroyWindow(hWnd);
			}
		return 0;
	case WM_DESTROY:
		m_Running = false;
		PostQuitMessage(0);
		return 0;
	}
	return DefWindowProc(hWnd, msg, wParam, lParam);
}

bool KAIROS_API InitD3D()
{
	HRESULT hr;
	
	IDXGIFactory4* dxgiFactory;
	hr = CreateDXGIFactory1(IID_PPV_ARGS(&dxgiFactory));
	if (FAILED(hr))
		return false;
	
	// 适配器是图形卡（这包括主板上的嵌入式图形)
	IDXGIAdapter1* adapter;

	// 我们将从索引0开始寻找directx 12兼容的图形设备
	int adapterIndex = 0;

	// 如果我们找到了兼容的适配器，就设置为 true
	bool adapterFound = false;

	// 寻找支持d3d12的适配器
	while (dxgiFactory->EnumAdapters1(adapterIndex, &adapter) != DXGI_ERROR_NOT_FOUND)
	{
		DXGI_ADAPTER_DESC1 desc;
		adapter->GetDesc1(&desc);

		if (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE)
		{
			// 我们不想要软件适配器
			// 继续寻找下一个支持的适配器
			++adapterIndex;
			continue;
		}

		// 检查适配器是否支持direct3d 12 (feature level 11 或 更高)
		hr = D3D12CreateDevice(adapter, D3D_FEATURE_LEVEL_11_0, __uuidof(ID3D12Device), nullptr);
		if (SUCCEEDED(hr))
		{
			adapterFound = true;
			break;
		}
		++adapterIndex;
	}

	if (!adapterFound)
		return false;

	hr = D3D12CreateDevice(adapter, D3D_FEATURE_LEVEL_11_0, IID_PPV_ARGS(&m_Device));
	if (FAILED(hr))
		return false;

	// 全部都使用默认值
	D3D12_COMMAND_QUEUE_DESC cqDesc = {};
	cqDesc.Type = D3D12_COMMAND_LIST_TYPE_DIRECT; // DX12必须显式指定，否则默认值可能不兼容
	cqDesc.Flags = D3D12_COMMAND_QUEUE_FLAG_NONE;
	cqDesc.Priority = 0;
	// 创建命令队列
	hr = m_Device->CreateCommandQueue(&cqDesc, IID_PPV_ARGS(&m_CommandQueue));
	if (FAILED(hr))
		return false;

	// 创建交换链

	// 该结构描述我们的显示模式
	DXGI_MODE_DESC backBufferDesc = {};
	backBufferDesc.Width = m_Width;
	backBufferDesc.Height = m_Height;
	backBufferDesc.Format = DXGI_FORMAT_R8G8B8A8_UNORM;

	// 描述多重采样，我们不需要多重采样，因此设置为1
	DXGI_SAMPLE_DESC sampleDesc = {};
	sampleDesc.Count = 1;
	sampleDesc.Quality = 0;

	// 描述和创建交换链
	DXGI_SWAP_CHAIN_DESC swapChainDesc = {};
	swapChainDesc.BufferDesc = backBufferDesc;
	swapChainDesc.SampleDesc = sampleDesc;
	swapChainDesc.BufferCount = k_FrameBufferCount;
	swapChainDesc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
	swapChainDesc.SwapEffect = DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL;;
	swapChainDesc.OutputWindow = m_Hwnd;
	swapChainDesc.Windowed = !m_FullScreen;
	swapChainDesc.Flags = DXGI_SWAP_CHAIN_FLAG_ALLOW_MODE_SWITCH; // 允许全屏切换

	IDXGISwapChain* tempSwapChain;
	hr = dxgiFactory->CreateSwapChain(m_CommandQueue, &swapChainDesc, &tempSwapChain); // 交换链创建后 CommandQueue 会被刷新
	if (FAILED(hr))
	{
		MessageBox(0, L"Failed to CreateSwapChain", L"ERROR", MB_OK);
		return false;
	}
	m_SwapChain = static_cast<IDXGISwapChain3*>(tempSwapChain);

	m_FrameIndex = m_SwapChain->GetCurrentBackBufferIndex();

	// 创建后台缓冲(render target view) 描述符堆

	// 描述 rtv 描述符堆 并 创建
	D3D12_DESCRIPTOR_HEAP_DESC rtvHeapDesc = {};
	rtvHeapDesc.NumDescriptors = k_FrameBufferCount;
	rtvHeapDesc.Type = D3D12_DESCRIPTOR_HEAP_TYPE_RTV;
	rtvHeapDesc.Flags = D3D12_DESCRIPTOR_HEAP_FLAG_NONE;
	hr = m_Device->CreateDescriptorHeap(&rtvHeapDesc, IID_PPV_ARGS(&m_RTVDescroptorHeap));
	if (FAILED(hr))
		return false;

	// 获取描述符堆类型大小
	m_RTVDescriptorSize = m_Device->GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV);

	// 获取描述符堆中第一个描述符的句柄
	CD3DX12_CPU_DESCRIPTOR_HANDLE rtvHandle(m_RTVDescroptorHeap->GetCPUDescriptorHandleForHeapStart());
	
	// 为每个后台缓冲创建RTV
	for (int i = 0; i < k_FrameBufferCount; ++i)
	{
		// 先获取后台缓冲的渲染目标资源
		hr = m_SwapChain->GetBuffer(i, IID_PPV_ARGS(&m_RenderTargets[i]));
		if (FAILED(hr))
			return false;

		// 创建 RTV
		m_Device->CreateRenderTargetView(m_RenderTargets[i], nullptr, rtvHandle);
		rtvHandle.Offset(1, m_RTVDescriptorSize);
	}

	// 创建命令分配器
	for (int i = 0; i < k_FrameBufferCount; ++i)
	{
		hr = m_Device->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&m_CommandAllocators[i]));
		if (FAILED(hr))
			return false;
	}
	
	// 创建命令列表
	hr = m_Device->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, m_CommandAllocators[0], NULL, IID_PPV_ARGS(&m_CommandList));
	if (FAILED(hr))
		return false;
	m_CommandList->Close();

	// 创建屏障和屏障事件

	// 创建所有屏障
	for (int i = 0; i < k_FrameBufferCount; ++i)
	{
		hr = m_Device->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&m_Fences[i]));
		if (FAILED(hr))
			return false;

		// 屏障值默认设置为0
		m_FenceValues[i] = 0;
	}
	// 创建屏障事件
	m_FenceEvent = CreateEvent(nullptr, FALSE, FALSE, nullptr);
	if (m_FenceEvent == nullptr)
		return false;

	return true;
}

void KAIROS_API Update()
{
	// do nothing now
}

void KAIROS_API UpdatePipeline()
{
	// 重置命令分配器前，先等待GPU完成它
	WaitForPreviousFrame();

	HRESULT hr;
	// 重置命令分配器。这会释放它占有的内存
	hr = m_CommandAllocators[m_FrameIndex]->Reset();
	if (FAILED(hr))
		m_Running = false;

	hr = m_CommandList->Reset(m_CommandAllocators[m_FrameIndex], NULL);
	if (FAILED(hr))
		m_Running = false;

	// 把渲染目标资源从 PRESENT 状态转换为 RENDER_TARGET 状态
	CD3DX12_RESOURCE_BARRIER barrier = CD3DX12_RESOURCE_BARRIER::Transition(m_RenderTargets[m_FrameIndex], D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_RENDER_TARGET);
	m_CommandList->ResourceBarrier(1, &barrier);
	// 获取当前RTV描述符句柄
	CD3DX12_CPU_DESCRIPTOR_HANDLE rtvHandle(m_RTVDescroptorHeap->GetCPUDescriptorHandleForHeapStart(), m_FrameIndex, m_RTVDescriptorSize);
	// 设置渲染目标到 OM stage
	m_CommandList->OMSetRenderTargets(1, &rtvHandle, FALSE, nullptr);

	// 清空渲染目标
	const float clearColor[] = { 0.0f, 0.2f, 0.4f, 1.0f };
	m_CommandList->ClearRenderTargetView(rtvHandle, clearColor, 0, nullptr);

	// 把渲染目标切换回 PRESENT 状态，以便交换链呈现它
	barrier = CD3DX12_RESOURCE_BARRIER::Transition(m_RenderTargets[m_FrameIndex], D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PRESENT);
	m_CommandList->ResourceBarrier(1, &barrier);

	hr = m_CommandList->Close();
	if (FAILED(hr))
		m_Running = false;
}

void KAIROS_API Render()
{
	HRESULT hr;

	UpdatePipeline();

	// 创建一个命令列表数组
	ID3D12CommandList* ppCommandList[] = { m_CommandList };

	// 执行命令列表
	m_CommandQueue->ExecuteCommandLists(_countof(ppCommandList), ppCommandList);

	// 该命令会被插入到命令列表末尾
	// 当GPU执行完前面的命令后，会设置屏障的值为fenceValue，这样我们就知道GPU已完成了当前帧任务
	hr = m_CommandQueue->Signal(m_Fences[m_FrameIndex], m_FenceValues[m_FrameIndex]);
	if (FAILED(hr))
		m_Running = false;

	// 呈现后台缓冲
	hr = m_SwapChain->Present(0, 0);
	if (FAILED(hr))
		m_Running = false;
}

void KAIROS_API Cleanup()
{
	// 等待所有帧任务完成
	for (int i = 0; i < k_FrameBufferCount; ++i)
	{
		m_FrameIndex = i;
		WaitForPreviousFrame();
	}

	// 退出前将交换链退出全屏状态
	BOOL fs = false;
	HRESULT hr = m_SwapChain->GetFullscreenState(&fs, NULL);
	if (FAILED(hr))
		m_Running = false;
	m_SwapChain->SetFullscreenState(false, NULL);

	SAFE_RELEASE(m_Device);
	SAFE_RELEASE(m_SwapChain);
	SAFE_RELEASE(m_CommandQueue);
	SAFE_RELEASE(m_RTVDescroptorHeap);
	SAFE_RELEASE(m_CommandList);

	for (int i = 0; i < k_FrameBufferCount; ++i)
	{
		SAFE_RELEASE(m_RenderTargets[i]);
		SAFE_RELEASE(m_CommandAllocators[i]);
		SAFE_RELEASE(m_Fences[i]);
	}
}

void KAIROS_API WaitForPreviousFrame()
{
	HRESULT hr;

	// 获取当前后台缓冲index
	m_FrameIndex = m_SwapChain->GetCurrentBackBufferIndex();

	// 如果屏障值小于预期值，发起屏障事件
	if (m_Fences[m_FrameIndex]->GetCompletedValue() < m_FenceValues[m_FrameIndex])
	{
		hr = m_Fences[m_FrameIndex]->SetEventOnCompletion(m_FenceValues[m_FrameIndex], m_FenceEvent);
		if (FAILED(hr))
		{
			m_Running = false;
			return;
		}

		// 等待屏障事件被触发
		WaitForSingleObject(m_FenceEvent, INFINITE);
	}

	// 自增屏障值
	++m_FenceValues[m_FrameIndex];
}

KAIROS_EXPORT_END