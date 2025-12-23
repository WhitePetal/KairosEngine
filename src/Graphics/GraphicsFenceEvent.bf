namespace KairosEngine.Graphics
{
	public struct GraphicsFenceEvent
	{
		private void* m_pFenceEvent;

		public int Create() mut
		{
			CreateResult result = GraphicsFenceEvent_Create();
			m_pFenceEvent = result.Ptr;
			return result.HR;
		}

		public void Dispose()
		{
			if(m_pFenceEvent != null)
				GraphicsFenceEvent_Dispose(m_pFenceEvent);
		}
	}
}