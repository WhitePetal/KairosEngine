using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsFactory
	{
		private void* m_pFactory;

		public int Create() mut
		{
			int hr = GraphicsFactory_Create(&this);
			return hr;
		}

		public void Dispose() mut
		{
			Console.WriteLine($"GraphicsFactory Native Ptr: {m_pFactory}");
			if(m_pFactory != null)
			{
				GraphicsFactory_Dispose(&this);
			}
		}

		public (int hr, GraphicsDevice device) CreateDevice() mut
		{
			CreateResult result = GraphicsFactory_CreateDevice(&this);
			return (result.HR, GraphicsDevice(result.Ptr));
		}

		public (int hr, GraphicsSwapChain swapChain) CreateSwapChain(GraphicsCommandQueue commandQueue, int width, int height, RenderTargetFormat format, int msaa, int aaQuality, int bufferCount, int windowId) mut
		{
			var windowComponents = WindowComponents.Instance;
			int wndIndex = windowComponents.IdToIndex[windowId];
			Windows.HWnd hwnd = windowComponents.Hwnds[wndIndex];
			var flags = windowComponents.Flags[wndIndex];
			bool windowed = (flags & WindowComponents.WindowFlags.FullScreen) > 0 ? false : true;
			CreateResult result = GraphicsFactory_CreateSwapChain(&this, commandQueue.GetInternalPtr(), width, height, format, msaa, aaQuality, bufferCount, hwnd, windowed);
			return (result.HR, GraphicsSwapChain(result.Ptr));
		}
	}
}