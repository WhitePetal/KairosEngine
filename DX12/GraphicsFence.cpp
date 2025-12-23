#include "GraphicsFence.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsFence_Dispose(GraphicsFence* _this)
{
	SAFE_RELEASE(_this->m_pFence);
}

KAIROS_EXPORT_END