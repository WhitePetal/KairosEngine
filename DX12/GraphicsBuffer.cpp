#include "GraphicsBuffer.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsBuffer_Dispose(GraphicsBuffer* _this)
{
	SAFE_RELEASE(_this->m_pBuffer);
}

KAIROS_EXPORT_END