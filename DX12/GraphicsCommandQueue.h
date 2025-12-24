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

#define SUPPORT_MAX_EXECUTE_COMMAND_LIST_COUNT 8

typedef struct GraphicsCommandQueue
{
	ID3D12CommandQueue* m_pCommandQueue;
} GraphicsCommandQueue;

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandQueue_Dispose(GraphicsCommandQueue* _this);

void KAIROS_API GraphicsCommandQueue_ExecuteCommandLists(GraphicsCommandQueue* _this, GraphicsCommandList* pGraphicsCommandLst, int executeCount);

int KAIROS_API GraphicsCommandQueue_Signal(GraphicsCommandQueue* _this, GraphicsFence* pGraphicsFence, UINT64 fenceValue);

KAIROS_EXPORT_END

#endif
