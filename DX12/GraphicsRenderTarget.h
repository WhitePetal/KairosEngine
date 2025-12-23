#ifndef __KAIROS_GRAPHICS_RENDER_TARGET__
#define __KAIROS_GRAPHICS_RENDER_TARGET__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

typedef struct GraphicsRenderTarget
{
	ID3D12Resource* m_pRenderTarget;
} GraphicsRenderTarget;

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsRenderTarget_Dispose(GraphicsRenderTarget* _this);

KAIROS_EXPORT_END

#endif
