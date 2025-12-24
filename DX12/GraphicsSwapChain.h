#ifndef __KAIROS_GRAPHICS_SWAP_CHAIN__
#define __KAIROS_GRAPHICS_SWAP_CHAIN__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include "GraphicsRenderTarget.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

typedef struct GraphicsSwapChain
{
	IDXGISwapChain3* m_pSwapChain;
} GraphicsSwapChain;

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsSwapChain_Dispose(GraphicsSwapChain* _this);

UINT KAIROS_API GraphicsSwapChain_GetCurrentBackBufferIndex(GraphicsSwapChain* _this);

int KAIROS_API GraphicsSwapChain_GetRenderTarget(GraphicsSwapChain* _this, GraphicsRenderTarget* pGraphicsRenderTarget, int index);

int KAIROS_API GraphicsSwapChain_Present(GraphicsSwapChain* _this, UINT syncInternal, UINT flags);

KAIROS_EXPORT_END

#endif // !__KAIROS_GRAPHICS_SWAP_CHAIN__
