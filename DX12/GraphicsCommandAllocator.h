#ifndef __KAIROS_GRAPHICS_COMMAND_ALLOCATOR__
#define __KAIROS_GRAPHICS_COMMAND_ALLOCATOR__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include "CreateResult.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

typedef struct GraphicsCommandAllocator
{
	ID3D12CommandAllocator* m_pCommandAllocator;
} GraphicsCommandAllocator;

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandAllocator_Dispose(GraphicsCommandAllocator* _this);

KAIROS_EXPORT_END

#endif
