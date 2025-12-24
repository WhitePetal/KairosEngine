using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsFenceEvent
	{
		public const uint32 INFINITE_WAIT_TIME = 0xFFFFFFFF;

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

		public uint32 Wait(uint32 dwMilliseconds) mut
		{
			return GraphicsFenceEvent_Wait(&this, dwMilliseconds);
		}
	}
}