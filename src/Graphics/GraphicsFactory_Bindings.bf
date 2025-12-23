using System;

namespace KairosEngine.Graphics
{
	extension GraphicsFactory
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_Create")]
		private static extern int32 GraphicsFactory_Create(GraphicsFactory* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_Dispose")]
		private static extern void GraphicsFactory_Dispose(GraphicsFactory* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_CreateDevice")]
		private static extern int32 GraphicsFactory_CreateDevice(GraphicsFactory* _this, GraphicsDevice* pGraphicsDevice);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_CreateSwapChain")]
		private static extern int32 GraphicsFactory_CreateSwapChain(GraphicsFactory* _this, GraphicsCommandQueue* pCommandQueue, GraphicsSwapChain* pGraphicsSwapChain, int32 width, int32 height, RenderTargetFormat format, int32 msaa, int32 aaQuality, int32 bufferCount, Windows.HWnd hwnd, bool windowed);
	}
}