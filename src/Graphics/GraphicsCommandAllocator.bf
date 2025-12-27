using System;

namespace KairosEngine.Graphics
{
	public class GraphicsCommandAllocator
	{
		public void* pInternalCommandAllocator;

		public ~this()
		{
			if(pInternalCommandAllocator != null)
			{
				GraphicsCommandAllocator_Dispose(pInternalCommandAllocator);
				pInternalCommandAllocator = null;
			}
		}

		public int32 Reset()
		{
			return GraphicsCommandAllocator_Reset(pInternalCommandAllocator);
		}
	}
}