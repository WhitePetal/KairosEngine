#ifndef __KAIROS_GRAPHICS_DEVICE__
#define __KAIROS_GRAPHICS_DEVICE__

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
#include "GraphicsResource.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"


KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsDevice_Dispose(ID3D12Device* _this);

int KAIROS_API GraphicsDevice_CreateCommandQueue(ID3D12Device* _this, ID3D12CommandQueue** ppCommandQueue, D3D12_COMMAND_LIST_TYPE type, int priority, D3D12_COMMAND_QUEUE_FLAGS flags, int nodeMask);

int KAIROS_API GraphicsDevice_CreateDescriptorHeap(ID3D12Device* _this, ID3D12DescriptorHeap** ppDescriptorHeap, int count, D3D12_DESCRIPTOR_HEAP_TYPE type, D3D12_DESCRIPTOR_HEAP_FLAGS flags);

UINT KAIROS_API GraphicsDevice_GetDescriptorHandleIncrementSize(ID3D12Device* _this, D3D12_DESCRIPTOR_HEAP_TYPE descriptorHeapType);

void KAIROS_API GraphicsDevice_CreateRenderTargetViewByHandle(ID3D12Device* _this, ID3D12Resource* pRenderTarget, D3D12_CPU_DESCRIPTOR_HANDLE handle);

void KAIROS_API GraphicsDevice_CreateRenderTargetViewByHeapIndex(ID3D12Device* _this, ID3D12Resource* pRenderTarget, ID3D12DescriptorHeap* pDescriptorHeap, int index);

int KAIROS_API GraphicsDevice_CreateCommandAllocator(ID3D12Device* _this, ID3D12CommandAllocator** ppCommandAllocator, D3D12_COMMAND_LIST_TYPE type);

int KAIROS_API GraphicsDevice_CreateCommandList(ID3D12Device* _this, ID3D12CommandList** ppCommandList, ID3D12CommandAllocator* pCommandAllocator, D3D12_COMMAND_LIST_TYPE type, UINT nodeMask);

int KAIROS_API GraphicsDevice_CreateFence(ID3D12Device* _this, ID3D12Fence** ppFence, UINT64 initialValue, D3D12_FENCE_FLAGS flags);

int KAIROS_API GraphicsDevice_CreateEmptyRootSignature(ID3D12Device* _this, ID3D12RootSignature** ppRootSignature, D3D12_ROOT_SIGNATURE_FLAGS flags);

int KAIROS_API GraphicsDevice_CreateRootSignature(ID3D12Device* _this, ID3D12RootSignature** ppRootSignature, D3D12_ROOT_SIGNATURE_DESC* pDesc);

int KAIROS_API GraphicsDevice_CreatePipelineState(ID3D12Device* _this, ID3D12PipelineState** ppPipelineState, D3D12_INPUT_ELEMENT_DESC* pInputLayout, UINT inputLayoutCount, ID3D12RootSignature* pRootSignature,
	ID3DBlob* pVertexShader, ID3DBlob* pFragmentShader, D3D12_PRIMITIVE_TOPOLOGY_TYPE topologyType, DXGI_FORMAT renderTargetFormat, UINT msaa, UINT aaQuality, UINT sampleMask);

int KAIROS_API GraphicsDevice_CreateCommittedBufferResource(ID3D12Device* _this, ID3D12Resource** ppBuffer, D3D12_HEAP_TYPE heapType, UINT64 resourceSize, D3D12_HEAP_FLAGS heapFlags, D3D12_RESOURCE_STATES resourceStates);

KAIROS_EXPORT_END

#endif
