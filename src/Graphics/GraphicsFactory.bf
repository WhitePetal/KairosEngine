using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsFactory
	{
		private void* m_pFactory;

		public int32 Create() mut
		{
			int32 hr = GraphicsFactory_Create(&this);
			return hr;
		}

		public void Dispose() mut
		{
			if(m_pFactory != null)
			{
				GraphicsFactory_Dispose(&this);
			}
		}

		public (int32 hr, GraphicsDevice device) CreateDevice() mut
		{
			GraphicsDevice device = GraphicsDevice();
			int32 hr = GraphicsFactory_CreateDevice(&this, &device);
			return (hr, device);
		}

		public (int32 hr, GraphicsSwapChain swapChain) CreateSwapChain(ref GraphicsCommandQueue commandQueue, int32 width, int32 height, RenderTargetFormat format, int32 msaa, int32 aaQuality, int32 bufferCount, int32 windowId) mut
		{
			var windowComponents = WindowComponents.Instance;
			int32 wndIndex = windowComponents.IdToIndex[windowId];
			Windows.HWnd hwnd = windowComponents.Hwnds[wndIndex];
			var flags = windowComponents.Flags[wndIndex];
			bool windowed = (flags & WindowComponents.WindowFlags.FullScreen) > 0 ? false : true;
			GraphicsSwapChain swapChain = GraphicsSwapChain();
			int32 hr = GraphicsFactory_CreateSwapChain(&this, &commandQueue, &swapChain, width, height, format, msaa, aaQuality, bufferCount, hwnd, windowed);
			return (hr, swapChain);
		}
	}
}