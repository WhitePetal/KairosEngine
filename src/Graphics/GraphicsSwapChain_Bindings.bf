using System;

namespace KairosEngine.Graphics
{
	extension GraphicsSwapChain
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsSwapChain_Dispose")]
		private static extern void GraphicsSwapChain_Dispose(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsSwapChain_GetCurrentBackBufferIndex")]
		private static extern uint32 GraphicsSwapChain_GetCurrentBackBufferIndex(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsSwapChain_GetRenderTarget")]
		private static extern int32 GraphicsSwapChain_GetRenderTarget(void* _this, void** ppRenderTarget, int32 index);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsSwapChain_Present")]
		private static extern int32 GraphicsSwapChain_Present(void* _this, uint32 syncInternal, uint32 flags);
	}
}