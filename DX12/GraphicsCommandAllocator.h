#ifndef __KAIROS_GRAPHICS_COMMAND_ALLOCATOR__
#define __KAIROS_GRAPHICS_COMMAND_ALLOCATOR__

#include "KairosEngineDefines.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandAllocator_Dispose(ID3D12CommandAllocator* _this);

int KAIROS_API GraphicsCommandAllocator_Reset(ID3D12CommandAllocator* _this);

KAIROS_EXPORT_END

#endif
