#include "GraphicsCommandQueue.h"

GraphicsCommandQueue::GraphicsCommandQueue(ID3D12CommandQueue* pQueue)
{
	m_pCommandQueue = pQueue;
}

void GraphicsCommandQueue::Dispose()
{
	SAFE_RELEASE(m_pCommandQueue);
}

ID3D12CommandQueue* GraphicsCommandQueue::GetInternalPtr()
{
	return m_pCommandQueue;
}

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandQueue_Dispose(GraphicsCommandQueue* _this)
{
	_this->Dispose();
	delete _this;
}

KAIROS_EXPORT_END