#ifndef __KAIROS_GRAPHICS_SWAP_CHAIN__
#define __KAIROS_GRAPHICS_SWAP_CHAIN__

#include "KairosEngineDefines.h"
#include "GraphicsRenderTarget.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsSwapChain_Dispose(IDXGISwapChain3* _this);

UINT KAIROS_API GraphicsSwapChain_GetCurrentBackBufferIndex(IDXGISwapChain3* _this);

int KAIROS_API GraphicsSwapChain_GetRenderTarget(IDXGISwapChain3* _this, ID3D12Resource** ppRenderTarget, int index);

int KAIROS_API GraphicsSwapChain_Present(IDXGISwapChain3* _this, UINT syncInternal, UINT flags);

KAIROS_EXPORT_END

#endif // !__KAIROS_GRAPHICS_SWAP_CHAIN__
