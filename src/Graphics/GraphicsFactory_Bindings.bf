using System;

namespace KairosEngine.Graphics
{
	extension GraphicsFactory
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_Allocate")]
		private static extern void* GraphicsFactory_Allocate();

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_Create")]
		private static extern int GraphicsFactory_Create(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_Dispose")]
		private static extern void GraphicsFactory_Dispose(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_CreateDevice")]
		private static extern CreateResult GraphicsFactory_CreateDevice(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_CreateSwapChain")]
		private static extern CreateResult GraphicsFactory_CreateSwapChain(void* _this, void* pCommandQueue, int width, int height, RenderTargetFormats format, int msaa, int aaQuality, int bufferCount, Windows.HWnd hwnd, bool windowed);
	}
}