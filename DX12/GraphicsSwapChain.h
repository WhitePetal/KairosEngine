#ifndef __KAIROS_GRAPHICS_SWAP_CHAIN__
#define __KAIROS_GRAPHICS_SWAP_CHAIN__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include "CreateResult.h"
#include "GraphicsRenderTarget.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

class GraphicsSwapChain
{
public:
	GraphicsSwapChain(IDXGISwapChain3* pSwapChain);

	void Dispose();

	UINT GetCurrentBackBufferIndex();

	CreateResult GetRenderTarget(int index);

private:
	IDXGISwapChain3* m_pSwapChain;
};

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsSwapChain_Dispose(GraphicsSwapChain* _this);

UINT KAIROS_API GraphicsSwapChain_GetCurrentBackBufferIndex(GraphicsSwapChain* _this);

CreateResult GraphicsSwapChain_GetRenderTarget(GraphicsSwapChain* _this, int index);

KAIROS_EXPORT_END

#endif // !__KAIROS_GRAPHICS_SWAP_CHAIN__
