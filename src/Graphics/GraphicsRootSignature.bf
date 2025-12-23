namespace KairosEngine.Graphics
{
	public struct GraphicsRootSignature
	{
		private void* m_pRootSignature;

		public this(void* pRootSignature)
		{
			m_pRootSignature = pRootSignature;
		}

		public void Dispose()
		{
			if(m_pRootSignature != null)
				GraphicsRootSignature_Dispose(m_pRootSignature);
		}
	}
}