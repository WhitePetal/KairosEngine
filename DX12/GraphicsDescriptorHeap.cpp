#include "GraphicsDescriptorHeap.h"

GraphicsDescriptorHeap::GraphicsDescriptorHeap(ID3D12DescriptorHeap* pDescriptorHeap)
{
	m_pDescriptorHeap = pDescriptorHeap;
}

void GraphicsDescriptorHeap::Dispose()
{
	SAFE_RELEASE(m_pDescriptorHeap);
}

ID3D12DescriptorHeap* GraphicsDescriptorHeap::GetInternalPtr()
{
	return m_pDescriptorHeap;
}

SIZE_T GraphicsDescriptorHeap::GetCPUDescriptorHandleForHeapStart()
{
	return  m_pDescriptorHeap->GetCPUDescriptorHandleForHeapStart().ptr;
}

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsDescriptorHeap_Dispose(GraphicsDescriptorHeap* _this)
{
	_this->Dispose();
	delete _this;
}

SIZE_T KAIROS_API GraphicsDescriptorHeap_GetCPUDescriptorHandleForHeapStart(GraphicsDescriptorHeap* _this)
{
	return _this->GetCPUDescriptorHandleForHeapStart();
}

KAIROS_EXPORT_END