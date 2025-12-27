using System;

namespace KairosEngine.Graphics
{
	public class GraphicsFactory
	{
		public void* pInternalFactory;

		public int32 Create()
		{
			int32 hr = GraphicsFactory_Create(&pInternalFactory);
			return hr;
		}

		public ~this()
		{
			if(pInternalFactory != null)
			{
				GraphicsFactory_Dispose(pInternalFactory);
				pInternalFactory = null;
			}
		}

		public (int32 hr, GraphicsDevice device) CreateDevice()
		{
			GraphicsDevice device = new GraphicsDevice();
			int32 hr = GraphicsFactory_CreateDevice(pInternalFactory, &device.pInternalDevice);
			return (hr, device);
		}

		public (int32 hr, GraphicsSwapChain swapChain) CreateSwapChain(GraphicsCommandQueue commandQueue, int32 width, int32 height, RenderTargetFormat format, int32 msaa, int32 aaQuality, int32 bufferCount, int32 windowId)
		{
			var windowComponents = WindowComponents.Instance;
			int32 wndIndex = windowComponents.IdToIndex[windowId];
			Windows.HWnd hwnd = windowComponents.Hwnds[wndIndex];
			var flags = windowComponents.Flags[wndIndex];
			bool windowed = (flags & WindowComponents.WindowFlags.FullScreen) > 0 ? false : true;
			GraphicsSwapChain swapChain = new GraphicsSwapChain();
			int32 hr = GraphicsFactory_CreateSwapChain(pInternalFactory, &swapChain.pInternalSwapChain, commandQueue.pInternalCommandQueue, width, height, format, msaa, aaQuality, bufferCount, hwnd, windowed);
			return (hr, swapChain);
		}

		public int32 CheckFeatureSupport(Feature feature, void* pFeatureSupportData, uint32 featureSupportDataSize)
		{
			return GraphicsFactory_CheckFeatureSupport(pInternalFactory, feature, pFeatureSupportData, featureSupportDataSize);
		}
	}
}