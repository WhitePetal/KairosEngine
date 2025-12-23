#include "GraphicsFence.h"

GraphicsFence::GraphicsFence(ID3D12Fence* pFence)
{
	m_pFence = pFence;
}

void GraphicsFence::Dispose()
{
	SAFE_RELEASE(m_pFence);
}

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsFence_Dispose(GraphicsFence* _this)
{
	_this->Dispose();
	delete _this;
}

KAIROS_EXPORT_END