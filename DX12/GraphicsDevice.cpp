#include "GraphicsDevice.h"

#define DeviceCreateSuccess 0
#define CreateDXGIFactoryFailed 1
#define NoUsefulAdapter 2
#define CreateDeviceFailed 3
#define CreateCommandQueueFailed 4

int GraphicsDevice::Create()
{
	HRESULT hr;

	IDXGIFactory4* dxgiFactory;
	hr = CreateDXGIFactory1(IID_PPV_ARGS(&dxgiFactory));
	if (FAILED(hr))
		return CreateDXGIFactoryFailed;

	IDXGIAdapter1* adapter;
	int adapterIndex = 0;
	bool adapterFound = false;
	while (dxgiFactory->EnumAdapters1(adapterIndex, &adapter) != DXGI_ERROR_NOT_FOUND)
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
		return NoUsefulAdapter;

	hr = D3D12CreateDevice(adapter, D3D_FEATURE_LEVEL_11_0, IID_PPV_ARGS(&m_pDevice));
	if (FAILED(hr))
		return CreateDeviceFailed;

	return DeviceCreateSuccess;
}

void GraphicsDevice::Dispose()
{
	SAFE_RELEASE(m_pDevice);
}

CreateResult GraphicsDevice::CreateaCommandQueue(D3D12_COMMAND_LIST_TYPE type, int priority, D3D12_COMMAND_QUEUE_FLAGS flags, UINT nodeMask)
{
	HRESULT hr;
	D3D12_COMMAND_QUEUE_DESC desc = { type, priority, flags, nodeMask };
	ID3D12CommandQueue* pQueue;
	hr = m_pDevice->CreateCommandQueue(&desc, IID_PPV_ARGS(&pQueue));
	if (FAILED(hr))
		return CreateResult{ CreateCommandQueueFailed, new GraphicsCommandQueue(nullptr) };
	GraphicsCommandQueue* pGraphicsQueue = new GraphicsCommandQueue(pQueue);
	return CreateResult{ 0, pGraphicsQueue };
}

KAIROS_EXPORT_BEGIN

GraphicsDevice* KAIROS_API GraphicsDevice_Allocate()
{
	return new GraphicsDevice();
}

int KAIROS_API GraphicsDevice_Create(GraphicsDevice* _this)
{
	return _this->Create();
}

void KAIROS_API GraphicsDevice_Dispose(GraphicsDevice* _this)
{
	_this->Dispose();
	delete _this;
}

CreateResult KAIROS_API GraphicsDevice_CreateCommandQueue(GraphicsDevice* _this, D3D12_COMMAND_LIST_TYPE type, int priority, D3D12_COMMAND_QUEUE_FLAGS flags, int nodeMask)
{
	return _this->CreateaCommandQueue(type, priority, flags, nodeMask);
}

KAIROS_EXPORT_END