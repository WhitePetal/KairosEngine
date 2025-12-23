using System;

namespace KairosEngine.Graphics
{
	extension GraphicsDevice
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_Dispose")]
		private static extern void GraphicsDevice_Dispose(GraphicsDevice* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateCommandQueue")]
		private static extern int32 GraphicsDevice_CreateCommandQueue(GraphicsDevice* _this, GraphicsCommandQueue* pGraphicsCommandQueue, CommandListType type, int32 priority, CommandQueueFlags flags, uint32 nodeMask);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateDescriptorHeap")]
		private static extern int32 GraphicsDevice_CreateDescriptorHeap(GraphicsDevice* _this, GraphicsDescriptorHeap* pGraphicsDescriptorHeap, int32 count, DescriptorHeapType type, DescriptorHeapFlags flags);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_GetDescriptorHandleIncrementSize")]
		private static extern uint32 GraphicsDevice_GetDescriptorHandleIncrementSize(GraphicsDevice* _this, DescriptorHeapType type);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateRenderTargetViewByHandle")]
		private static extern void GraphicsDevice_CreateRenderTargetView(GraphicsDevice* _this, GraphicsRenderTarget* pGraphicsRenderTarget, uint64 handle);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateRenderTargetViewByHeapIndex")]
		private static extern void GraphicsDevice_CreateRenderTargetView(GraphicsDevice* _this, GraphicsRenderTarget* pGraphicsRenderTarget, GraphicsDescriptorHeap* pDescriptorHeap, int32 index);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateCommandAllocator")]
		private static extern int32 GraphicsDevice_CreateCommandAllocator(GraphicsDevice* _this, GraphicsCommandAllocator* pGraphicsCommandAllocator, CommandListType type);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateCommandList")]
		private static extern int32 GraphicsDevice_CreateCommandList(GraphicsDevice* _this, GraphicsCommandAllocator* pGraphicsCommandAllocator, GraphicsCommandList* pGraphicsCommandList, CommandListType type, uint32 nodeMask);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateFence")]
		private static extern int32 GraphicsDevice_CreateFence(GraphicsDevice* _this, GraphicsFence* pGraphicsFence, uint64 initialValue, FenceFlags flags);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateEmptyRootSignature")]
		private static extern int32 GraphicsDevice_CreateEmptyRootSignature(GraphicsDevice* _this, GraphicsRootSignature* pGraphicsRootSignature, RootSignatureFlags flags);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreatePipelineState")]
		private static extern int32 GraphicsDevice_CreatePipelineState(GraphicsDevice* _this, GraphicsPipelineState* pGraphicsPipelineState, GraphicsInputLayoutElement* pInputlayouts, uint32 inputLayoutCount, GraphicsRootSignature* pGraphicsRootSignature,
			GraphicsShader* pGraphicsVertexShader, GraphicsShader* pGraphicsFragmentShader, TopologyType topologyType, RenderTargetFormat renderTargetFormat, uint32 msaa, uint32 aaQuality, uint32 sampleMask);
	}
}