#ifndef __KAIROS_GRAPHICS_DEVICE__
#define __KAIROS_GRAPHICS_DEVICE__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include "GraphicsCommandQueue.h"
#include "GraphicsDescriptorHeap.h"
#include "GraphicsRenderTarget.h"
#include "GraphicsCommandAllocator.h"
#include "GraphicsCommandList.h"
#include "GraphicsFence.h"
#include "GraphicsRootSignature.h"
#include "GraphicsPipelineState.h"
#include "GraphicsShader.h"
#include "GraphicsBuffer.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

typedef struct GraphicsDevice
{
	ID3D12Device* m_pDevice;
} GraphicsDevice;

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsDevice_Dispose(GraphicsDevice* _this);

int KAIROS_API GraphicsDevice_CreateCommandQueue(GraphicsDevice* _this, GraphicsCommandQueue* pGraphicsCommandQueue, D3D12_COMMAND_LIST_TYPE type, int priority, D3D12_COMMAND_QUEUE_FLAGS flags, int nodeMask);

int KAIROS_API GraphicsDevice_CreateDescriptorHeap(GraphicsDevice* _this, GraphicsDescriptorHeap* pGraphicsDescriptorHeap, int count, D3D12_DESCRIPTOR_HEAP_TYPE type, D3D12_DESCRIPTOR_HEAP_FLAGS flags);

UINT KAIROS_API GraphicsDevice_GetDescriptorHandleIncrementSize(GraphicsDevice* _this, D3D12_DESCRIPTOR_HEAP_TYPE descriptorHeapType);

void KAIROS_API GraphicsDevice_CreateRenderTargetViewByHandle(GraphicsDevice* _this, GraphicsRenderTarget* pGraphicsRenderTarget, CD3DX12_CPU_DESCRIPTOR_HANDLE handle);

void KAIROS_API GraphicsDevice_CreateRenderTargetViewByHeapIndex(GraphicsDevice* _this, GraphicsRenderTarget* pGraphicsRenderTarget, GraphicsDescriptorHeap* pDescriptorHeap, int index);

int KAIROS_API GraphicsDevice_CreateCommandAllocator(GraphicsDevice* _this, GraphicsCommandAllocator* pGraphicsCommandAllocator, D3D12_COMMAND_LIST_TYPE type);

int KAIROS_API GraphicsDevice_CreateCommandList(GraphicsDevice* _this, GraphicsCommandAllocator* pGraphicsCommandAllocator, GraphicsCommandList* pGraphicsCommandList, D3D12_COMMAND_LIST_TYPE type, UINT nodeMask);

int KAIROS_API GraphicsDevice_CreateFence(GraphicsDevice* _this, GraphicsFence* pGraphicsFence, UINT64 initialValue, D3D12_FENCE_FLAGS flags);

int KAIROS_API GraphicsDevice_CreateEmptyRootSignature(GraphicsDevice* _this, GraphicsRootSignature* pGraphicsRootSignature, D3D12_ROOT_SIGNATURE_FLAGS flags);

int KAIROS_API GraphicsDevice_CreatePipelineState(GraphicsDevice* _this, GraphicsPipelineState* pGraphicsPipelineState, D3D12_INPUT_ELEMENT_DESC* pInputLayout, UINT inputLayoutCount, GraphicsRootSignature* pGraphicsRootSignature,
	GraphicsShader* pGraphicsVertexShader, GraphicsShader* pGraphicsFragmentShader, D3D12_PRIMITIVE_TOPOLOGY_TYPE topologyType, DXGI_FORMAT renderTargetFormat, UINT msaa, UINT aaQuality, UINT sampleMask);

int KAIROS_API GraphicsDevice_CreateCommittedBufferResource(GraphicsDevice* _this, GraphicsBuffer* pGraphicsBuffer, D3D12_HEAP_TYPE heapType, UINT64 resourceSize, D3D12_HEAP_FLAGS heapFlags, D3D12_RESOURCE_STATES resourceStates);

KAIROS_EXPORT_END

#endif
