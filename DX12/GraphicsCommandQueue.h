#ifndef __KAIROS_GRAPHICS_COMMAND_QUEUE__
#define __KAIROS_GRAPHICS_COMMAND_QUEUE__

#include "KairosEngineDefines.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

typedef struct GraphicsCommandQueue
{
	ID3D12CommandQueue* m_pCommandQueue;
} GraphicsCommandQueue;

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandQueue_Dispose(GraphicsCommandQueue* _this);

KAIROS_EXPORT_END

#endif
