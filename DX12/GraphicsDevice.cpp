#include "GraphicsDevice.h"

#define DeviceCreateSuccess 0
#define CreateDXGIFactoryFailed 1
#define NoUsefulAdapter 2
#define CreateDeviceFailed 3

int GraphicsDevice::Initialize()
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

void GraphicsDevice::DeInitialize()
{
	SAFE_RELEASE(m_pDevice);
}

KAIROS_EXPORT_BEGIN

GraphicsDevice* KAIROS_API GraphicsDevice_Create()
{
	return new GraphicsDevice();
}

int KAIROS_API GraphicsDevice_Initialize(GraphicsDevice* _this)
{
	return _this->Initialize();
}

void KAIROS_API Graphics_DeInitialize(GraphicsDevice* _this)
{
	_this->DeInitialize();
}

KAIROS_EXPORT_END