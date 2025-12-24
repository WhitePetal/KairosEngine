#ifndef __KAIROS_GRAPHICS_FENCE__
#define __KAIROS_GRAPHICS_FENCE__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include "GraphicsFenceEvent.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

typedef struct GraphicsFence
{
	ID3D12Fence* m_pFence;
} GraphicsFence;

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsFence_Dispose(GraphicsFence* _this);

UINT64 KAIROS_API GraphicsFence_GetCompletedValue(GraphicsFence* _this);

int KAIROS_API GraphicsFence_SetEventOnCompletion(GraphicsFence* _this, UINT64 fenceValue, GraphicsFenceEvent* pGraphicsFenceEvent);

KAIROS_EXPORT_END

#endif