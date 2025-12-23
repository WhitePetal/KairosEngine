using System;

namespace KairosEngine.Graphics
{
	extension GraphicsSwapChain
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsSwapChain_Dispose")]
		private static extern void GraphicsSwapChain_Dispose(GraphicsSwapChain* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsSwapChain_GetCurrentBackBufferIndex")]
		private static extern uint GraphicsSwapChain_GetCurrentBackBufferIndex(GraphicsSwapChain* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsSwapChain_GetRenderTarget")]
		private static extern int GraphicsSwapChain_GetRenderTarget(GraphicsSwapChain* _this, GraphicsRenderTarget* pGraphicsRenderTarget, int index);
	}
}