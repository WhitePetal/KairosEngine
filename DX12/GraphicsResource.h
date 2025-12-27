#ifndef __KAIROS_GRAPHICS_RESOURCE__
#define __KAIROS_GRAPHICS_RESOURCE__

#include "KairosEngineDefines.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"


KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsResource_Dispose(ID3D12Resource* _this);

UINT64 KAIROS_API GraphicsResource_GetGPUVirtualAddress(ID3D12Resource* _this);

void KAIROS_API GraphicsResource_Unmap(ID3D12Resource* _this, UINT subResource, UINT64 begin, UINT64 end);

KAIROS_EXPORT_END

#endif