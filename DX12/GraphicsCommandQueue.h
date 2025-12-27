#ifndef __KAIROS_GRAPHICS_COMMAND_QUEUE__
#define __KAIROS_GRAPHICS_COMMAND_QUEUE__

#include "KairosEngineDefines.h"
#include "GraphicsCommandList.h"
#include "GraphicsFence.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandQueue_Dispose(ID3D12CommandQueue* _this);

void KAIROS_API GraphicsCommandQueue_ExecuteCommandLists(ID3D12CommandQueue* _this, ID3D12CommandList** ppCommandLst, int executeCount);

int KAIROS_API GraphicsCommandQueue_Signal(ID3D12CommandQueue* _this, ID3D12Fence* pFence, UINT64 fenceValue);

KAIROS_EXPORT_END

#endif
