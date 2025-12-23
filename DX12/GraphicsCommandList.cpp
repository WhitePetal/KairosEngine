#include "GraphicsCommandList.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandList_Dispose(GraphicsCommandList* _this)
{
	SAFE_RELEASE(_this->m_pCommandList);
}

KAIROS_EXPORT_END