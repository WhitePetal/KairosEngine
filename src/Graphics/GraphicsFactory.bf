using System;

namespace KairosEngine.Graphics
{
	public struct GraphicsFactory
	{
		private void* m_pFactory;

		public int Create() mut
		{
			m_pFactory = GraphicsFactory_Allocate();
			return GraphicsFactory_Create(m_pFactory);
		}

		public void Dispose()
		{	if(m_pFactory != null)
				GraphicsFactory_Dispose(m_pFactory);
		}

		public (int hr, GraphicsDevice device) CreateDevice()
		{
			CreateResult result = GraphicsFactory_CreateDevice(m_pFactory);
			return (result.HR, GraphicsDevice(result.Ptr));
		}

		public (int hr, GraphicsSwapChain swapChain) CreateSwapChain(GraphicsCommandQueue commandQueue, int width, int height, RenderTargetFormat format, int msaa, int aaQuality, int bufferCount, int windowId)
		{
			var windowComponents = WindowComponents.Instance;
			int wndIndex = windowComponents.IdToIndex[windowId];
			Windows.HWnd hwnd = windowComponents.Hwnds[wndIndex];
			var flags = windowComponents.Flags[wndIndex];
			bool windowed = (flags & WindowComponents.WindowFlags.FullScreen) > 0 ? false : true;
			CreateResult result = GraphicsFactory_CreateSwapChain(m_pFactory, commandQueue.GetInternalPtr(), width, height, format, msaa, aaQuality, bufferCount, hwnd, windowed);
			return (result.HR, GraphicsSwapChain(result.Ptr));
		}
	}
}