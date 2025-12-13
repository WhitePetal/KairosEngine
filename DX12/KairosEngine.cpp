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

KAIROS_API bool InitializeWindow(HINSTANCE hInstance, int ShowWnd, int width, int height, bool fullScreen)
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

KAIROS_API void MainLoop()
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
		}
	}
}

KAIROS_API LRESULT WndProc(HWND hWnd, UINT msg, WPARAM wParam, LPARAM lParam)
{
	switch (msg)
	{
	case WM_KEYDOWN:
		if (wParam == VK_ESCAPE)
			if (MessageBox(0, L"Are you sure you want to exit?", L"Really?", MB_YESNO | MB_ICONQUESTION) == IDYES)
				DestroyWindow(hWnd);
		return 0;
	case WM_DESTROY:
		PostQuitMessage(0);
		return 0;
	}
	return DefWindowProc(hWnd, msg, wParam, lParam);
}

KAIROS_API bool InitD3D()
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

	// 描述和创建交换链
	DXGI_SWAP_CHAIN_DESC swapChainDesc = {};
	swapChainDesc.BufferDesc = backBufferDesc;
	swapChainDesc.SampleDesc = sampleDesc;
	swapChainDesc.BufferCount = 3;
	swapChainDesc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
	swapChainDesc.SwapEffect = DXGI_SWAP_EFFECT_DISCARD;
	swapChainDesc.OutputWindow = m_Hwnd;
	swapChainDesc.Windowed = true;

	IDXGISwapChain* tempSwapChain;
	dxgiFactory->CreateSwapChain(m_CommandQueue, &swapChainDesc, &tempSwapChain); // 交换链创建后 CommandQueue 会被刷新
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

KAIROS_API void Update()
{
	// do nothing now
}

KAIROS_API void UpdatePipeline()
{
	HRESULT hr;
	
}

KAIROS_EXPORT_END