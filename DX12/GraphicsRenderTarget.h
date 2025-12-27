#ifndef __KAIROS_GRAPHICS_RENDER_TARGET__
#define __KAIROS_GRAPHICS_RENDER_TARGET__

#include "KairosEngineDefines.h"
#include "GraphicsResource.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"


KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsRenderTarget_Dispose(ID3D12Resource* _this);

KAIROS_EXPORT_END

#endif
