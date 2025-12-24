#ifndef __KAIROS_GRAPHICS_COMMAND_LIST__
#define __KAIROS_GRAPHICS_COMMAND_LIST__

#include "KairosEngineDefines.h"
#include "GraphicsResource.h"
#include <windows.h>
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"
#include <string>

typedef struct GraphicsCommandList
{
	ID3D12GraphicsCommandList* m_pCommandList;
} GraphicsCommandList;

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandList_Dispose(GraphicsCommandList* _this);

int KAIROS_API GraphicsCommandList_UpdateSubResources(GraphicsCommandList* _this, GraphicsResource* pDsfResource, GraphicsResource* pResource, UINT64 intermediateOffset, UINT firstSubResource, UINT numSubResource, void* pData, INT64 dataSize);

void KAIROS_API GraphicsCommandList_ResourceBarrier(GraphicsCommandList* _this, GraphicsResource* pResource, D3D12_RESOURCE_STATES beforeStates, D3D12_RESOURCE_STATES afterStates);

int KAIROS_API GraphicsCommandList_Close(GraphicsCommandList* _this);

KAIROS_EXPORT_END

#endif