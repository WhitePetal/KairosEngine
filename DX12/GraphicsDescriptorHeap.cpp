#include "GraphicsDescriptorHeap.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsDescriptorHeap_Dispose(GraphicsDescriptorHeap* _this)
{
	SAFE_RELEASE(_this->m_pDescriptorHeap);
}

SIZE_T KAIROS_API GraphicsDescriptorHeap_GetCPUDescriptorHandleForHeapStart(GraphicsDescriptorHeap* _this)
{
	return _this->m_pDescriptorHeap->GetCPUDescriptorHandleForHeapStart().ptr;
}

KAIROS_EXPORT_END