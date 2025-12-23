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
		return CreateResult{ CreateCommandQueueFailed, new GraphicsCommandQueue{ nullptr } };
	return CreateResult{ (int)GraphicsSuccess, new GraphicsCommandQueue{ pQueue } };
}

CreateResult GraphicsDevice::CreateDescriptorHeap(int count, D3D12_DESCRIPTOR_HEAP_TYPE type, D3D12_DESCRIPTOR_HEAP_FLAGS flags)
{
	D3D12_DESCRIPTOR_HEAP_DESC desc = { type, count, flags };
	ID3D12DescriptorHeap* pDescriptorHeap;
	HRESULT hr = m_pDevice->CreateDescriptorHeap(&desc, IID_PPV_ARGS(&pDescriptorHeap));
	if (FAILED(hr))
		return CreateResult{ CreateDescriptorHeapFailed, nullptr };

	return CreateResult{ GraphicsSuccess, new GraphicsDescriptorHeap{ pDescriptorHeap } };
}

UINT GraphicsDevice::GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE descriptorHeapType)
{
	return m_pDevice->GetDescriptorHandleIncrementSize(descriptorHeapType);
}

void GraphicsDevice::CreateRenderTargetView(GraphicsRenderTarget* pRenderTarget, CD3DX12_CPU_DESCRIPTOR_HANDLE handle)
{
	m_pDevice->CreateRenderTargetView(pRenderTarget->GetInternalPtr(), nullptr, handle);
}

void GraphicsDevice::CreateRenderTargetView(GraphicsRenderTarget* pRenderTarget, GraphicsDescriptorHeap* pDescriptorHeap, int index)
{
	ID3D12DescriptorHeap* ipDescriptorHeap = pDescriptorHeap->GetInternalPtr();
	UINT descriptorSize = m_pDevice->GetDescriptorHandleIncrementSize(ipDescriptorHeap->GetDesc().Type);
	CD3DX12_CPU_DESCRIPTOR_HANDLE handle{ ipDescriptorHeap->GetCPUDescriptorHandleForHeapStart() };
	handle.Offset(index, descriptorSize);
	m_pDevice->CreateRenderTargetView(pRenderTarget->GetInternalPtr(), nullptr, handle);
}

CreateResult GraphicsDevice::CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE type)
{
	ID3D12CommandAllocator* pCommandAllocator;
	HRESULT hr = m_pDevice->CreateCommandAllocator(type, IID_PPV_ARGS(&pCommandAllocator));
	if (FAILED(hr))
		return CreateResult{ CreateCommandAllocatorFailed, nullptr };
	return CreateResult{ GraphicsSuccess, new GraphicsCommandAllocator{pCommandAllocator} };
}

CreateResult GraphicsDevice::CreateCommandList(GraphicsCommandAllocator* pCommandAllocator, D3D12_COMMAND_LIST_TYPE type, UINT nodeMask)
{
	ID3D12CommandList* pCommandList;
	HRESULT hr = m_pDevice->CreateCommandList(nodeMask, type, pCommandAllocator->GetInternalPtr(), NULL, IID_PPV_ARGS(&pCommandList));
	if (FAILED(hr))
		return CreateResult{ CreateCommandListFailed , nullptr };
	return CreateResult{ GraphicsSuccess, new GraphicsCommandList{pCommandList} };
}

CreateResult GraphicsDevice::CreateFence(UINT64 initialValue, D3D12_FENCE_FLAGS flags)
{
	ID3D12Fence* pFence;
	HRESULT hr = m_pDevice->CreateFence(initialValue, flags, IID_PPV_ARGS(&pFence));
	if (FAILED(hr))
		return CreateResult{ CreateFenceFailed, nullptr };
	return CreateResult{ GraphicsSuccess, new GraphicsFence{pFence} };
}

CreateResult GraphicsDevice::CreateEmptyRootSignature(D3D12_ROOT_SIGNATURE_FLAGS flags)
{
	CD3DX12_ROOT_SIGNATURE_DESC rootSignatureDesc;
	rootSignatureDesc.Init(0, nullptr, 0, nullptr, flags);

	HRESULT hr;
	ID3DBlob* signature;
	hr = D3D12SerializeRootSignature(&rootSignatureDesc, D3D_ROOT_SIGNATURE_VERSION_1, &signature, nullptr);
	if (FAILED(hr))
		return CreateResult{ SerializeRootSignatureFailed , nullptr };

	ID3D12RootSignature* pRootSignature;
	hr = m_pDevice->CreateRootSignature(0, signature->GetBufferPointer(), signature->GetBufferSize(), IID_PPV_ARGS(&pRootSignature));
	SAFE_RELEASE(signature);
	if (FAILED(hr))
		return CreateResult{ CreateRootSignatureFailed , nullptr };

	return CreateResult{ GraphicsSuccess, new GraphicsRootSignature{pRootSignature} };
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

CreateResult KAIROS_API GraphicsDevice_CreateDescriptorHeap(GraphicsDevice* _this, int count, D3D12_DESCRIPTOR_HEAP_TYPE type, D3D12_DESCRIPTOR_HEAP_FLAGS flags)
{
	return _this->CreateDescriptorHeap(count, type, flags);
}

UINT KAIROS_API GraphicsDevice_GetDescriptorHandleIncrementSize(GraphicsDevice* _this, D3D12_DESCRIPTOR_HEAP_TYPE descriptorHeapType)
{
	return _this->GetDescriptorHandleIncrementSize(descriptorHeapType);
}

void KAIROS_API GraphicsDevice_CreateRenderTargetViewByHandle(GraphicsDevice* _this, GraphicsRenderTarget* pRenderTarget, CD3DX12_CPU_DESCRIPTOR_HANDLE handle)
{
	_this->CreateRenderTargetView(pRenderTarget, handle);
}

void KAIROS_API GraphicsDevice_CreateRenderTargetViewByHeapIndex(GraphicsDevice* _this, GraphicsRenderTarget* pRenderTarget, GraphicsDescriptorHeap* pDescriptorHeap, int index)
{
	_this->CreateRenderTargetView(pRenderTarget, pDescriptorHeap, index);
}

KAIROS_EXPORT_END

CreateResult KAIROS_API GraphicsDevice_CreateCommandAllocator(GraphicsDevice* _this, D3D12_COMMAND_LIST_TYPE type)
{
	return _this->CreateCommandAllocator(type);
}

CreateResult KAIROS_API GraphicsDevice_CreateCommandList(GraphicsDevice* _this, GraphicsCommandAllocator* pCommandAllocator, D3D12_COMMAND_LIST_TYPE type, UINT nodeMask)
{
	return _this->CreateCommandList(pCommandAllocator, type, nodeMask);
}

CreateResult KAIROS_API GraphicsDevice_CreateFence(GraphicsDevice* _this, UINT64 initialValue, D3D12_FENCE_FLAGS flags)
{
	return _this->CreateFence(initialValue, flags);
}

CreateResult KAIROS_API GraphicsDevice_CreateEmptyRootSignature(GraphicsDevice* _this, D3D12_ROOT_SIGNATURE_FLAGS flags)
{
	return _this->CreateEmptyRootSignature(flags);
}
