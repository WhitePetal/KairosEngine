#include "GraphicsFactory.h"

int GraphicsFactory::Create()
{
	HRESULT hr;

	hr = CreateDXGIFactory1(IID_PPV_ARGS(&m_pFactory));
	if (FAILED(hr))
		return CreateDXGIFactoryFailed;

	return GraphicsSuccess;
}

void GraphicsFactory::Dispose()
{
	SAFE_RELEASE(m_pFactory);
}

CreateResult GraphicsFactory::CreateDevice()
{
	HRESULT hr;

	IDXGIAdapter1* adapter;
	int adapterIndex = 0;
	bool adapterFound = false;
	while (m_pFactory->EnumAdapters1(adapterIndex, &adapter) != DXGI_ERROR_NOT_FOUND)
	{
		DXGI_ADAPTER_DESC1 desc;
		adapter->GetDesc1(&desc);

		if (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE)
		{
			++adapterIndex;
			continue;
		}

		hr = D3D12CreateDevice(adapter, D3D_FEATURE_LEVEL_11_0, __uuidof(ID3D12Device), nullptr);
		if (SUCCEEDED(hr))
		{
			adapterFound = true;
			break;
		}
		++adapterIndex;
	}

	if (!adapterFound)
	{
		return CreateResult{ NoUsefulAdapter, nullptr };
	}

	ID3D12Device* pDevice;
	hr = D3D12CreateDevice(adapter, D3D_FEATURE_LEVEL_11_0, IID_PPV_ARGS(&pDevice));
	if (FAILED(hr))
	{
		return CreateResult{ CreateDeviceFailed, nullptr };
	}

	return CreateResult{ GraphicsSuccess, new GraphicsDevice{ pDevice } };
}

CreateResult GraphicsFactory::CreateSwapChain(GraphicsCommandQueue* pCommandQueue, int width, int height, DXGI_FORMAT format, int msaa, int aaQuality, int bufferCount, HWND hwnd, BOOL windowed)
{
	HRESULT hr;
	// 该结构描述我们的显示模式
	DXGI_MODE_DESC backBufferDesc = {};
	backBufferDesc.Width = width;
	backBufferDesc.Height = height;
	backBufferDesc.Format = DXGI_FORMAT_R8G8B8A8_UNORM;

	// 描述多重采样，我们不需要多重采样，因此设置为1
	DXGI_SAMPLE_DESC sampleDesc = {};
	sampleDesc.Count = msaa;
	sampleDesc.Quality = aaQuality;

	// 描述和创建交换链
	DXGI_SWAP_CHAIN_DESC swapChainDesc = {};
	swapChainDesc.BufferDesc = backBufferDesc;
	swapChainDesc.SampleDesc = sampleDesc;
	swapChainDesc.BufferCount = bufferCount;
	swapChainDesc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
	swapChainDesc.SwapEffect = DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL;;
	swapChainDesc.OutputWindow = hwnd;
	swapChainDesc.Windowed = windowed;
	swapChainDesc.Flags = DXGI_SWAP_CHAIN_FLAG_ALLOW_MODE_SWITCH; // 允许全屏切换

	IDXGISwapChain* tempSwapChain;
	hr = m_pFactory->CreateSwapChain(pCommandQueue->GetInternalPtr(), &swapChainDesc, &tempSwapChain); // 交换链创建后 CommandQueue 会被刷新
	if (FAILED(hr))
	{
		return CreateResult{ CreateSwapChainFailed, nullptr };
	}
	IDXGISwapChain3* pSwpaChain;
	pSwpaChain = static_cast<IDXGISwapChain3*>(tempSwapChain);

	return CreateResult{ GraphicsSuccess, new GraphicsSwapChain(pSwpaChain) };
}

KAIROS_EXPORT_BEGIN

GraphicsFactory* KAIROS_API GraphicsFactory_Allocate()
{
	return new GraphicsFactory{};
}

int KAIROS_API GraphicsFactory_Create(GraphicsFactory* _this)
{
	return _this->Create();
}

void KAIROS_API GraphicsFactory_Dispose(GraphicsFactory* _this)
{
	_this->Dispose();
	delete _this;
}

CreateResult KAIROS_API GraphicsFactory_CreateDevice(GraphicsFactory* _this)
{
	return _this->CreateDevice();
}

CreateResult KAIROS_API GraphicsFactory_CreateSwapChain(GraphicsFactory* _this, GraphicsCommandQueue* pCommandQueue, int width, int height, DXGI_FORMAT format, int msaa, int aaQuality, int bufferCount, HWND hwnd, BOOL windowed)
{
	return _this->CreateSwapChain(pCommandQueue, width, height, format, msaa, aaQuality, bufferCount, hwnd, windowed);
}

KAIROS_EXPORT_END