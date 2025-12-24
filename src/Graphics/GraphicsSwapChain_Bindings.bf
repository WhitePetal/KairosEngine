using System;

namespace KairosEngine.Graphics
{
	extension GraphicsSwapChain
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsSwapChain_Dispose")]
		private static extern void GraphicsSwapChain_Dispose(GraphicsSwapChain* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsSwapChain_GetCurrentBackBufferIndex")]
		private static extern uint32 GraphicsSwapChain_GetCurrentBackBufferIndex(GraphicsSwapChain* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsSwapChain_GetRenderTarget")]
		private static extern int32 GraphicsSwapChain_GetRenderTarget(GraphicsSwapChain* _this, GraphicsRenderTarget* pGraphicsRenderTarget, int32 index);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsSwapChain_Present")]
		private static extern int32 GraphicsSwapChain_Present(GraphicsSwapChain* _this, uint32 syncInternal, uint32 flags);
	}
}