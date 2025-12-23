#include "GraphicsSwapChain.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsSwapChain_Dispose(GraphicsSwapChain* _this)
{
	SAFE_RELEASE(_this->m_pSwapChain);
}

UINT KAIROS_API GraphicsSwapChain_GetCurrentBackBufferIndex(GraphicsSwapChain* _this)
{
	return _this->m_pSwapChain->GetCurrentBackBufferIndex();
}

int KAIROS_API GraphicsSwapChain_GetRenderTarget(GraphicsSwapChain* _this, GraphicsRenderTarget* pGraphicsRenderTarget, int index)
{
	ID3D12Resource* pRenderTarget;
	HRESULT hr = _this->m_pSwapChain->GetBuffer(index, IID_PPV_ARGS(&pRenderTarget));
	if (FAILED(hr))
		return GetSwapChainRenderTargetFailed;

	pGraphicsRenderTarget->m_pRenderTarget = pRenderTarget;
	return GraphicsSuccess;
}


KAIROS_EXPORT_END