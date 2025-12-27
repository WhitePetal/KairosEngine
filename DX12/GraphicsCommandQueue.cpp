#include "GraphicsCommandQueue.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandQueue_Dispose(ID3D12CommandQueue* _this)
{
	_this->Release();
}

void KAIROS_API GraphicsCommandQueue_ExecuteCommandLists(ID3D12CommandQueue* _this, ID3D12CommandList** ppCommandLst, int executeCount)
{
	_this->ExecuteCommandLists(executeCount, ppCommandLst);
}

int KAIROS_API GraphicsCommandQueue_Signal(ID3D12CommandQueue* _this, ID3D12Fence* pFence, UINT64 fenceValue)
{
	return _this->Signal(pFence, fenceValue);
}

KAIROS_EXPORT_END