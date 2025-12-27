using System;

namespace KairosEngine.Graphics
{
	public class GraphicsFence
	{
		public void* pInternalFence;

		public ~this()
		{
			if(pInternalFence != null)
			{
				GraphicsFence_Dispose(pInternalFence);
				pInternalFence = null;
			}
		}

		public uint64 GetCompletedValue()
		{
			return GraphicsFence_GetCompletedValue(pInternalFence);
		}

		public int32 SetEventOnCompletion(FenceEvent event, uint64 fenceValue)
		{
			return GraphicsFence_SetEventOnCompletion(pInternalFence, fenceValue, event.Handle);
		}
	}
}