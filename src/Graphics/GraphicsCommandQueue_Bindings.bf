using System;

namespace KairosEngine.Graphics
{
	extension GraphicsCommandQueue
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandQueue_Dispose")]
		private static extern void GraphicsCommandQueue_Dispose(GraphicsCommandQueue* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandQueue_ExecuteCommandLists")]
		private static extern void GraphicsCommandQueue_ExecuteCommandLists(GraphicsCommandQueue* _this, GraphicsCommandList* pGraphicsCommandList, int32 executeCount);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandQueue_Signal")]
		private static extern int32 GraphicsCommandQueue_Signal(GraphicsCommandQueue* _this, GraphicsFence* pGraphicsFence, uint64 fenceValue);
	}
}