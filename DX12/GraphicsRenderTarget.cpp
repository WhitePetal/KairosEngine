#include "GraphicsRenderTarget.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsRenderTarget_Dispose(GraphicsRenderTarget* _this)
{
	SAFE_RELEASE(_this->m_pRenderTarget);
}

KAIROS_EXPORT_END