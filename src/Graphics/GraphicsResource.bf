using System;

namespace KairosEngine.Graphics
{
	public class GraphicsResource
	{
		public void* pInternalResource;

		public ~this()
		{
			if(pInternalResource != null)
			{
				GraphicsResource_Dispose(pInternalResource);
				pInternalResource = null;
			}
		}

		public uint64 GetGPUVirtualAddress()
		{
			return GraphicsResource_GetGPUVirtualAddress(pInternalResource);
		}

		public void Unmap(uint32 subResource, uint64 begin, uint64 end)
		{
			GraphicsResource_Unmap(pInternalResource, subResource, begin, end);
		}
	}
}