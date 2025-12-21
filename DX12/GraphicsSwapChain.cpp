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

CreateResult GraphicsSwapChain::GetRenderTarget(int index)
{
	ID3D12Resource* pRenderTarget;
	HRESULT hr = m_pSwapChain->GetBuffer(index, IID_PPV_ARGS(&pRenderTarget));
	if (FAILED(hr))
		return CreateResult{ GetSwapChainRenderTargetFailed , nullptr };

	return CreateResult{ GraphicsSuccess, new GraphicsRenderTarget{ pRenderTarget } };
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

CreateResult GraphicsSwapChain_GetRenderTarget(GraphicsSwapChain* _this, int index)
{
	return _this->GetRenderTarget(index);
}


KAIROS_EXPORT_END