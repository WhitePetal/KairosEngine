#include "GraphicsCommandAllocator.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandAllocator_Dispose(GraphicsCommandAllocator* _this)
{
	SAFE_RELEASE(_this->m_pCommandAllocator);
}

int KAIROS_API GraphicsCommandAllocator_Reset(GraphicsCommandAllocator* _this)
{
	HRESULT hr = _this->m_pCommandAllocator->Reset();
	if (FAILED(hr))
		return CommandAllocatorResetFailed;

	return GraphicsSuccess;
}

KAIROS_EXPORT_END