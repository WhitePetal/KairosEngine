#include "GraphicsCommandQueue.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandQueue_Dispose(GraphicsCommandQueue* _this)
{
	SAFE_RELEASE(_this->m_pCommandQueue);
}

KAIROS_EXPORT_END