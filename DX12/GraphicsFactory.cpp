#include "GraphicsFactory.h"


KAIROS_EXPORT_BEGIN

int KAIROS_API GraphicsFactory_Create(IDXGIFactory5** p_this)
{
	return CreateDXGIFactory1(IID_PPV_ARGS(p_this));
}

void KAIROS_API GraphicsFactory_Dispose(IDXGIFactory5* _this)
{
	_this->Release();
}

int KAIROS_API GraphicsFactory_CreateDevice(IDXGIFactory5* _this, ID3D12Device** ppDevice)
{
	HRESULT hr;

	IDXGIAdapter1* adapter;
	int adapterIndex = 0;
	bool adapterFound = false;
	while (_this->EnumAdapters1(adapterIndex, &adapter) != DXGI_ERROR_NOT_FOUND)
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
		return hr;
	}

	ID3D12Device* pDevice;
	hr = D3D12CreateDevice(adapter, D3D_FEATURE_LEVEL_11_0, IID_PPV_ARGS(&pDevice));
	*ppDevice = pDevice;
	return hr;
}

int KAIROS_API GraphicsFactory_CreateSwapChain(IDXGIFactory5* _this, IDXGISwapChain3** ppSwapChain, ID3D12CommandQueue* pCommandQueue, int width, int height, DXGI_FORMAT format, int msaa, int aaQuality, int bufferCount, HWND hwnd, BOOL windowed)
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
	hr = _this->CreateSwapChain(pCommandQueue, &swapChainDesc, &tempSwapChain); // 交换链创建后 CommandQueue 会被刷新
	if (FAILED(hr))
		return hr;

	IDXGISwapChain3* pSwpaChain;
	pSwpaChain = static_cast<IDXGISwapChain3*>(tempSwapChain);
	*ppSwapChain = pSwpaChain;
	return hr;
}

int KAIROS_API GraphicsFactory_CheckFeatureSupport(IDXGIFactory5* _this, DXGI_FEATURE feature, void* pFeatureSupportData, UINT featureSupportDataSize)
{
	return _this->CheckFeatureSupport(feature, pFeatureSupportData, featureSupportDataSize);
}

KAIROS_EXPORT_END