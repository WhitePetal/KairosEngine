namespace KairosEngine.Graphics
{
	public struct GraphicsCommandAllocator
	{
		private void* m_pCommandAllocator;

		public this(void* pCommandAllocator)
		{
			m_pCommandAllocator = pCommandAllocator;
		}

		public void Dispose()
		{
			if(m_pCommandAllocator != null)
				GraphicsCommandAllocator_Dispose(m_pCommandAllocator);
		}

		public void* GetInternalPtr()
		{
			return m_pCommandAllocator;
		}
	}
}