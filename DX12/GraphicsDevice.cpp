#include "GraphicsDevice.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsDevice_Dispose(GraphicsDevice* _this)
{
	SAFE_RELEASE(_this->m_pDevice);
}

int KAIROS_API GraphicsDevice_CreateCommandQueue(GraphicsDevice* _this, GraphicsCommandQueue* pGraphicsCommandQueue, D3D12_COMMAND_LIST_TYPE type, int priority, D3D12_COMMAND_QUEUE_FLAGS flags, int nodeMask)
{
	HRESULT hr;
	D3D12_COMMAND_QUEUE_DESC desc = { type, priority, flags, nodeMask };
	ID3D12CommandQueue* pQueue;
	hr = _this->m_pDevice->CreateCommandQueue(&desc, IID_PPV_ARGS(&pQueue));
	if (FAILED(hr))
		return CreateCommandQueueFailed;

	pGraphicsCommandQueue->m_pCommandQueue = pQueue;
	return GraphicsSuccess;
}

int KAIROS_API GraphicsDevice_CreateDescriptorHeap(GraphicsDevice* _this, GraphicsDescriptorHeap* pGraphicsDescriptorHeap, int count, D3D12_DESCRIPTOR_HEAP_TYPE type, D3D12_DESCRIPTOR_HEAP_FLAGS flags)
{
	D3D12_DESCRIPTOR_HEAP_DESC desc = { type, count, flags };
	ID3D12DescriptorHeap* pDescriptorHeap;
	HRESULT hr = _this->m_pDevice->CreateDescriptorHeap(&desc, IID_PPV_ARGS(&pDescriptorHeap));
	if (FAILED(hr))
		return CreateDescriptorHeapFailed;

	pGraphicsDescriptorHeap->m_pDescriptorHeap = pDescriptorHeap;
	return GraphicsSuccess;
}

UINT KAIROS_API GraphicsDevice_GetDescriptorHandleIncrementSize(GraphicsDevice* _this, D3D12_DESCRIPTOR_HEAP_TYPE descriptorHeapType)
{
	return _this->m_pDevice->GetDescriptorHandleIncrementSize(descriptorHeapType);
}

void KAIROS_API GraphicsDevice_CreateRenderTargetViewByHandle(GraphicsDevice* _this, GraphicsRenderTarget* pGraphicsRenderTarget, CD3DX12_CPU_DESCRIPTOR_HANDLE handle)
{
	_this->m_pDevice->CreateRenderTargetView(pGraphicsRenderTarget->m_pRenderTarget, nullptr, handle);
}

void KAIROS_API GraphicsDevice_CreateRenderTargetViewByHeapIndex(GraphicsDevice* _this, GraphicsRenderTarget* pGraphicsRenderTarget, GraphicsDescriptorHeap* pDescriptorHeap, int index)
{
	ID3D12DescriptorHeap* ipDescriptorHeap = pDescriptorHeap->m_pDescriptorHeap;
	UINT descriptorSize = _this->m_pDevice->GetDescriptorHandleIncrementSize(ipDescriptorHeap->GetDesc().Type);
	CD3DX12_CPU_DESCRIPTOR_HANDLE handle{ ipDescriptorHeap->GetCPUDescriptorHandleForHeapStart() };
	handle.Offset(index, descriptorSize);
	_this->m_pDevice->CreateRenderTargetView(pGraphicsRenderTarget->m_pRenderTarget, nullptr, handle);
}

int KAIROS_API GraphicsDevice_CreateCommandAllocator(GraphicsDevice* _this, GraphicsCommandAllocator* pGraphicsCommandAllocator, D3D12_COMMAND_LIST_TYPE type)
{
	ID3D12CommandAllocator* pCommandAllocator;
	HRESULT hr = _this->m_pDevice->CreateCommandAllocator(type, IID_PPV_ARGS(&pCommandAllocator));
	if (FAILED(hr))
		return CreateCommandAllocatorFailed;

	pGraphicsCommandAllocator->m_pCommandAllocator = pCommandAllocator;
	return GraphicsSuccess;
}

int KAIROS_API GraphicsDevice_CreateCommandList(GraphicsDevice* _this, GraphicsCommandAllocator* pGraphicsCommandAllocator, GraphicsCommandList* pGraphicsCommandList, D3D12_COMMAND_LIST_TYPE type, UINT nodeMask)
{
	ID3D12CommandList* pCommandList;
	HRESULT hr = _this->m_pDevice->CreateCommandList(nodeMask, type, pGraphicsCommandAllocator->m_pCommandAllocator, NULL, IID_PPV_ARGS(&pCommandList));
	if (FAILED(hr))
		return CreateCommandListFailed;

	pGraphicsCommandList->m_pCommandList = pCommandList;
	return GraphicsSuccess;
}

int KAIROS_API GraphicsDevice_CreateFence(GraphicsDevice* _this, GraphicsFence* pGraphicsFence, UINT64 initialValue, D3D12_FENCE_FLAGS flags)
{
	ID3D12Fence* pFence;
	HRESULT hr = _this->m_pDevice->CreateFence(initialValue, flags, IID_PPV_ARGS(&pFence));
	if (FAILED(hr))
		return CreateFenceFailed;

	pGraphicsFence->m_pFence = pFence;
	return GraphicsSuccess;
}

int KAIROS_API GraphicsDevice_CreateEmptyRootSignature(GraphicsDevice* _this, GraphicsRootSignature* pGraphicsRootSignature, D3D12_ROOT_SIGNATURE_FLAGS flags)
{
	CD3DX12_ROOT_SIGNATURE_DESC rootSignatureDesc;
	rootSignatureDesc.Init(0, nullptr, 0, nullptr, flags);

	HRESULT hr;
	ID3DBlob* signature;
	hr = D3D12SerializeRootSignature(&rootSignatureDesc, D3D_ROOT_SIGNATURE_VERSION_1, &signature, nullptr);
	if (FAILED(hr))
		return SerializeRootSignatureFailed;

	ID3D12RootSignature* pRootSignature;
	hr = _this->m_pDevice->CreateRootSignature(0, signature->GetBufferPointer(), signature->GetBufferSize(), IID_PPV_ARGS(&pRootSignature));
	SAFE_RELEASE(signature);
	if (FAILED(hr))
		return CreateRootSignatureFailed;

	pGraphicsRootSignature->m_pRootSignature = pRootSignature;
	return GraphicsSuccess;
}

KAIROS_EXPORT_END
