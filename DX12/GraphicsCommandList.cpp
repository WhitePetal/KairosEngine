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

KAIROS_EXPORT_END