using System;

namespace KairosEngine.Graphics
{
	extension GraphicsFactory
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_Create")]
		private static extern int GraphicsFactory_Create(GraphicsFactory* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_Dispose")]
		private static extern void GraphicsFactory_Dispose(GraphicsFactory* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_CreateDevice")]
		private static extern int GraphicsFactory_CreateDevice(GraphicsFactory* _this, GraphicsDevice* pGraphicsDevice);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_CreateSwapChain")]
		private static extern int GraphicsFactory_CreateSwapChain(GraphicsFactory* _this, GraphicsCommandQueue* pCommandQueue, GraphicsSwapChain* pGraphicsSwapChain, int width, int height, RenderTargetFormat format, int msaa, int aaQuality, int bufferCount, Windows.HWnd hwnd, bool windowed);
	}
}