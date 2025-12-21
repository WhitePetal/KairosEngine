using System;

namespace KairosEngine.Graphics
{
	extension GraphicsDevice
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_Dispose")]
		private static extern void GraphicsDevice_Dispose(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateCommandQueue")]
		private static extern CreateResult GraphicsDevice_CreateCommandQueue(void* _this, CommandListType type, int priority, CommandQueueFlags flags, uint nodeMask);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_GetDescriptorHandleIncrementSize")]
		private static extern uint GraphicsDevice_GetDescriptorHandleIncrementSize(void* _this, DescriptorHeapType type);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateDescriptorHeap")]
		private static extern CreateResult GraphicsDevice_CreateDescriptorHeap(void* _this, int count, DescriptorHeapType type, DescriptorHeapFlags flags);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateRenderTargetViewByHandle")]
		private static extern void GraphicsDevice_CreateRenderTargetView(void* _this, void* pRenderTarget, uint64 handle);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_CreateRenderTargetViewByHeapIndex")]
		private extern static void GraphicsDevice_CreateRenderTargetView(void* _this, void* pRenderTarget, void* pDescriptorHeap, int index);
	}
}