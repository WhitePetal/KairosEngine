using System;

namespace KairosEngine.Graphics
{
	extension GraphicsSwapChain
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsSwapChain_Dispose")]
		private static extern void GraphicsSwapChain_Dispose(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsSwapChain_GetCurrentBackBufferIndex")]
		private static extern uint GraphicsSwapChain_GetCurrentBackBufferIndex(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsSwapChain_GetRenderTarget")]
		private static extern CreateResult GraphicsSwapChain_GetRenderTarget(void* _this, int index);
	}
}