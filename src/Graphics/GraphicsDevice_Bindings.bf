using System;

namespace KairosEngine.Graphics
{
	extension GraphicsDevice
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_Dispose")]
		private static extern void GraphicsDevice_Dispose(GraphicsDevice* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateCommandQueue")]
		private static extern int GraphicsDevice_CreateCommandQueue(GraphicsDevice* _this, GraphicsCommandQueue* pGraphicsCommandQueue, CommandListType type, int priority, CommandQueueFlags flags, uint nodeMask);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateDescriptorHeap")]
		private static extern int GraphicsDevice_CreateDescriptorHeap(GraphicsDevice* _this, GraphicsDescriptorHeap* pGraphicsDescriptorHeap, int count, DescriptorHeapType type, DescriptorHeapFlags flags);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_GetDescriptorHandleIncrementSize")]
		private static extern uint GraphicsDevice_GetDescriptorHandleIncrementSize(GraphicsDevice* _this, DescriptorHeapType type);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateRenderTargetViewByHandle")]
		private static extern void GraphicsDevice_CreateRenderTargetView(GraphicsDevice* _this, GraphicsRenderTarget* pGraphicsRenderTarget, uint64 handle);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateRenderTargetViewByHeapIndex")]
		private static extern void GraphicsDevice_CreateRenderTargetView(GraphicsDevice* _this, GraphicsRenderTarget* pGraphicsRenderTarget, GraphicsDescriptorHeap* pDescriptorHeap, int index);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateCommandAllocator")]
		private static extern int GraphicsDevice_CreateCommandAllocator(GraphicsDevice* _this, GraphicsCommandAllocator* pGraphicsCommandAllocator, CommandListType type);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateCommandList")]
		private static extern int GraphicsDevice_CreateCommandList(GraphicsDevice* _this, GraphicsCommandAllocator* pGraphicsCommandAllocator, GraphicsCommandList* pGraphicsCommandList, CommandListType type, uint nodeMask);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateFence")]
		private static extern int GraphicsDevice_CreateFence(GraphicsDevice* _this, GraphicsFence* pGraphicsFence, uint64 initialValue, FenceFlags flags);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateEmptyRootSignature")]
		private static extern int GraphicsDevice_CreateEmptyRootSignature(GraphicsDevice* _this, GraphicsRootSignature* pGraphicsRootSignature, RootSignatureFlags flags);
	}
}