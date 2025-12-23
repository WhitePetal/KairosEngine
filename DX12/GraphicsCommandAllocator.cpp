#include "GraphicsCommandAllocator.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandAllocator_Dispose(GraphicsCommandAllocator* _this)
{
	SAFE_RELEASE(_this->m_pCommandAllocator);
}

KAIROS_EXPORT_END