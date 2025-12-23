using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsDescriptorHeap
	{
		private void* m_pDescriptorHeap;

		public DescriptorHeapType Type;
		public DescriptorHeapFlags Flags;

		public void Dispose() mut
		{
			if(m_pDescriptorHeap != null)
				GraphicsDescriptorHeap_Dispose(&this);
		}

		public GraphicsCPUDescriptorHandle GetCPUDescriptorHandleForHeapStart() mut
		{
			return GraphicsCPUDescriptorHandle(GraphicsDescriptorHeap_GetCPUDescriptorHandleForHeapStart(&this));
		}
	}
}