namespace KairosEngine.Graphics
{
	public struct GraphicsSwapChain
	{
		private void* m_pSwapChain;

		public this(void* pSwapChain)
		{
			m_pSwapChain = pSwapChain;
		}

		public void Dispose()
		{
			if(m_pSwapChain != null)
				GraphicsSwapChain_Dispose(m_pSwapChain);
		}

		public uint GetCurrentBackBufferIndex()
		{
			return GraphicsSwapChain_GetCurrentBackBufferIndex(m_pSwapChain);
		}

		public (int hr, GraphicsRenderTarget renderTarget) GetRenderTarget(int index)
		{
			CreateResult result = GraphicsSwapChain_GetRenderTarget(m_pSwapChain, index);
			GraphicsRenderTarget renderTarget  = GraphicsRenderTarget(result.Ptr);
			return (result.HR, renderTarget);
		}
	}
}