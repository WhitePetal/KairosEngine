#include "GraphicsResource.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsResource_Dispose(GraphicsResource* _this)
{
	SAFE_RELEASE(_this->m_pResource);
}

KAIROS_EXPORT_END