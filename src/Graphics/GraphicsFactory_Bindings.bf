using System;

namespace KairosEngine.Graphics
{
	extension GraphicsFactory
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_Create")]
		private static extern int32 GraphicsFactory_Create(void** p_this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_Dispose")]
		private static extern void GraphicsFactory_Dispose(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_CreateDevice")]
		private static extern int32 GraphicsFactory_CreateDevice(void* _this, void** ppDevice);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_CreateSwapChain")]
		private static extern int32 GraphicsFactory_CreateSwapChain(void* _this, void** ppSwapChain, void* pCommandQueue, int32 width, int32 height, RenderTargetFormat format, int32 msaa, int32 aaQuality, int32 bufferCount, Windows.HWnd hwnd, bool windowed);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFactory_CheckFeatureSupport")]
		private static extern int32 GraphicsFactory_CheckFeatureSupport(void* _this, Feature feature, void* pFeatureSupportData, uint32 featureSupportDataSize);
	}
}