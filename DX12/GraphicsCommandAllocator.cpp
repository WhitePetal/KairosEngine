#include "GraphicsCommandAllocator.h"

GraphicsCommandAllocator::GraphicsCommandAllocator(ID3D12CommandAllocator* pCommandAllocator)
{
	m_pCommandAllocator = pCommandAllocator;
}

void GraphicsCommandAllocator::Dispose()
{
	SAFE_RELEASE(m_pCommandAllocator);
}

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandAllocator_Dispose(GraphicsCommandAllocator* _this)
{
	_this->Dispose();
	delete _this;
}

KAIROS_EXPORT_END