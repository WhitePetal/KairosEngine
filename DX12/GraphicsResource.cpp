#include "GraphicsResource.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsResource_Dispose(GraphicsResource* _this)
{
	SAFE_RELEASE(_this->m_pResource);
}

UINT64 KAIROS_API GraphicsResource_GetGPUVirtualAddress(GraphicsResource* _this)
{
	return _this->m_pResource->GetGPUVirtualAddress();
}

KAIROS_EXPORT_END