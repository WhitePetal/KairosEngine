#include "GraphicsCommandQueue.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandQueue_Dispose(GraphicsCommandQueue* _this)
{
	SAFE_RELEASE(_this->m_pCommandQueue);
}

void KAIROS_API GraphicsCommandQueue_ExecuteCommandLists(GraphicsCommandQueue* _this, GraphicsCommandList* pGraphicsCommandLst, int executeCount)
{
	ID3D12CommandList* ppCommandLists[SUPPORT_MAX_EXECUTE_COMMAND_LIST_COUNT];
	for (int i = 0; i < executeCount; ++i)
	{
		ppCommandLists[i] = pGraphicsCommandLst->m_pCommandList;
	}

	_this->m_pCommandQueue->ExecuteCommandLists(executeCount, ppCommandLists);
}

int KAIROS_API GraphicsCommandQueue_Signal(GraphicsCommandQueue* _this, GraphicsFence* pGraphicsFence, UINT64 fenceValue)
{
	HRESULT hr = _this->m_pCommandQueue->Signal(pGraphicsFence->m_pFence, fenceValue);
	if (FAILED(hr))
		return CommandQueueSignatureFailed;
	return GraphicsSuccess;
}

KAIROS_EXPORT_END