#include "GraphicsRenderTarget.h"

GraphicsRenderTarget::GraphicsRenderTarget(ID3D12Resource* pRenderTarget)
{
	m_pRenderTarget = pRenderTarget;
}
