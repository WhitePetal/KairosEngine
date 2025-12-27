#ifndef __KAIROS_GRAPHICS_FENCE__
#define __KAIROS_GRAPHICS_FENCE__

#include "KairosEngineDefines.h"
#include "FenceEvent.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsFence_Dispose(ID3D12Fence* _this);

UINT64 KAIROS_API GraphicsFence_GetCompletedValue(ID3D12Fence* _this);

int KAIROS_API GraphicsFence_SetEventOnCompletion(ID3D12Fence* _this, UINT64 fenceValue, HANDLE* pFenceEvent);

KAIROS_EXPORT_END

#endif