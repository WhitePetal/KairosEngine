using System;

namespace KairosEngine.Graphics
{
	public class GraphicsDescriptorHeap
	{
		public void* pInternalDescriptorHeap;

		public ~this()
		{
			if(pInternalDescriptorHeap != null)
			{
				GraphicsDescriptorHeap_Dispose(pInternalDescriptorHeap);
				pInternalDescriptorHeap = null;
			}
		}

		public DescriptorHeapDesc GetDesc()
		{
			return GraphicsDescriptorHeap_GetDesc(pInternalDescriptorHeap);
		}

		public DescriptorCpuHandle GetCPUDescriptorHandleForHeapStart()
		{
			return GraphicsDescriptorHeap_GetCPUDescriptorHandleForHeapStart(pInternalDescriptorHeap);
		}

		public DescriptorGpuHandle GetGPUDescriptorHandleForHeapStart()
		{
			return GraphicsDescriptorHeap_GetGPUDescriptorHandleForHeapStart(pInternalDescriptorHeap);
		}
	}
}