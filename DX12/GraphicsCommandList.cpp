#include "GraphicsCommandList.h"

GraphicsCommandList::GraphicsCommandList(ID3D12CommandList* pCommandList)
{
	m_pCommandList = pCommandList;
}

void GraphicsCommandList::Dispose()
{
	SAFE_RELEASE(m_pCommandList);
}

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandList_Dispose(GraphicsCommandList* _this)
{
	_this->Dispose();
	delete _this;
}

KAIROS_EXPORT_END