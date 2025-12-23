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

		public uint32 GetCurrentBackBufferIndex() mut
		{
			return GraphicsSwapChain_GetCurrentBackBufferIndex(&this);
		}

		public (int32 hr, GraphicsRenderTarget renderTarget) GetRenderTarget(int32 index) mut
		{
			GraphicsRenderTarget renderTarget = GraphicsRenderTarget();
			int32 hr = GraphicsSwapChain_GetRenderTarget(&this, &renderTarget, index);
			return (hr, renderTarget);
		}
	}
}