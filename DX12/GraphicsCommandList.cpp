#include "GraphicsCommandList.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandList_Dispose(GraphicsCommandList* _this)
{
	SAFE_RELEASE(_this->m_pCommandList);
}

int KAIROS_API GraphicsCommandList_UpdateSubResources(GraphicsCommandList* _this, GraphicsResource* pDsfResource, GraphicsResource* pFromResource, UINT64 intermediateOffset, UINT firstSubResource, UINT numSubResource, void* pData, INT64 dataSize)
{
	D3D12_SUBRESOURCE_DATA vertexData = {};
	vertexData.pData = reinterpret_cast<BYTE*>(pData);
	vertexData.SlicePitch = dataSize;

	UINT64 requiredSize = UpdateSubresources(_this->m_pCommandList, pDsfResource->m_pResource, pFromResource->m_pResource, intermediateOffset, firstSubResource, numSubResource, &vertexData);
	if (requiredSize <= 0)
		return UpdateSubBufferResourceFailed;

	return GraphicsSuccess;
}

void KAIROS_API GraphicsCommandList_ResourceBarrier(GraphicsCommandList* _this, GraphicsResource* pResource, D3D12_RESOURCE_STATES beforeStates, D3D12_RESOURCE_STATES afterStates)
{
	D3D12_RESOURCE_BARRIER barrier = CD3DX12_RESOURCE_BARRIER::Transition(pResource->m_pResource, beforeStates, afterStates);
	_this->m_pCommandList->ResourceBarrier(1, &barrier);
}

int KAIROS_API GraphicsCommandList_Close(GraphicsCommandList* _this)
{
	HRESULT hr = _this->m_pCommandList->Close();
	if (FAILED(hr))
		return CloseCommandListFailed;

	return GraphicsSuccess;
}

int KAIROS_API GraphicsCommandList_Reset(GraphicsCommandList* _this, GraphicsCommandAllocator* pGraphicsCommandAllocator, GraphicsPipelineState* pGraphicsPipelineState)
{
	HRESULT hr = _this->m_pCommandList->Reset(pGraphicsCommandAllocator->m_pCommandAllocator, pGraphicsPipelineState->m_pPipelineState);
	if (FAILED(hr))
		return CommandListResetFailed;

	return GraphicsSuccess;
}

void KAIROS_API GraphicsCommandList_OMSetRenderTargets(GraphicsCommandList* _this, GraphicsDescriptorHeap* pGraphicsDescriptorHeap, UINT descriptorOffset, UINT descriptorSize, UINT descriptorCount)
{
	CD3DX12_CPU_DESCRIPTOR_HANDLE rtvHandle(pGraphicsDescriptorHeap->m_pDescriptorHeap->GetCPUDescriptorHandleForHeapStart(), descriptorOffset, descriptorSize);
	_this->m_pCommandList->OMSetRenderTargets(descriptorCount, &rtvHandle, FALSE, nullptr);
}

void KAIROS_API GraphicsCommandList_ClearRenderTargetView(GraphicsCommandList* _this, GraphicsDescriptorHeap* pGraphicsDescriptorHeap, UINT descriptorOffset, UINT descriptorSize, FLOAT* pColor, UINT rectCount, D3D12_RECT* pRects)
{
	CD3DX12_CPU_DESCRIPTOR_HANDLE rtvHandle(pGraphicsDescriptorHeap->m_pDescriptorHeap->GetCPUDescriptorHandleForHeapStart(), descriptorOffset, descriptorSize);
	_this->m_pCommandList->ClearRenderTargetView(rtvHandle, pColor, rectCount, pRects);
}

KAIROS_EXPORT_END