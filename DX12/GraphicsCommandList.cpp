#include "GraphicsCommandList.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandList_Dispose(ID3D12GraphicsCommandList* _this)
{
	_this->Release();
}

UINT64 KAIROS_API GraphicsCommandList_UpdateSubResources(ID3D12GraphicsCommandList* _this, ID3D12Resource* pDstResource, ID3D12Resource* pFromResource, UINT64 intermediateOffset, UINT firstSubResource, UINT numSubResource, void* pData, INT64 dataSize)
{
	D3D12_SUBRESOURCE_DATA vertexData = {};
	vertexData.pData = reinterpret_cast<BYTE*>(pData);
	vertexData.SlicePitch = dataSize;

	return UpdateSubresources(_this, pDstResource, pFromResource, intermediateOffset, firstSubResource, numSubResource, &vertexData);
}

void KAIROS_API GraphicsCommandList_ResourceBarrier(ID3D12GraphicsCommandList* _this, ID3D12Resource* pResource, D3D12_RESOURCE_STATES beforeStates, D3D12_RESOURCE_STATES afterStates)
{
	D3D12_RESOURCE_BARRIER barrier = CD3DX12_RESOURCE_BARRIER::Transition(pResource, beforeStates, afterStates);
	_this->ResourceBarrier(1, &barrier);
}

int KAIROS_API GraphicsCommandList_Close(ID3D12GraphicsCommandList* _this)
{
	return _this->Close();
}

int KAIROS_API GraphicsCommandList_Reset(ID3D12GraphicsCommandList* _this, ID3D12CommandAllocator* pCommandAllocator, ID3D12PipelineState* pPipelineState)
{
	return _this->Reset(pCommandAllocator, pPipelineState);
}

void KAIROS_API GraphicsCommandList_OMSetRenderTargets(ID3D12GraphicsCommandList* _this, ID3D12DescriptorHeap* pDescriptorHeap, UINT descriptorOffset, UINT descriptorSize, UINT descriptorCount)
{
	CD3DX12_CPU_DESCRIPTOR_HANDLE rtvHandle(pDescriptorHeap->GetCPUDescriptorHandleForHeapStart(), descriptorOffset, descriptorSize);
	_this->OMSetRenderTargets(descriptorCount, &rtvHandle, FALSE, nullptr);
}

void KAIROS_API GraphicsCommandList_ClearRenderTargetView(ID3D12GraphicsCommandList* _this, ID3D12DescriptorHeap* pDescriptorHeap, UINT descriptorOffset, UINT descriptorSize, FLOAT* pColor, UINT rectCount, D3D12_RECT* pRects)
{
	CD3DX12_CPU_DESCRIPTOR_HANDLE rtvHandle(pDescriptorHeap->GetCPUDescriptorHandleForHeapStart(), descriptorOffset, descriptorSize);
	_this->ClearRenderTargetView(rtvHandle, pColor, rectCount, pRects);
}

KAIROS_EXPORT_END