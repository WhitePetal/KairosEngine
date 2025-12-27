using System;

namespace KairosEngine.Graphics
{
	extension GraphicsDevice
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_Dispose")]
		private static extern void GraphicsDevice_Dispose(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateCommandQueue")]
		private static extern int32 GraphicsDevice_CreateCommandQueue(void* _this, void** ppCommandQueue, CommandListType type, int32 priority, CommandQueueFlags flags, uint32 nodeMask);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateDescriptorHeap")]
		private static extern int32 GraphicsDevice_CreateDescriptorHeap(void* _this, void** ppDescriptorHeap, int32 count, DescriptorHeapType type, DescriptorHeapFlags flags);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_GetDescriptorHandleIncrementSize")]
		private static extern uint32 GraphicsDevice_GetDescriptorHandleIncrementSize(void* _this, DescriptorHeapType type);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateRenderTargetViewByHandle")]
		private static extern void GraphicsDevice_CreateRenderTargetView(void* _this, void* pRenderTarget, DescriptorCpuHandle handle);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateRenderTargetViewByHeapIndex")]
		private static extern void GraphicsDevice_CreateRenderTargetView(void* _this, void* pRenderTarget, void* pDescriptorHeap, int32 index);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateCommandAllocator")]
		private static extern int32 GraphicsDevice_CreateCommandAllocator(void* _this, void** ppAllocator, CommandListType type);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateCommandList")]
		private static extern int32 GraphicsDevice_CreateCommandList(void* _this, void** pGraphicsCommandList, void* pCommandAllocator, CommandListType type, uint32 nodeMask);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateFence")]
		private static extern int32 GraphicsDevice_CreateFence(void* _this, void** pFence, uint64 initialValue, FenceFlags flags);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateEmptyRootSignature")]
		private static extern int32 GraphicsDevice_CreateEmptyRootSignature(void* _this, void** pRootSignature, RootSignatureFlags flags);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateEmptyRootSignature")]
		private static extern int32 GraphicsDevice_CreateRootSignature(void* _this, void** pRootSignature, RootSignatureDesc* pDesc);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreatePipelineState")]
		private static extern int32 GraphicsDevice_CreatePipelineState(void* _this, void** pPipelineState, InputLayoutElement* pInputlayouts, uint32 inputLayoutCount, void* pRootSignature,
			void* pVertexShader, void* pFragmentShader, TopologyType topologyType, RenderTargetFormat renderTargetFormat, uint32 msaa, uint32 aaQuality, uint32 sampleMask);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateCommittedBufferResource")]
		private static extern int32 GraphicsDevice_CreateCommittedBufferResource(void* _this, void** pBuffer, HeapType heapType, uint64 resourceSize, HeapFlags heapFlags, ResourceStates resourceStates);
	}
}