#include "GraphicsDevice.h"

GraphicsDevice::GraphicsDevice(ID3D12Device* pDevice)
{
	m_pDevice = pDevice;
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
	return CreateResult{ (int)GraphicsSuccess, pGraphicsQueue };
}

KAIROS_EXPORT_BEGIN

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