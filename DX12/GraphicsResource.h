#ifndef __KAIROS_GRAPHICS_RESOURCE__
#define __KAIROS_GRAPHICS_RESOURCE__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

typedef struct GraphicsResource
{
	ID3D12Resource* m_pResource;

} GraphicsResource;


KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsResource_Dispose(GraphicsResource* _this);

KAIROS_EXPORT_END

#endif