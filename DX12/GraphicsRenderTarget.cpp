#include "GraphicsRenderTarget.h"

GraphicsRenderTarget::GraphicsRenderTarget(ID3D12Resource* pRenderTarget)
{
	m_pRenderTarget = pRenderTarget;
}

void GraphicsRenderTarget::Dispose()
{
	SAFE_RELEASE(m_pRenderTarget);
}

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsRenderTarget_Dispose(GraphicsRenderTarget* _this)
{
	_this->Dispose();
	delete _this;
}

KAIROS_EXPORT_END