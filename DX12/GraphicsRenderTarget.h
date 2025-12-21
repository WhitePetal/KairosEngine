#ifndef __KAIROS_GRAPHICS_RENDER_TARGET__
#define __KAIROS_GRAPHICS_RENDER_TARGET__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include "CreateResult.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

class GraphicsRenderTarget
{
public:
	GraphicsRenderTarget(ID3D12Resource* pRenderTarget);

private:
	ID3D12Resource* m_pRenderTarget;

};

#endif
