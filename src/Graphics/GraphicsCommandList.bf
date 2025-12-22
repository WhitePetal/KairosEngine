namespace KairosEngine.Graphics
{
	public struct GraphicsCommandList
	{
		private void* m_pGraphicsCommandList;

		public this(void* pGraphicsCommandList)
		{
			m_pGraphicsCommandList = pGraphicsCommandList;
		}

		public void Dispose()
		{
			if(m_pGraphicsCommandList != null)
				GraphicsCommandList_Dispose(m_pGraphicsCommandList);
		}
	}
}