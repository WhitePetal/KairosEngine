using System;
using KairosEngine.Math;

namespace KairosEngine.Graphics
{
	public class GraphicsCommandList
	{
		public void* pInternalCommandList;

		~this()
		{
			if(pInternalCommandList != null)
			{
				GraphicsCommandList_Dispose(pInternalCommandList);
				pInternalCommandList = null;
			}
		}

		public uint64 UpdateSubResources<T>(GraphicsResource dstResource, GraphicsResource fromResource, uint32 intermediateOffset, uint32 firstSubResource, uint32 numSubResource, T[] data, int dataSize) where T : struct
		{
			return GraphicsCommandList_UpdateSubResources(pInternalCommandList, dstResource.pInternalResource, &fromResource.pInternalResource, intermediateOffset, firstSubResource, numSubResource, &data[0], dataSize);
		}

		public void ResourceBarrier(GraphicsResource resource, ResourceStates beforeStates, ResourceStates afterStates)
		{
			GraphicsCommandList_ResourceBarrier(pInternalCommandList, resource.pInternalResource, beforeStates, afterStates);
		}

		public int32 Close()
		{
			return GraphicsCommandList_Close(pInternalCommandList);
		}

		public int32 Reset(GraphicsCommandAllocator commandAllocator, GraphicsPipelineState pso)
		{
			return GraphicsCommandList_Reset(pInternalCommandList, commandAllocator.pInternalCommandAllocator, pso.pInternalPipelineState);
		}

		public void OMSetRenderTargets(GraphicsDescriptorHeap descriptorHeap, uint32 descriptorOffset, uint32 descriptorSize, uint32 descriptorCount)
		{
			GraphicsCommandList_OMSetRenderTargets(pInternalCommandList, descriptorHeap.pInternalDescriptorHeap, descriptorOffset, descriptorSize, descriptorCount);
		}

		public void ClearRenderTargetView(GraphicsDescriptorHeap descriptorHeap, uint32 descriptorOffset, uint32 descriptorSize, ref float4 color, uint32 rectCount, Rect[] rects)
		{
			GraphicsCommandList_ClearRenderTargetView(pInternalCommandList, descriptorHeap.pInternalDescriptorHeap, descriptorOffset, descriptorSize, &color, rectCount, rectCount == 0 ? null : &rects[0]);
		}
	}
}