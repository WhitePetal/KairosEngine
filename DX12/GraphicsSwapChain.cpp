#include "GraphicsSwapChain.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsSwapChain_Dispose(IDXGISwapChain3* _this)
{
	_this->Release();
}

UINT KAIROS_API GraphicsSwapChain_GetCurrentBackBufferIndex(IDXGISwapChain3* _this)
{
	return _this->GetCurrentBackBufferIndex();
}

int KAIROS_API GraphicsSwapChain_GetRenderTarget(IDXGISwapChain3* _this, ID3D12Resource** ppRenderTarget, int index)
{
	ID3D12Resource* pRenderTarget;
	HRESULT hr = _this->GetBuffer(index, IID_PPV_ARGS(&pRenderTarget));
	*ppRenderTarget = pRenderTarget;
	return hr;
}

int KAIROS_API GraphicsSwapChain_Present(IDXGISwapChain3* _this, UINT syncInternal, UINT flags)
{
	HRESULT hr = _this->Present(syncInternal, flags);
	return hr;
}


KAIROS_EXPORT_END