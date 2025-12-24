using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsFence
	{
		private void* m_pFence;

		public void Dispose() mut
		{
			if(m_pFence != null)
				GraphicsFence_Dispose(&this);
		}

		public uint64 GetCompletedValue() mut
		{
			return GraphicsFence_GetCompletedValue(&this);
		}

		public int32 SetEventOnCompletion(ref GraphicsFenceEvent event, uint64 fenceValue) mut
		{
			return GraphicsFence_SetEventOnCompletion(&this, fenceValue, &event);
		}
	}
}