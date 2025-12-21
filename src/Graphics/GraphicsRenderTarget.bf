namespace KairosEngine.Graphics
{
	public struct GraphicsRenderTarget
	{
		private void* m_pRenderTarget;

		public this(void* pRenderTarget)
		{
			m_pRenderTarget = pRenderTarget;
		}

		public void Dispose()
		{
			if(m_pRenderTarget != null)
				GraphicsRenderTarget_Dispose(m_pRenderTarget);
		}

		public void* GetInternalPtr()
		{
			return m_pRenderTarget;
		}
	}
}