#include "GraphicsSwapChain.h"

GraphicsSwapChain::GraphicsSwapChain(IDXGISwapChain3* pSwapChain)
{
	m_pSwapChain = pSwapChain;
}

void GraphicsSwapChain::Dispose()
{
	SAFE_RELEASE(m_pSwapChain);
}

UINT GraphicsSwapChain::GetCurrentBackBufferIndex()
{
	return m_pSwapChain->GetCurrentBackBufferIndex();
}


KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsSwapChain_Dispose(GraphicsSwapChain* _this)
{
	_this->Dispose();
	delete _this;
}

UINT KAIROS_API GraphicsSwapChain_GetCurrentBackBufferIndex(GraphicsSwapChain* _this)
{
	return _this->GetCurrentBackBufferIndex();
}


KAIROS_EXPORT_END