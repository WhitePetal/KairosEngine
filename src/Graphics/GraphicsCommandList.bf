using System;
using KairosEngine.Math;

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

		public int32 Reset(ref GraphicsCommandAllocator commandAllocator, ref GraphicsPipelineState pso) mut
		{
			return GraphicsCommandList_Reset(&this, &commandAllocator, &pso);
		}

		public void OMSetRenderTargets(ref GraphicsDescriptorHeap descriptorHeap, uint32 descriptorOffset, uint32 descriptorSize, uint32 descriptorCount) mut
		{
			GraphicsCommandList_OMSetRenderTargets(&this, &descriptorHeap, descriptorOffset, descriptorSize, descriptorCount);
		}

		public void ClearRenderTargetView(ref GraphicsDescriptorHeap descriptorHeap, uint32 descriptorOffset, uint32 descriptorSize, ref float4 color, uint32 rectCount, Rect[] rects) mut
		{
			GraphicsCommandList_ClearRenderTargetView(&this, &descriptorHeap, descriptorOffset, descriptorSize, &color, rectCount, rectCount == 0 ? null : &rects[0]);
		}
	}
}