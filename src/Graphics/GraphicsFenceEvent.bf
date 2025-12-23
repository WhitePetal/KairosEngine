using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsFenceEvent
	{
		private void* m_pFenceEvent;

		public int Create() mut
		{
			int hr = GraphicsFenceEvent_Create(&this);
			return hr;
		}

		public void Dispose() mut
		{
			if(m_pFenceEvent != null)
				GraphicsFenceEvent_Dispose(&this);
		}
	}
}