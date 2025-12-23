using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsSwapChain
	{
		private void* m_pSwapChain;

		public void Dispose() mut
		{
			if(m_pSwapChain != null)
				GraphicsSwapChain_Dispose(&this);
		}

		public uint GetCurrentBackBufferIndex() mut
		{
			return GraphicsSwapChain_GetCurrentBackBufferIndex(&this);
		}

		public (int hr, GraphicsRenderTarget renderTarget) GetRenderTarget(int index) mut
		{
			GraphicsRenderTarget renderTarget = GraphicsRenderTarget();
			int hr = GraphicsSwapChain_GetRenderTarget(&this, &renderTarget, index);
			return (hr, renderTarget);
		}
	}
}