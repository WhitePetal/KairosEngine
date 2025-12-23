#ifndef __KAIROS_GRAPHICS_DEVICE__
#define __KAIROS_GRAPHICS_DEVICE__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include "GraphicsCommandQueue.h"
#include "CreateResult.h"
#include "GraphicsDescriptorHeap.h"
#include "GraphicsRenderTarget.h"
#include "GraphicsCommandAllocator.h"
#include "GraphicsCommandList.h"
#include "GraphicsFence.h"
#include "GraphicsRootSignature.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

class GraphicsDevice
{
public:
	GraphicsDevice(ID3D12Device* pDevice);

	void Dispose();

	CreateResult CreateaCommandQueue(D3D12_COMMAND_LIST_TYPE type, int priority, D3D12_COMMAND_QUEUE_FLAGS flags, UINT nodeMask);

	CreateResult CreateDescriptorHeap(int count, D3D12_DESCRIPTOR_HEAP_TYPE type, D3D12_DESCRIPTOR_HEAP_FLAGS flags);

	UINT GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE descriptorHeapType);

	void CreateRenderTargetView(GraphicsRenderTarget* pRenderTarget, CD3DX12_CPU_DESCRIPTOR_HANDLE handle);

	void CreateRenderTargetView(GraphicsRenderTarget* pRenderTarget, GraphicsDescriptorHeap* pDescriptorHeap, int index);

	CreateResult CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE type);

	CreateResult CreateCommandList(GraphicsCommandAllocator* pCommandAllocator, D3D12_COMMAND_LIST_TYPE type, UINT nodeMask);

	CreateResult CreateFence(UINT64 initialValue, D3D12_FENCE_FLAGS flags);

	CreateResult CreateEmptyRootSignature(D3D12_ROOT_SIGNATURE_FLAGS flags);

private:
	ID3D12Device* m_pDevice;
};

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsDevice_Dispose(GraphicsDevice* _this);

CreateResult KAIROS_API GraphicsDevice_CreateCommandQueue(GraphicsDevice* _this, D3D12_COMMAND_LIST_TYPE type, int priority, D3D12_COMMAND_QUEUE_FLAGS flags, int nodeMask);

CreateResult KAIROS_API GraphicsDevice_CreateDescriptorHeap(GraphicsDevice* _this, int count, D3D12_DESCRIPTOR_HEAP_TYPE type, D3D12_DESCRIPTOR_HEAP_FLAGS flags);

UINT KAIROS_API GraphicsDevice_GetDescriptorHandleIncrementSize(GraphicsDevice* _this, D3D12_DESCRIPTOR_HEAP_TYPE descriptorHeapType);

void KAIROS_API GraphicsDevice_CreateRenderTargetViewByHandle(GraphicsDevice* _this, GraphicsRenderTarget* pRenderTarget, CD3DX12_CPU_DESCRIPTOR_HANDLE handle);

void KAIROS_API GraphicsDevice_CreateRenderTargetViewByHeapIndex(GraphicsDevice* _this, GraphicsRenderTarget* pRenderTarget, GraphicsDescriptorHeap* pDescriptorHeap, int index);

CreateResult KAIROS_API GraphicsDevice_CreateCommandAllocator(GraphicsDevice* _this, D3D12_COMMAND_LIST_TYPE type);

CreateResult KAIROS_API GraphicsDevice_CreateCommandList(GraphicsDevice* _this, GraphicsCommandAllocator* pCommandAllocator, D3D12_COMMAND_LIST_TYPE type, UINT nodeMask);

CreateResult KAIROS_API GraphicsDevice_CreateFence(GraphicsDevice* _this, UINT64 initialValue, D3D12_FENCE_FLAGS flags);

CreateResult KAIROS_API GraphicsDevice_CreateEmptyRootSignature(GraphicsDevice* _this, D3D12_ROOT_SIGNATURE_FLAGS flags);

KAIROS_EXPORT_END

#endif
