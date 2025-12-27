#ifndef __KAIROS_GRAPHICS_COMMAND_LIST__
#define __KAIROS_GRAPHICS_COMMAND_LIST__

#include "KairosEngineDefines.h"
#include "GraphicsResource.h"
#include "GraphicsCommandAllocator.h"
#include "GraphicsPipelineState.h"
#include "GraphicsDescriptorHeap.h"
#include <windows.h>
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"
#include <string>

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandList_Dispose(ID3D12GraphicsCommandList* _this);

UINT64 KAIROS_API GraphicsCommandList_UpdateSubResources(ID3D12GraphicsCommandList* _this, ID3D12Resource* pDstResource, ID3D12Resource* pFromResource, UINT64 intermediateOffset, UINT firstSubResource, UINT numSubResource, void* pData, INT64 dataSize);

void KAIROS_API GraphicsCommandList_ResourceBarrier(ID3D12GraphicsCommandList* _this, ID3D12Resource* pResource, D3D12_RESOURCE_STATES beforeStates, D3D12_RESOURCE_STATES afterStates);

int KAIROS_API GraphicsCommandList_Close(ID3D12GraphicsCommandList* _this);

int KAIROS_API GraphicsCommandList_Reset(ID3D12GraphicsCommandList* _this, ID3D12CommandAllocator* pCommandAllocator, ID3D12PipelineState* pPipelineState);

void KAIROS_API GraphicsCommandList_OMSetRenderTargets(ID3D12GraphicsCommandList* _this, ID3D12DescriptorHeap* pDescriptorHeap, UINT descriptorOffset, UINT descriptorSize, UINT descriptorCount);

void KAIROS_API GraphicsCommandList_ClearRenderTargetView(ID3D12GraphicsCommandList* _this, ID3D12DescriptorHeap* pDescriptorHeap, UINT descriptorOffset, UINT descriptorSize, FLOAT* pColor, UINT rectCount, D3D12_RECT* pRects);

KAIROS_EXPORT_END

#endif