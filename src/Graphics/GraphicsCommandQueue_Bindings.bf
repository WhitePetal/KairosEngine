using System;

namespace KairosEngine.Graphics
{
	extension GraphicsCommandQueue
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandQueue_Dispose")]
		private static extern void GraphicsCommandQueue_Dispose(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandQueue_ExecuteCommandLists")]
		private static extern void GraphicsCommandQueue_ExecuteCommandLists(void* _this, void** ppCommandList, int32 executeCount);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandQueue_Signal")]
		private static extern int32 GraphicsCommandQueue_Signal(void* _this, void* pFence, uint64 fenceValue);
	}
}