using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsFenceEvent
	{
		private void* m_pFenceEvent;

		public int32 Create() mut
		{
			int32 hr = GraphicsFenceEvent_Create(&this);
			return hr;
		}

		public void Dispose() mut
		{
			if(m_pFenceEvent != null)
				GraphicsFenceEvent_Dispose(&this);
		}
	}
}