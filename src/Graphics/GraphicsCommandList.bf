using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsCommandList
	{
		private void* m_pGraphicsCommandList;

		public void Dispose() mut
		{
			if(m_pGraphicsCommandList != null)
				GraphicsCommandList_Dispose(&this);
		}

		public int32 UpdateSubResources<T>(ref GraphicsResource dstResource, ref GraphicsResource fromResource,
			uint32 intermediateOffset, uint32 firstSubResource, uint32 numSubResource, T[] data, int dataSize) mut where T : struct
		{
			return GraphicsCommandList_UpdateSubResources(&this, &dstResource, &fromResource, intermediateOffset, firstSubResource, numSubResource, &data[0], dataSize);
		}

		public void ResourceBarrier(ref GraphicsResource resource, ResourceStates beforeStates, ResourceStates afterStates) mut
		{
			GraphicsCommandList_ResourceBarrier(&this, &resource, beforeStates, afterStates);
		}

		public int32 Close() mut
		{
			return GraphicsCommandList_Close(&this);
		}
	}
}