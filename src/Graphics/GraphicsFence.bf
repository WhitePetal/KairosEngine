namespace KairosEngine.Graphics
{
	public struct GraphicsFence
	{
		private void* m_pFence;

		public this(void* pFence)
		{
			m_pFence = pFence;
		}

		public void Dispose()
		{
			if(m_pFence != null)
				GraphicsFence_Dispose(m_pFence);
		}
	}
}